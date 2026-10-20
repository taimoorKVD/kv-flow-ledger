use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{NewProject, NewTask, Project, ProjectFinanceUpdate, Task};
use crate::time_engine::{compute_breakdown, IntervalKind, TimeInterval};
use chrono::{DateTime, Utc};
use mysql::prelude::*;
use mysql::{PooledConn, TxOpts};
use uuid::Uuid;

pub fn list_projects(db: &Db) -> AppResult<Vec<Project>> {
    db.with_conn(|conn| {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
            i64,
            String,
            Option<i64>,
            String,
            Option<String>,
        )> = conn.query(
            "SELECT id, name, client, description, git_repository, status, billing_model,
                    contracted_budget_minor, currency, labor_rate_override_minor, created_at, archived_at
             FROM projects ORDER BY created_at DESC",
        )?;
        let mut projects = Vec::new();
        for (
            id,
            name,
            client,
            description,
            git_repository,
            status,
            billing_model,
            contracted_budget_minor,
            currency,
            labor_rate_override_minor,
            created_at,
            archived_at,
        ) in rows
        {
            let workspaces = workspace_paths(conn, &id)?;
            projects.push(Project {
                id,
                name,
                client,
                description,
                git_repository,
                status,
                billing_model,
                contracted_budget_minor,
                currency,
                labor_rate_override_minor,
                created_at,
                archived_at,
                workspaces,
            });
        }
        Ok(projects)
    })
}

fn workspace_paths(conn: &mut PooledConn, project_id: &str) -> AppResult<Vec<String>> {
    let paths: Vec<String> = conn.exec(
        "SELECT path FROM project_workspaces WHERE project_id = ? ORDER BY path",
        (project_id,),
    )?;
    Ok(paths)
}

pub fn create_project(db: &Db, input: NewProject) -> AppResult<Project> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let currency = crate::services::currency::normalize(&input.currency);
    db.with_conn_mut(|conn| {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        tx.exec_drop(
            "INSERT INTO projects (id, name, client, description, git_repository, status, billing_model,
             contracted_budget_minor, currency, created_at)
             VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, ?)",
            (
                &id,
                &input.name,
                &input.client,
                &input.description,
                &input.git_repository,
                &input.billing_model,
                input.contracted_budget_minor,
                &currency,
                &now,
            ),
        )?;
        for path in &input.workspaces {
            let normalized = normalize_path(path);
            tx.exec_drop(
                "INSERT INTO project_workspaces (id, project_id, path, created_at) VALUES (?, ?, ?, ?)",
                (Uuid::new_v4().to_string(), &id, normalized, &now),
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;
    get_project(db, &id)?.ok_or_else(|| AppError::Message("project missing after create".into()))
}

pub fn get_project(db: &Db, id: &str) -> AppResult<Option<Project>> {
    db.with_conn(|conn| {
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
            i64,
            String,
            Option<i64>,
            String,
            Option<String>,
        )> = conn.exec_first(
            "SELECT id, name, client, description, git_repository, status, billing_model,
                    contracted_budget_minor, currency, labor_rate_override_minor, created_at, archived_at
             FROM projects WHERE id = ?",
            (id,),
        )?;
        Ok(match row {
            Some((
                id,
                name,
                client,
                description,
                git_repository,
                status,
                billing_model,
                contracted_budget_minor,
                currency,
                labor_rate_override_minor,
                created_at,
                archived_at,
            )) => {
                let workspaces = workspace_paths(conn, &id)?;
                Some(Project {
                    id,
                    name,
                    client,
                    description,
                    git_repository,
                    status,
                    billing_model,
                    contracted_budget_minor,
                    currency,
                    labor_rate_override_minor,
                    created_at,
                    archived_at,
                    workspaces,
                })
            }
            None => None,
        })
    })
}

pub fn find_project_by_workspace(db: &Db, workspace: &str) -> AppResult<Option<String>> {
    let normalized = normalize_path(workspace);
    db.with_conn(|conn| {
        let id: Option<String> = conn.exec_first(
            "SELECT project_id FROM project_workspaces WHERE LOWER(path) = LOWER(?)",
            (&normalized,),
        )?;
        Ok(id)
    })
}

pub fn list_tasks(db: &Db, project_id: Option<&str>) -> AppResult<Vec<Task>> {
    db.with_conn(|conn| {
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            String,
        )> = if let Some(pid) = project_id {
            conn.exec(
                "SELECT id, project_id, title, description, status, github_issue, completed_at, created_at
                 FROM tasks WHERE project_id = ? ORDER BY created_at DESC",
                (pid,),
            )?
        } else {
            conn.query(
                "SELECT id, project_id, title, description, status, github_issue, completed_at, created_at
                 FROM tasks ORDER BY created_at DESC",
            )?
        };
        let mut tasks = Vec::new();
        for (
            id,
            project_id,
            title,
            description,
            status,
            github_issue,
            completed_at,
            created_at,
        ) in rows
        {
            let mut task = Task {
                id,
                project_id,
                title,
                description,
                status,
                github_issue,
                completed_at,
                created_at,
                combined_work_ms: 0,
                agent_ms: 0,
                developer_ms: 0,
            };
            task = enrich_task(conn, task)?;
            tasks.push(task);
        }
        Ok(tasks)
    })
}

fn enrich_task(conn: &mut PooledConn, mut task: Task) -> AppResult<Task> {
    let breakdown = task_time_breakdown(conn, &task.id)?;
    task.combined_work_ms = breakdown.combined_ms;
    task.agent_ms = breakdown.agent_ms;
    task.developer_ms = breakdown.developer_ms;
    Ok(task)
}

pub fn task_time_breakdown(
    conn: &mut PooledConn,
    task_id: &str,
) -> AppResult<crate::time_engine::TimeBreakdown> {
    let rows: Vec<(String, String, String)> = conn.exec(
        "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals
         WHERE task_id = ? AND kind IN ('developer', 'agent', 'idle_excluded', 'unattributed')",
        (task_id,),
    )?;
    let mut intervals = Vec::new();
    for (kind_s, start_s, end_s) in rows {
        let kind = match kind_s.as_str() {
            "developer" => IntervalKind::Developer,
            "agent" => IntervalKind::Agent,
            "idle_excluded" => IntervalKind::IdleExcluded,
            _ => IntervalKind::Unattributed,
        };
        let start = DateTime::parse_from_rfc3339(&start_s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let end = DateTime::parse_from_rfc3339(&end_s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(start);
        intervals.push(TimeInterval { kind, start, end });
    }
    Ok(compute_breakdown(&intervals))
}

pub fn create_task(db: &Db, input: NewTask) -> AppResult<Task> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.exec_drop(
            "INSERT INTO tasks (id, project_id, title, description, status, created_at)
             VALUES (?, ?, ?, ?, 'in_progress', ?)",
            (
                &id,
                &input.project_id,
                &input.title,
                &input.description,
                &now,
            ),
        )?;
        if let Some(cid) = input.conversation_id.as_ref().filter(|s| !s.is_empty()) {
            conn.exec_drop(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, first_seen_at, last_seen_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON DUPLICATE KEY UPDATE
                   project_id = VALUES(project_id),
                   task_id = VALUES(task_id),
                   last_seen_at = VALUES(last_seen_at)",
                (
                    Uuid::new_v4().to_string(),
                    cid,
                    &input.project_id,
                    &id,
                    &now,
                    &now,
                ),
            )?;
            conn.exec_drop(
                "INSERT INTO task_attributions (id, target_type, target_id, project_id, task_id, source, confidence, created_at)
                 VALUES (?, 'conversation', ?, ?, ?, 'explicit', 'high', ?)",
                (
                    Uuid::new_v4().to_string(),
                    cid,
                    &input.project_id,
                    &id,
                    &now,
                ),
            )?;
        }
        Ok(())
    })?;
    list_tasks(db, Some(&input.project_id))?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::Message("task missing".into()))
}

/// Idempotent: return existing task for this conversation, or create + link one.
pub fn ensure_task_for_conversation(
    db: &Db,
    project_id: &str,
    conversation_id: &str,
) -> AppResult<(String, String)> {
    let existing: Option<(Option<String>, Option<String>)> = db.with_conn(|conn| {
        let row = conn.exec_first(
            "SELECT project_id, task_id FROM cursor_conversations WHERE conversation_id = ?",
            (conversation_id,),
        )?;
        Ok(row)
    })?;
    if let Some((pid, Some(tid))) = existing {
        let pid = pid.unwrap_or_else(|| project_id.to_string());
        return Ok((pid, tid));
    }

    let short: String = conversation_id.chars().take(8).collect();
    let title = format!("Chat · {short}");
    let task = create_task(
        db,
        NewTask {
            project_id: project_id.to_string(),
            title,
            description: None,
            conversation_id: Some(conversation_id.to_string()),
        },
    )?;

    // Attribution source for auto path (create_task wrote 'explicit'; annotate auto).
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.exec_drop(
            "UPDATE task_attributions SET source = 'auto_conversation'
             WHERE target_type = 'conversation' AND target_id = ? AND task_id = ?",
            (conversation_id, &task.id),
        )?;
        // Touch conversation timestamps if row already existed without task_id
        conn.exec_drop(
            "UPDATE cursor_conversations
             SET project_id = ?, task_id = ?, last_seen_at = ?
             WHERE conversation_id = ?",
            (project_id, &task.id, &now, conversation_id),
        )?;
        Ok(())
    })?;

    Ok((project_id.to_string(), task.id))
}

pub fn update_task_title(db: &Db, task_id: &str, title: &str) -> AppResult<Task> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Message("task title required".into()));
    }
    db.with_conn(|conn| {
        let exists: Option<String> =
            conn.exec_first("SELECT id FROM tasks WHERE id = ?", (task_id,))?;
        if exists.is_none() {
            return Err(AppError::Message("task not found".into()));
        }
        conn.exec_drop("UPDATE tasks SET title = ? WHERE id = ?", (title, task_id))?;
        Ok(())
    })?;
    list_tasks(db, None)?
        .into_iter()
        .find(|t| t.id == task_id)
        .ok_or_else(|| AppError::Message("task missing".into()))
}

pub fn update_project_finances(
    db: &Db,
    project_id: &str,
    update: ProjectFinanceUpdate,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let currency = crate::services::currency::normalize(&update.currency);
        tx.exec_drop(
            "UPDATE projects SET contracted_budget_minor = ?, currency = ? WHERE id = ?",
            (update.contracted_budget_minor, &currency, project_id),
        )?;
        tx.exec_drop(
            "DELETE FROM revenue_records WHERE project_id = ? AND kind = 'recognized'",
            (project_id,),
        )?;
        tx.exec_drop(
            "INSERT INTO revenue_records (id, project_id, kind, amount_minor, currency, recognized_at, notes)
             VALUES (?, ?, 'recognized', ?, COALESCE((SELECT currency FROM projects WHERE id = ?), 'USD'), ?, 'manual')",
            (
                Uuid::new_v4().to_string(),
                project_id,
                update.recognized_revenue_minor,
                project_id,
                &now,
            ),
        )?;
        tx.exec_drop("DELETE FROM payments WHERE project_id = ?", (project_id,))?;
        if update.payments_received_minor > 0 {
            tx.exec_drop(
                "INSERT INTO payments (id, project_id, amount_minor, currency, received_at, notes)
                 VALUES (?, ?, ?, COALESCE((SELECT currency FROM projects WHERE id = ?), 'USD'), ?, 'manual')",
                (
                    Uuid::new_v4().to_string(),
                    project_id,
                    update.payments_received_minor,
                    project_id,
                    &now,
                ),
            )?;
        }
        tx.exec_drop(
            "DELETE FROM project_expenses WHERE project_id = ? AND category = 'other' AND notes = 'rollup'",
            (project_id,),
        )?;
        if update.other_expenses_minor > 0 {
            tx.exec_drop(
                "INSERT INTO project_expenses (id, project_id, category, amount_minor, currency, occurred_at, notes)
                 VALUES (?, ?, 'other', ?, COALESCE((SELECT currency FROM projects WHERE id = ?), 'USD'), ?, 'rollup')",
                (
                    Uuid::new_v4().to_string(),
                    project_id,
                    update.other_expenses_minor,
                    project_id,
                    &now,
                ),
            )?;
        }
        tx.commit()?;
        Ok(())
    })
}

pub fn normalize_path(path: &str) -> String {
    path.replace('/', "\\").trim_end_matches('\\').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test_db;

    #[test]
    fn create_and_list_project_with_workspace() {
        let db = open_test_db().unwrap();
        let p = create_project(
            &db,
            NewProject {
                name: "ABC".into(),
                client: Some("Client".into()),
                description: None,
                git_repository: None,
                billing_model: "fixed_price".into(),
                contracted_budget_minor: 100_000,
                currency: "USD".into(),
                workspaces: vec!["D:\\work\\abc".into()],
            },
        )
        .unwrap();
        assert_eq!(p.name, "ABC");
        let found = find_project_by_workspace(&db, "D:\\work\\abc").unwrap();
        assert_eq!(found.as_deref(), Some(p.id.as_str()));
        let task = create_task(
            &db,
            NewTask {
                project_id: p.id.clone(),
                title: "Implement".into(),
                description: None,
                conversation_id: Some("conv-1".into()),
            },
        )
        .unwrap();
        assert_eq!(task.title, "Implement");
    }
}
