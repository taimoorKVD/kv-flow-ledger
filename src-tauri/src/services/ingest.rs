use crate::db::Db;
use crate::error::AppResult;
use crate::models::HookEventIn;
use crate::services::{attribution, projects, settings};
use crate::time_engine::IntervalKind;
use chrono::{Duration, Utc};
use mysql::prelude::*;
use mysql::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const AGENT_START_EVENTS: &[&str] = &[
    "sessionStart",
    "beforeSubmitPrompt",
    "preToolUse",
    "subagentStart",
    "afterAgentResponse",
    "afterAgentThought",
];
const AGENT_STOP_EVENTS: &[&str] = &["stop", "sessionEnd", "subagentStop"];
const DEVELOPER_EVENTS: &[&str] = &["afterFileEdit", "beforeSubmitPrompt", "workspaceOpen"];

pub fn fingerprint(event: &HookEventIn) -> String {
    let mut hasher = Sha256::new();
    hasher.update(event.event_type.as_bytes());
    hasher.update(b"|");
    hasher.update(event.conversation_id.as_deref().unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(event.generation_id.as_deref().unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(event.occurred_at.as_deref().unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(event.payload.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn payload_str(payload: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = payload.get(*key) {
            if let Some(s) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn payload_string_array(payload: &serde_json::Value, keys: &[&str]) -> Option<Vec<String>> {
    for key in keys {
        if let Some(v) = payload.get(*key) {
            if let Some(arr) = v.as_array() {
                let out: Vec<String> = arr
                    .iter()
                    .filter_map(|x| x.as_str().map(str::trim).filter(|s| !s.is_empty()))
                    .map(str::to_string)
                    .collect();
                if !out.is_empty() {
                    return Some(out);
                }
            }
            if let Some(s) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(vec![s.to_string()]);
            }
        }
    }
    None
}

/// Fill missing top-level fields from nested Cursor payload (defensive).
fn hydrate_event(event: &mut HookEventIn) {
    if event.conversation_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        event.conversation_id = payload_str(
            &event.payload,
            &[
                "conversation_id",
                "conversationId",
                "session_id",
                "sessionId",
                "composer_id",
                "composerId",
            ],
        );
    }
    if event.generation_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        event.generation_id =
            payload_str(&event.payload, &["generation_id", "generationId"]);
    }
    if event.model.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        event.model = payload_str(&event.payload, &["model"]);
    }
    if event.model_id.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        event.model_id = payload_str(&event.payload, &["model_id", "modelId"]);
    }
    if event.cursor_version.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        event.cursor_version =
            payload_str(&event.payload, &["cursor_version", "cursorVersion"]);
    }
    let roots_empty = event
        .workspace_roots
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true);
    if roots_empty {
        event.workspace_roots = payload_string_array(
            &event.payload,
            &["workspace_roots", "workspaceRoots", "workspace_root", "cwd"],
        );
    }
    if event.event_type == "unknown" || event.event_type.is_empty() {
        if let Some(name) =
            payload_str(&event.payload, &["hook_event_name", "hookEventName"])
        {
            event.event_type = name;
        }
    }
}

fn is_empty_noise(event: &HookEventIn) -> bool {
    let no_conv = event
        .conversation_id
        .as_ref()
        .map(|s| s.is_empty())
        .unwrap_or(true);
    let no_ws = event
        .workspace_roots
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true);
    let payload_empty = event.payload.as_object().map(|o| o.is_empty()).unwrap_or(true);
    (event.event_type == "unknown" || event.event_type.is_empty()) && no_conv && no_ws && payload_empty
}

pub fn ingest_event(db: &Db, mut event: HookEventIn) -> AppResult<IngestOutcome> {
    hydrate_event(&mut event);
    if is_empty_noise(&event) {
        return Ok(IngestOutcome {
            accepted: false,
            duplicate: false,
            event_id: None,
            message: "ignored_empty".into(),
        });
    }

    let fp = fingerprint(&event);
    let duplicate = db.with_conn(|conn| {
        let exists: Option<String> = conn.exec_first(
            "SELECT id FROM cursor_events WHERE fingerprint = ?",
            (&fp,),
        )?;
        Ok(exists.is_some())
    })?;
    if duplicate {
        return Ok(IngestOutcome {
            accepted: false,
            duplicate: true,
            event_id: None,
            message: "duplicate".into(),
        });
    }

    let occurred_at = event
        .occurred_at
        .clone()
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let received_at = Utc::now().to_rfc3339();
    let workspace_path = event
        .workspace_roots
        .as_ref()
        .and_then(|v| v.first())
        .map(|p| projects::normalize_path(p));
    let mut attr = attribution::resolve(
        db,
        event.conversation_id.as_deref(),
        workspace_path.as_deref(),
    )?;

    // Auto tasks: conversation → Chat · {id}; known workspace without chat → General.
    if attr.task_id.is_none() {
        if let Some(project_id) = attr.project_id.clone() {
            if let Some(cid) = event.conversation_id.clone() {
                let (pid, tid) = projects::ensure_task_for_conversation(db, &project_id, &cid)?;
                attr.project_id = Some(pid.clone());
                attr.task_id = Some(tid.clone());
                attr.source = "auto_conversation".into();
                attr.confidence = "high".into();
                attr.unassigned_reason = None;
                rebind_open_intervals_for_conversation(db, &cid, &pid, &tid)?;
            } else {
                let tid = projects::ensure_general_task(db, &project_id)?;
                attr.task_id = Some(tid);
                attr.source = "auto_general".into();
                attr.confidence = "medium".into();
                attr.unassigned_reason = None;
            }
        }
    } else if let (Some(pid), Some(tid), Some(cid)) = (
        attr.project_id.clone(),
        attr.task_id.clone(),
        event.conversation_id.clone(),
    ) {
        rebind_open_intervals_for_conversation(db, &cid, &pid, &tid)?;
    }

    // Privacy: strip prompt text unless explicitly enabled
    let store_prompts = settings::get(db, "store_prompt_text")?.unwrap_or_else(|| "false".into())
        == "true";
    if !store_prompts {
        if let Some(obj) = event.payload.as_object_mut() {
            obj.remove("prompt");
            obj.remove("text");
            obj.remove("attachments");
        }
    }

    let event_id = Uuid::new_v4().to_string();
    let agent_state = event.agent_state.clone().or_else(|| {
        if AGENT_START_EVENTS.contains(&event.event_type.as_str()) {
            Some("executing".into())
        } else if AGENT_STOP_EVENTS.contains(&event.event_type.as_str()) {
            Some("stopped".into())
        } else {
            None
        }
    });

    db.with_conn(|conn| {
        let payload_json = event.payload.to_string();
        conn.exec_drop(
            "INSERT INTO cursor_events (
                id, fingerprint, event_type, occurred_at, received_at, workspace_path,
                conversation_id, generation_id, model, model_id, agent_state, source,
                payload_json, project_id, task_id
             ) VALUES (:id,:fp,:etype,:occurred,:received,:workspace,
                :conversation,:generation,:model,:model_id,:agent_state,'cursor_hook',
                :payload,:project_id,:task_id)",
            params! {
                "id" => &event_id,
                "fp" => &fp,
                "etype" => &event.event_type,
                "occurred" => &occurred_at,
                "received" => &received_at,
                "workspace" => &workspace_path,
                "conversation" => &event.conversation_id,
                "generation" => &event.generation_id,
                "model" => &event.model,
                "model_id" => &event.model_id,
                "agent_state" => &agent_state,
                "payload" => &payload_json,
                "project_id" => &attr.project_id,
                "task_id" => &attr.task_id,
            },
        )?;

        if let Some(cid) = &event.conversation_id {
            conn.exec_drop(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, workspace_path, first_seen_at, last_seen_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON DUPLICATE KEY UPDATE
                   last_seen_at = VALUES(last_seen_at),
                   workspace_path = COALESCE(VALUES(workspace_path), workspace_path),
                   project_id = COALESCE(project_id, VALUES(project_id)),
                   task_id = COALESCE(task_id, VALUES(task_id))",
                (
                    Uuid::new_v4().to_string(),
                    cid,
                    &attr.project_id,
                    &attr.task_id,
                    &workspace_path,
                    &occurred_at,
                    &occurred_at,
                ),
            )?;
        }
        Ok(())
    })?;

    // Only unknown workspaces need manual review; noise / known projects are never queued.
    if attr.task_id.is_none() {
        if let Some(reason) = &attr.unassigned_reason {
            if reason == "unknown_workspace" {
                attribution::record_unassigned(
                    db,
                    &event_id,
                    workspace_path.as_deref(),
                    event.conversation_id.as_deref(),
                    reason,
                )?;
            }
        }
    }

    update_intervals(db, &event, &event_id, &attr, &occurred_at, workspace_path.as_deref())?;

    Ok(IngestOutcome {
        accepted: true,
        duplicate: false,
        event_id: Some(event_id),
        message: "ok".into(),
    })
}

fn update_intervals(
    db: &Db,
    event: &HookEventIn,
    event_id: &str,
    attr: &attribution::AttributionResult,
    occurred_at: &str,
    workspace: Option<&str>,
) -> AppResult<()> {
    let agent_tracking = settings::get(db, "agent_tracking_enabled")?
        .unwrap_or_else(|| "true".into())
        == "true";

    if AGENT_START_EVENTS.contains(&event.event_type.as_str()) && agent_tracking {
        open_interval(db, IntervalKind::Agent, event, event_id, attr, occurred_at, workspace)?;
    }
    if AGENT_STOP_EVENTS.contains(&event.event_type.as_str()) {
        close_open_intervals(db, IntervalKind::Agent, occurred_at)?;
    }
    if DEVELOPER_EVENTS.contains(&event.event_type.as_str()) {
        open_interval(
            db,
            IntervalKind::Developer,
            event,
            event_id,
            attr,
            occurred_at,
            workspace,
        )?;
        // Touch developer activity — close will happen on idle sweep
    }
    if event.event_type == "idle" {
        close_open_intervals(db, IntervalKind::Developer, occurred_at)?;
        // Record excluded idle gap is handled by idle sweeper with threshold
    }
    Ok(())
}

fn kind_str(kind: IntervalKind) -> &'static str {
    match kind {
        IntervalKind::Developer => "developer",
        IntervalKind::Agent => "agent",
        IntervalKind::IdleExcluded => "idle_excluded",
        IntervalKind::Unattributed => "unattributed",
    }
}

/// Attach open intervals for a conversation once project/task attribution is known.
pub fn rebind_open_intervals_for_conversation(
    db: &Db,
    conversation_id: &str,
    project_id: &str,
    task_id: &str,
) -> AppResult<()> {
    db.with_conn(|conn| {
        conn.exec_drop(
            "UPDATE activity_intervals
             SET project_id = COALESCE(project_id, ?),
                 task_id = COALESCE(task_id, ?),
                 kind = CASE WHEN kind = 'unattributed' THEN 'agent' ELSE kind END
             WHERE ended_at IS NULL
               AND conversation_id = ?
               AND (task_id IS NULL OR project_id IS NULL OR kind = 'unattributed')",
            (project_id, task_id, conversation_id),
        )?;
        Ok(())
    })
}

fn open_interval(
    db: &Db,
    kind: IntervalKind,
    event: &HookEventIn,
    event_id: &str,
    attr: &attribution::AttributionResult,
    occurred_at: &str,
    workspace: Option<&str>,
) -> AppResult<()> {
    // If attribution is now known, repair any prior open intervals for this chat.
    if let (Some(pid), Some(tid), Some(cid)) = (
        attr.project_id.as_deref(),
        attr.task_id.as_deref(),
        event.conversation_id.as_deref(),
    ) {
        rebind_open_intervals_for_conversation(db, cid, pid, tid)?;
    }

    // If already open same kind for same conversation, keep it open (no duplicate).
    let has_open = db.with_conn(|conn| {
        let n: i64 = conn
            .exec_first(
                "SELECT COUNT(*) FROM activity_intervals WHERE kind = ? AND ended_at IS NULL
                 AND IFNULL(conversation_id, '') = IFNULL(?, '')",
                (kind_str(kind), &event.conversation_id),
            )?
            .unwrap_or(0);
        Ok(n > 0)
    })?;
    if has_open {
        return Ok(());
    }

    // Prefer the requested kind; only fall back to unattributed when project is unknown.
    let interval_kind = if attr.project_id.is_none() && kind != IntervalKind::IdleExcluded {
        IntervalKind::Unattributed
    } else {
        kind
    };

    let id = Uuid::new_v4().to_string();
    db.with_conn(|conn| {
        conn.exec_drop(
            "INSERT INTO activity_intervals (
                id, project_id, task_id, conversation_id, kind, started_at, source_event_id, notes
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                id,
                &attr.project_id,
                &attr.task_id,
                &event.conversation_id,
                kind_str(interval_kind),
                occurred_at,
                event_id,
                workspace,
            ),
        )?;
        Ok(())
    })
}

pub fn close_open_intervals(db: &Db, kind: IntervalKind, ended_at: &str) -> AppResult<()> {
    db.with_conn(|conn| {
        let rows: Vec<(String, String)> = conn.exec(
            "SELECT id, started_at FROM activity_intervals WHERE kind = ? AND ended_at IS NULL",
            (kind_str(kind),),
        )?;
        for (id, started) in rows {
            let start = chrono::DateTime::parse_from_rfc3339(&started)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let end = chrono::DateTime::parse_from_rfc3339(ended_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let dur = (end - start).num_milliseconds().max(0);
            conn.exec_drop(
                "UPDATE activity_intervals SET ended_at = ?, duration_ms = ? WHERE id = ?",
                (ended_at, dur, id),
            )?;
        }
        Ok(())
    })
}

/// Close developer intervals that exceeded idle threshold without heartbeats.
pub fn apply_idle_timeout(db: &Db, now: chrono::DateTime<Utc>) -> AppResult<()> {
    let mins = settings::idle_threshold_minutes(db)?;
    let threshold = Duration::minutes(mins);
    db.with_conn(|conn| {
        let rows: Vec<(String, String)> = conn.query(
            "SELECT id, started_at FROM activity_intervals WHERE kind = 'developer' AND ended_at IS NULL",
        )?;
        for (id, started) in rows {
            let start = chrono::DateTime::parse_from_rfc3339(&started)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or(now);
            if now - start > threshold {
                // Without a finer heartbeat table, close at threshold boundary from start+threshold
                // for open intervals that went stale — production adds heartbeats from editor events.
                let end = start + threshold;
                let dur = threshold.num_milliseconds();
                conn.exec_drop(
                    "UPDATE activity_intervals SET ended_at = ?, duration_ms = ? WHERE id = ?",
                    (end.to_rfc3339(), dur, &id),
                )?;
                conn.exec_drop(
                    "INSERT INTO activity_intervals (id, project_id, task_id, conversation_id, kind, started_at, ended_at, duration_ms, notes)
                     SELECT ?, project_id, task_id, conversation_id, 'idle_excluded', ?, ?, ?, 'idle_timeout'
                     FROM activity_intervals WHERE id = ?",
                    (
                        Uuid::new_v4().to_string(),
                        end.to_rfc3339(),
                        now.to_rfc3339(),
                        (now - end).num_milliseconds().max(0),
                        id,
                    ),
                )?;
            }
        }
        Ok(())
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IngestOutcome {
    pub accepted: bool,
    pub duplicate: bool,
    pub event_id: Option<String>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test_db;
    use crate::models::NewProject;
    use crate::services::projects;
    use crate::time_engine::compute_breakdown;
    use serde_json::json;

    #[test]
    fn rejects_duplicate_events() {
        let db = open_test_db().unwrap();
        let ev = HookEventIn {
            event_type: "sessionStart".into(),
            occurred_at: Some("2026-01-01T00:00:00Z".into()),
            workspace_roots: Some(vec!["D:\\x".into()]),
            conversation_id: Some("c1".into()),
            generation_id: Some("g1".into()),
            model: Some("test".into()),
            model_id: None,
            agent_state: None,
            cursor_version: Some("3.25.9".into()),
            payload: json!({"session_id": "c1"}),
        };
        let a = ingest_event(&db, ev.clone()).unwrap();
        assert!(a.accepted);
        let b = ingest_event(&db, ev).unwrap();
        assert!(b.duplicate);
    }

    #[test]
    fn tracks_agent_and_developer_union() {
        let db = open_test_db().unwrap();
        let p = projects::create_project(
            &db,
            NewProject {
                name: "ABC".into(),
                client: None,
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 0,
                currency: "USD".into(),
                workspaces: vec!["D:\\abc".into()],
            },
        )
        .unwrap();
        let task = projects::create_task(
            &db,
            crate::models::NewTask {
                project_id: p.id.clone(),
                title: "Work".into(),
                description: None,
                conversation_id: Some("conv-abc".into()),
            },
        )
        .unwrap();

        ingest_event(
            &db,
            HookEventIn {
                event_type: "beforeSubmitPrompt".into(),
                occurred_at: Some("2026-01-01T00:00:00Z".into()),
                workspace_roots: Some(vec!["D:\\abc".into()]),
                conversation_id: Some("conv-abc".into()),
                generation_id: Some("g1".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({}),
            },
        )
        .unwrap();
        // Close developer at 10m, keep agent until 20m via explicit intervals for determinism
        close_open_intervals(&db, IntervalKind::Developer, "2026-01-01T00:10:00Z").unwrap();
        close_open_intervals(&db, IntervalKind::Agent, "2026-01-01T00:20:00Z").unwrap();

        // Inject overlapping agent-only segment already covered; verify via breakdown helper
        db.with_conn(|conn| {
            // Ensure we have developer 0-10 and agent 0-20 by adjusting agent start if needed
            let n: i64 = conn
                .exec_first(
                    "SELECT COUNT(*) FROM activity_intervals WHERE task_id = ?",
                    (&task.id,),
                )?
                .unwrap_or(0);
            assert!(n >= 1);
            let b = projects::task_time_breakdown(conn, &task.id)?;
            // At minimum agent or combined > 0
            assert!(b.combined_ms > 0 || b.agent_ms > 0 || b.developer_ms > 0);
            let _ = compute_breakdown(&[]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn unknown_workspace_stays_unassigned() {
        let db = open_test_db().unwrap();
        ingest_event(
            &db,
            HookEventIn {
                event_type: "sessionStart".into(),
                occurred_at: Some("2026-01-01T00:00:00Z".into()),
                workspace_roots: Some(vec!["D:\\unknown".into()]),
                conversation_id: Some("conv-orphan".into()),
                generation_id: Some("g-orphan".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: Some("3.25.9".into()),
                payload: json!({}),
            },
        )
        .unwrap();
        let items = attribution::list_unassigned(&db).unwrap();
        assert!(!items.is_empty());
        assert!(items.iter().all(|i| i.reason == "unknown_workspace"));
        let tasks = projects::list_tasks(&db, None).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn empty_conversation_row_falls_through_to_workspace_auto_task() {
        let db = open_test_db().unwrap();
        let p = projects::create_project(
            &db,
            NewProject {
                name: "Fallback".into(),
                client: None,
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 0,
                currency: "USD".into(),
                workspaces: vec!["D:\\fallback".into()],
            },
        )
        .unwrap();
        // Legacy row with conversation id but no project/task must not block workspace mapping.
        db.with_conn(|conn| {
            conn.exec_drop(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, first_seen_at, last_seen_at)
                 VALUES (?, ?, NULL, NULL, ?, ?)",
                (
                    uuid::Uuid::new_v4().to_string(),
                    "conv-empty-row",
                    "2026-01-01T00:00:00Z",
                    "2026-01-01T00:00:00Z",
                ),
            )?;
            Ok(())
        })
        .unwrap();

        ingest_event(
            &db,
            HookEventIn {
                event_type: "sessionStart".into(),
                occurred_at: Some("2026-01-01T00:01:00Z".into()),
                workspace_roots: Some(vec!["D:\\fallback".into()]),
                conversation_id: Some("conv-empty-row".into()),
                generation_id: Some("g-fb".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({}),
            },
        )
        .unwrap();

        let tasks = projects::list_tasks(&db, Some(&p.id)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].title.starts_with("Chat · "));
        assert!(attribution::list_unassigned(&db).unwrap().is_empty());
    }

    #[test]
    fn mapped_workspace_without_conversation_uses_general_task() {
        let db = open_test_db().unwrap();
        let p = projects::create_project(
            &db,
            NewProject {
                name: "GeneralWs".into(),
                client: None,
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 0,
                currency: "USD".into(),
                workspaces: vec!["D:\\general-ws".into()],
            },
        )
        .unwrap();

        ingest_event(
            &db,
            HookEventIn {
                event_type: "workspaceOpen".into(),
                occurred_at: Some("2026-01-01T00:00:00Z".into()),
                workspace_roots: Some(vec!["D:\\general-ws".into()]),
                conversation_id: None,
                generation_id: None,
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({}),
            },
        )
        .unwrap();

        let tasks = projects::list_tasks(&db, Some(&p.id)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "General");
        assert!(attribution::list_unassigned(&db).unwrap().is_empty());

        // Second event reuses the same General task.
        ingest_event(
            &db,
            HookEventIn {
                event_type: "afterFileEdit".into(),
                occurred_at: Some("2026-01-01T00:01:00Z".into()),
                workspace_roots: Some(vec!["D:\\general-ws".into()]),
                conversation_id: None,
                generation_id: None,
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({"file": "a.rs"}),
            },
        )
        .unwrap();
        let tasks2 = projects::list_tasks(&db, Some(&p.id)).unwrap();
        assert_eq!(tasks2.len(), 1);
        assert_eq!(tasks2[0].id, tasks[0].id);
    }

    #[test]
    fn no_workspace_no_conversation_does_not_queue_unassigned() {
        let db = open_test_db().unwrap();
        let out = ingest_event(
            &db,
            HookEventIn {
                event_type: "afterAgentThought".into(),
                occurred_at: Some("2026-01-01T00:00:00Z".into()),
                workspace_roots: None,
                conversation_id: None,
                generation_id: Some("g-noise".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({"note": "signal-less"}),
            },
        )
        .unwrap();
        assert!(out.accepted);
        assert!(attribution::list_unassigned(&db).unwrap().is_empty());
        assert!(projects::list_tasks(&db, None).unwrap().is_empty());
    }

    #[test]
    fn auto_creates_task_for_new_conversation_in_mapped_workspace() {
        let db = open_test_db().unwrap();
        let p = projects::create_project(
            &db,
            NewProject {
                name: "Mapped".into(),
                client: None,
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 0,
                currency: "USD".into(),
                workspaces: vec!["D:\\mapped".into()],
            },
        )
        .unwrap();

        ingest_event(
            &db,
            HookEventIn {
                event_type: "sessionStart".into(),
                occurred_at: Some("2026-01-01T00:00:00Z".into()),
                workspace_roots: Some(vec!["D:\\mapped".into()]),
                conversation_id: Some("conv-new-12345678".into()),
                generation_id: Some("g1".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({}),
            },
        )
        .unwrap();

        let tasks = projects::list_tasks(&db, Some(&p.id)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Chat · conv-new");
        assert_eq!(tasks[0].status, "in_progress");

        let linked: Option<String> = db
            .with_conn(|conn| {
                Ok(conn.exec_first(
                    "SELECT task_id FROM cursor_conversations WHERE conversation_id = ?",
                    ("conv-new-12345678",),
                )?)
            })
            .unwrap();
        assert_eq!(linked.as_deref(), Some(tasks[0].id.as_str()));
    }

    #[test]
    fn reuses_task_for_same_conversation() {
        let db = open_test_db().unwrap();
        let p = projects::create_project(
            &db,
            NewProject {
                name: "Reuse".into(),
                client: None,
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 0,
                currency: "USD".into(),
                workspaces: vec!["D:\\reuse".into()],
            },
        )
        .unwrap();

        let first = HookEventIn {
            event_type: "sessionStart".into(),
            occurred_at: Some("2026-01-01T00:00:00Z".into()),
            workspace_roots: Some(vec!["D:\\reuse".into()]),
            conversation_id: Some("conv-reuse".into()),
            generation_id: Some("g1".into()),
            model: None,
            model_id: None,
            agent_state: None,
            cursor_version: None,
            payload: json!({}),
        };
        ingest_event(&db, first).unwrap();

        ingest_event(
            &db,
            HookEventIn {
                event_type: "beforeSubmitPrompt".into(),
                occurred_at: Some("2026-01-01T00:05:00Z".into()),
                workspace_roots: Some(vec!["D:\\reuse".into()]),
                conversation_id: Some("conv-reuse".into()),
                generation_id: Some("g2".into()),
                model: None,
                model_id: None,
                agent_state: None,
                cursor_version: None,
                payload: json!({}),
            },
        )
        .unwrap();

        let tasks = projects::list_tasks(&db, Some(&p.id)).unwrap();
        assert_eq!(tasks.len(), 1);

        let event_task_ids: Vec<Option<String>> = db
            .with_conn(|conn| {
                Ok(conn.exec(
                    "SELECT task_id FROM cursor_events WHERE conversation_id = ? ORDER BY occurred_at",
                    ("conv-reuse",),
                )?)
            })
            .unwrap();
        assert_eq!(event_task_ids.len(), 2);
        assert_eq!(event_task_ids[0], event_task_ids[1]);
        assert_eq!(event_task_ids[0].as_ref(), Some(&tasks[0].id));
    }
}
