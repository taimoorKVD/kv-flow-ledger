use crate::db::Db;
use crate::error::AppResult;
use crate::models::UnassignedItem;
use crate::services::projects;
use mysql::prelude::*;
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
        let row: Option<(Option<String>, Option<String>)> = conn.exec_first(
            "SELECT project_id, task_id FROM cursor_conversations WHERE conversation_id = ?",
            (conversation_id,),
        )?;
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
        conn.exec_drop(
            "INSERT INTO unassigned_activity (id, event_id, workspace_path, conversation_id, reason, status, created_at)
             VALUES (?, ?, ?, ?, ?, 'pending', ?)",
            (
                Uuid::new_v4().to_string(),
                event_id,
                workspace_path,
                conversation_id,
                reason,
                now,
            ),
        )?;
        Ok(())
    })
}

pub fn list_unassigned(db: &Db) -> AppResult<Vec<UnassignedItem>> {
    db.with_conn(|conn| {
        let rows: Vec<(
            String,
            Option<String>,
            Option<String>,
            String,
            String,
        )> = conn.query(
            "SELECT id, workspace_path, conversation_id, reason, created_at
             FROM unassigned_activity WHERE status = 'pending' ORDER BY created_at DESC LIMIT 200",
        )?;
        Ok(rows
            .into_iter()
            .map(
                |(id, workspace_path, conversation_id, reason, created_at)| UnassignedItem {
                    id,
                    workspace_path,
                    conversation_id,
                    reason,
                    created_at,
                },
            )
            .collect())
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
        let row: Option<(Option<String>, Option<String>)> = conn.exec_first(
            "SELECT workspace_path, conversation_id FROM unassigned_activity WHERE id = ?",
            (unassigned_id,),
        )?;
        let (workspace, conversation) = row.unwrap_or((None, None));
        if let Some(cid) = conversation.as_ref() {
            conn.exec_drop(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, workspace_path, first_seen_at, last_seen_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON DUPLICATE KEY UPDATE
                   project_id = VALUES(project_id),
                   task_id = VALUES(task_id),
                   workspace_path = COALESCE(VALUES(workspace_path), workspace_path),
                   last_seen_at = VALUES(last_seen_at)",
                (
                    Uuid::new_v4().to_string(),
                    cid,
                    project_id,
                    task_id,
                    &workspace,
                    &now,
                    &now,
                ),
            )?;
        }
        conn.exec_drop(
            "INSERT INTO task_attributions (id, target_type, target_id, project_id, task_id, source, confidence, created_at, created_by)
             VALUES (?, 'unassigned_review', ?, ?, ?, 'manual_review', 'high', ?, 'user')",
            (
                Uuid::new_v4().to_string(),
                unassigned_id,
                project_id,
                task_id,
                &now,
            ),
        )?;
        conn.exec_drop(
            "UPDATE unassigned_activity SET status = 'resolved', resolved_at = ?,
             resolved_project_id = ?, resolved_task_id = ? WHERE id = ?",
            (&now, project_id, task_id, unassigned_id),
        )?;
        Ok(())
    })
}

pub fn unassigned_count(db: &Db) -> AppResult<i64> {
    db.with_conn(|conn| {
        let n: i64 = conn
            .query_first("SELECT COUNT(*) FROM unassigned_activity WHERE status = 'pending'")?
            .unwrap_or(0);
        Ok(n)
    })
}
