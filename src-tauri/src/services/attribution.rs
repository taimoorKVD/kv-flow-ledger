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
/// 1. Conversation→task mapping (only when project/task already linked)
/// 2. Workspace→project (task filled later by ingest: chat task or General)
/// 3. Unknown workspace → unassigned; no signal → ignore (no queue)
pub fn resolve(
    db: &Db,
    conversation_id: Option<&str>,
    workspace_path: Option<&str>,
) -> AppResult<AttributionResult> {
    if let Some(cid) = conversation_id {
        if let Some((project_id, task_id)) = lookup_conversation(db, cid)? {
            // Empty rows must not block workspace fallback.
            if project_id.is_some() || task_id.is_some() {
                return Ok(AttributionResult {
                    project_id,
                    task_id,
                    source: "conversation_mapping".into(),
                    confidence: "high".into(),
                    unassigned_reason: None,
                });
            }
        }
    }

    if let Some(ws) = workspace_path {
        if let Some(project_id) = projects::find_project_by_workspace(db, ws)? {
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

    // No workspace and no usable conversation — do not queue for review.
    Ok(AttributionResult {
        project_id: None,
        task_id: None,
        source: "none".into(),
        confidence: "none".into(),
        unassigned_reason: None,
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
    let page = list_unassigned_page(
        db,
        &crate::models::UnassignedQuery {
            page: Some(1),
            page_size: Some(200),
            ..Default::default()
        },
    )?;
    Ok(page.items)
}

pub fn list_unassigned_page(
    db: &Db,
    query: &crate::models::UnassignedQuery,
) -> AppResult<crate::models::UnassignedPage> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(5, 100);
    let offset = ((page - 1) * page_size) as i64;
    let reason_filter = query
        .reason
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "all");
    let search = query
        .search
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let conversation = query
        .conversation
        .as_deref()
        .unwrap_or("all")
        .to_ascii_lowercase();
    let project_id = query
        .project_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    db.with_conn(|conn| {
        let mut where_sql = String::from("WHERE status = 'pending'");
        let mut params_vec: Vec<mysql::Value> = Vec::new();

        if let Some(pid) = &project_id {
            // Workspace mapped to this project, or conversation already linked to it.
            let ws_paths: Vec<String> = conn.exec_map(
                "SELECT path FROM project_workspaces WHERE project_id = ?",
                (pid,),
                |p: String| p,
            )?;
            let mut variants: Vec<String> = Vec::new();
            for path in &ws_paths {
                variants.extend(workspace_match_variants(path));
            }
            variants.sort();
            variants.dedup();

            where_sql.push_str(" AND (");
            where_sql.push_str(
                "EXISTS (
                   SELECT 1 FROM cursor_conversations cc
                   WHERE cc.project_id = ?
                     AND cc.conversation_id = unassigned_activity.conversation_id
                 )",
            );
            params_vec.push(pid.clone().into());

            if !variants.is_empty() {
                where_sql.push_str(" OR LOWER(IFNULL(workspace_path,'')) IN (");
                for (i, _) in variants.iter().enumerate() {
                    if i > 0 {
                        where_sql.push(',');
                    }
                    where_sql.push('?');
                }
                where_sql.push(')');
                for v in &variants {
                    params_vec.push(v.to_ascii_lowercase().into());
                }
            }
            where_sql.push(')');
        }

        if let Some(reason) = &reason_filter {
            where_sql.push_str(" AND reason = ?");
            params_vec.push(reason.clone().into());
        }
        if let Some(q) = &search {
            where_sql.push_str(
                " AND (IFNULL(workspace_path,'') LIKE ? OR IFNULL(conversation_id,'') LIKE ? OR reason LIKE ?)",
            );
            let like = format!("%{q}%");
            params_vec.push(like.clone().into());
            params_vec.push(like.clone().into());
            params_vec.push(like.into());
        }
        match conversation.as_str() {
            "with_conversation" => {
                where_sql.push_str(
                    " AND conversation_id IS NOT NULL AND TRIM(conversation_id) <> ''",
                );
            }
            "without_conversation" => {
                where_sql.push_str(
                    " AND (conversation_id IS NULL OR TRIM(conversation_id) = '')",
                );
            }
            _ => {}
        }

        let count_sql = format!("SELECT COUNT(*) FROM unassigned_activity {where_sql}");
        let total: i64 = conn
            .exec_first(count_sql, params_vec.clone())?
            .unwrap_or(0);

        let list_sql = format!(
            "SELECT id, workspace_path, conversation_id, reason, created_at
             FROM unassigned_activity {where_sql}
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?"
        );
        let mut list_params = params_vec.clone();
        list_params.push((page_size as i64).into());
        list_params.push(offset.into());

        let rows: Vec<(
            String,
            Option<String>,
            Option<String>,
            String,
            String,
        )> = conn.exec(list_sql, list_params)?;

        let reasons: Vec<String> = if project_id.is_some() {
            let reasons_sql =
                format!("SELECT DISTINCT reason FROM unassigned_activity {where_sql} ORDER BY reason");
            conn.exec_map(reasons_sql, params_vec, |reason: String| reason)?
        } else {
            conn.query_map(
                "SELECT DISTINCT reason FROM unassigned_activity WHERE status = 'pending' ORDER BY reason",
                |reason: String| reason,
            )?
        };

        Ok(crate::models::UnassignedPage {
            items: rows
                .into_iter()
                .map(
                    |(id, workspace_path, conversation_id, reason, created_at)| {
                        crate::models::UnassignedItem {
                            id,
                            workspace_path,
                            conversation_id,
                            reason,
                            created_at,
                        }
                    },
                )
                .collect(),
            total,
            page,
            page_size,
            reasons,
        })
    })
}

fn workspace_match_variants(path: &str) -> Vec<String> {
    let normalized = crate::services::projects::normalize_path(path);
    let mut out = vec![
        normalized.clone(),
        normalized.to_ascii_lowercase(),
        path.to_string(),
        path.to_ascii_lowercase(),
    ];
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        let drive = normalized.chars().next().unwrap().to_ascii_lowercase();
        let rest = normalized[2..].replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        out.push(format!("/{drive}:/{rest}"));
        out.push(format!("/{drive}:{rest}"));
        out.push(format!("{}:/{}", drive, rest));
    }
    out
}

/// Drop noisy pending rows that have no conversation and insufficient_signal (legacy empty hooks).
pub fn dismiss_noise_unassigned(db: &Db) -> AppResult<i64> {
    db.with_conn(|conn| {
        conn.exec_drop(
            "UPDATE unassigned_activity
             SET status = 'dismissed', resolved_at = ?
             WHERE status = 'pending'
               AND reason = 'insufficient_signal'
               AND (conversation_id IS NULL OR TRIM(conversation_id) = '')",
            (chrono::Utc::now().to_rfc3339(),),
        )?;
        let n = conn.affected_rows() as i64;
        Ok(n)
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
