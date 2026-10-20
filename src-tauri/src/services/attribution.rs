use crate::db::Db;
use crate::error::AppResult;
use crate::models::UnassignedItem;
use crate::services::projects;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AttributionResult {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub source: String,
    pub confidence: String,
    pub unassigned_reason: Option<String>,
}

/// Attribution hierarchy:
/// 1. Explicit conversation→task mapping
/// 2. Workspace→project (+ optional single active task suggestion only as unassigned if ambiguous)
/// 3. Unassigned
pub fn resolve(
    db: &Db,
    conversation_id: Option<&str>,
    workspace_path: Option<&str>,
) -> AppResult<AttributionResult> {
    if let Some(cid) = conversation_id {
        if let Some((project_id, task_id)) = lookup_conversation(db, cid)? {
            return Ok(AttributionResult {
                project_id,
                task_id,
                source: "conversation_mapping".into(),
                confidence: "high".into(),
                unassigned_reason: None,
            });
        }
    }

    if let Some(ws) = workspace_path {
        if let Some(project_id) = projects::find_project_by_workspace(db, ws)? {
            // Workspace alone is not enough to pick a task — leave task unassigned for review if required.
            let require_review = crate::services::settings::get(db, "require_attribution_review")?
                .unwrap_or_else(|| "true".into())
                == "true";
            if require_review {
                return Ok(AttributionResult {
                    project_id: Some(project_id),
                    task_id: None,
                    source: "workspace_mapping".into(),
                    confidence: "medium".into(),
                    unassigned_reason: Some("workspace_known_task_unknown".into()),
                });
            }
            return Ok(AttributionResult {
                project_id: Some(project_id),
                task_id: None,
                source: "workspace_mapping".into(),
                confidence: "medium".into(),
                unassigned_reason: None,
            });
        }
        return Ok(AttributionResult {
            project_id: None,
            task_id: None,
            source: "none".into(),
            confidence: "none".into(),
            unassigned_reason: Some("unknown_workspace".into()),
        });
    }

    Ok(AttributionResult {
        project_id: None,
        task_id: None,
        source: "none".into(),
        confidence: "none".into(),
        unassigned_reason: Some("insufficient_signal".into()),
    })
}

fn lookup_conversation(
    db: &Db,
    conversation_id: &str,
) -> AppResult<Option<(Option<String>, Option<String>)>> {
    db.with_conn(|conn| {
        let row = conn
            .query_row(
                "SELECT project_id, task_id FROM cursor_conversations WHERE conversation_id = ?1",
                [conversation_id],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        Ok(row)
    })
}

pub fn record_unassigned(
    db: &Db,
    event_id: &str,
    workspace_path: Option<&str>,
    conversation_id: Option<&str>,
    reason: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO unassigned_activity (id, event_id, workspace_path, conversation_id, reason, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
            params![
                Uuid::new_v4().to_string(),
                event_id,
                workspace_path,
                conversation_id,
                reason,
                now
            ],
        )?;
        Ok(())
    })
}

pub fn list_unassigned(db: &Db) -> AppResult<Vec<UnassignedItem>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, workspace_path, conversation_id, reason, created_at
             FROM unassigned_activity WHERE status = 'pending' ORDER BY created_at DESC LIMIT 200",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(UnassignedItem {
                id: r.get(0)?,
                workspace_path: r.get(1)?,
                conversation_id: r.get(2)?,
                reason: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
}

pub fn assign_unassigned(
    db: &Db,
    unassigned_id: &str,
    project_id: &str,
    task_id: &str,
) -> AppResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        let (workspace, conversation): (Option<String>, Option<String>) = conn.query_row(
            "SELECT workspace_path, conversation_id FROM unassigned_activity WHERE id = ?1",
            [unassigned_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if let Some(cid) = conversation.as_ref() {
            conn.execute(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, workspace_path, first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(conversation_id) DO UPDATE SET
                   project_id = excluded.project_id,
                   task_id = excluded.task_id,
                   workspace_path = COALESCE(excluded.workspace_path, cursor_conversations.workspace_path),
                   last_seen_at = excluded.last_seen_at",
                params![
                    Uuid::new_v4().to_string(),
                    cid,
                    project_id,
                    task_id,
                    workspace,
                    now
                ],
            )?;
        }
        conn.execute(
            "INSERT INTO task_attributions (id, target_type, target_id, project_id, task_id, source, confidence, created_at, created_by)
             VALUES (?1, 'unassigned_review', ?2, ?3, ?4, 'manual_review', 'high', ?5, 'user')",
            params![Uuid::new_v4().to_string(), unassigned_id, project_id, task_id, now],
        )?;
        conn.execute(
            "UPDATE unassigned_activity SET status = 'resolved', resolved_at = ?1,
             resolved_project_id = ?2, resolved_task_id = ?3 WHERE id = ?4",
            params![now, project_id, task_id, unassigned_id],
        )?;
        Ok(())
    })
}

pub fn unassigned_count(db: &Db) -> AppResult<i64> {
    db.with_conn(|conn| {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM unassigned_activity WHERE status = 'pending'",
            [],
            |r| r.get(0),
        )?;
        Ok(n)
    })
}
