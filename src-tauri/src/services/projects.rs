use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{NewProject, NewTask, Project, ProjectFinanceUpdate, Task};
use crate::time_engine::{compute_breakdown, IntervalKind, TimeInterval};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

pub fn list_projects(db: &Db) -> AppResult<Vec<Project>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, client, description, git_repository, status, billing_model,
                    contracted_budget_minor, currency, labor_rate_override_minor, created_at, archived_at
             FROM projects ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                client: r.get(2)?,
                description: r.get(3)?,
                git_repository: r.get(4)?,
                status: r.get(5)?,
                billing_model: r.get(6)?,
                contracted_budget_minor: r.get(7)?,
                currency: r.get(8)?,
                labor_rate_override_minor: r.get(9)?,
                created_at: r.get(10)?,
                archived_at: r.get(11)?,
                workspaces: vec![],
            })
        })?;
        let mut projects = Vec::new();
        for row in rows {
            let mut p = row?;
            p.workspaces = workspace_paths(conn, &p.id)?;
            projects.push(p);
        }
        Ok(projects)
    })
}

fn workspace_paths(conn: &rusqlite::Connection, project_id: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT path FROM project_workspaces WHERE project_id = ?1 ORDER BY path")?;
    let rows = stmt.query_map([project_id], |r| r.get(0))?;
    rows.collect()
}

pub fn create_project(db: &Db, input: NewProject) -> AppResult<Project> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO projects (id, name, client, description, git_repository, status, billing_model,
             contracted_budget_minor, currency, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, ?7, ?8, ?9)",
            params![
                id,
                input.name,
                input.client,
                input.description,
                input.git_repository,
                input.billing_model,
                input.contracted_budget_minor,
                input.currency,
                now
            ],
        )?;
        for path in &input.workspaces {
            let normalized = normalize_path(path);
            tx.execute(
                "INSERT INTO project_workspaces (id, project_id, path, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![Uuid::new_v4().to_string(), id, normalized, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;
    get_project(db, &id)?.ok_or_else(|| AppError::Message("project missing after create".into()))
}

pub fn get_project(db: &Db, id: &str) -> AppResult<Option<Project>> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, client, description, git_repository, status, billing_model,
                    contracted_budget_minor, currency, labor_rate_override_minor, created_at, archived_at
             FROM projects WHERE id = ?1",
        )?;
        let project = stmt
            .query_row([id], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    client: r.get(2)?,
                    description: r.get(3)?,
                    git_repository: r.get(4)?,
                    status: r.get(5)?,
                    billing_model: r.get(6)?,
                    contracted_budget_minor: r.get(7)?,
                    currency: r.get(8)?,
                    labor_rate_override_minor: r.get(9)?,
                    created_at: r.get(10)?,
                    archived_at: r.get(11)?,
                    workspaces: vec![],
                })
            })
            .optional()?;
        Ok(match project {
            Some(mut p) => {
                p.workspaces = workspace_paths(conn, &p.id)?;
                Some(p)
            }
            None => None,
        })
    })
}

pub fn find_project_by_workspace(db: &Db, workspace: &str) -> AppResult<Option<String>> {
    let normalized = normalize_path(workspace);
    db.with_conn(|conn| {
        let id = conn
            .query_row(
                "SELECT project_id FROM project_workspaces WHERE lower(path) = lower(?1)",
                [&normalized],
                |r| r.get(0),
            )
            .optional()?;
        Ok(id)
    })
}

pub fn list_tasks(db: &Db, project_id: Option<&str>) -> AppResult<Vec<Task>> {
    db.with_conn(|conn| {
        let mut tasks = Vec::new();
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, title, description, status, github_issue, completed_at, created_at
                 FROM tasks WHERE project_id = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([pid], map_task_row)?;
            for row in rows {
                tasks.push(enrich_task(conn, row?)?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, title, description, status, github_issue, completed_at, created_at
                 FROM tasks ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], map_task_row)?;
            for row in rows {
                tasks.push(enrich_task(conn, row?)?);
            }
        }
        Ok(tasks)
    })
}

fn map_task_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        project_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        status: r.get(4)?,
        github_issue: r.get(5)?,
        completed_at: r.get(6)?,
        created_at: r.get(7)?,
        combined_work_ms: 0,
        agent_ms: 0,
        developer_ms: 0,
    })
}

fn enrich_task(conn: &rusqlite::Connection, mut task: Task) -> rusqlite::Result<Task> {
    let breakdown = task_time_breakdown(conn, &task.id)?;
    task.combined_work_ms = breakdown.combined_ms;
    task.agent_ms = breakdown.agent_ms;
    task.developer_ms = breakdown.developer_ms;
    Ok(task)
}

pub fn task_time_breakdown(
    conn: &rusqlite::Connection,
    task_id: &str,
) -> rusqlite::Result<crate::time_engine::TimeBreakdown> {
    let mut stmt = conn.prepare(
        "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals
         WHERE task_id = ?1 AND kind IN ('developer', 'agent', 'idle_excluded', 'unattributed')",
    )?;
    let rows = stmt.query_map([task_id], |r| {
        let kind_s: String = r.get(0)?;
        let start_s: String = r.get(1)?;
        let end_s: String = r.get(2)?;
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
        Ok(TimeInterval { kind, start, end })
    })?;
    let mut intervals = Vec::new();
    for row in rows {
        intervals.push(row?);
    }
    Ok(compute_breakdown(&intervals))
}

pub fn create_task(db: &Db, input: NewTask) -> AppResult<Task> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO tasks (id, project_id, title, description, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'in_progress', ?5)",
            params![id, input.project_id, input.title, input.description, now],
        )?;
        if let Some(cid) = input.conversation_id.as_ref().filter(|s| !s.is_empty()) {
            conn.execute(
                "INSERT INTO cursor_conversations (id, conversation_id, project_id, task_id, first_seen_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(conversation_id) DO UPDATE SET
                   project_id = excluded.project_id,
                   task_id = excluded.task_id,
                   last_seen_at = excluded.last_seen_at",
                params![Uuid::new_v4().to_string(), cid, input.project_id, id, now],
            )?;
            conn.execute(
                "INSERT INTO task_attributions (id, target_type, target_id, project_id, task_id, source, confidence, created_at)
                 VALUES (?1, 'conversation', ?2, ?3, ?4, 'explicit', 'high', ?5)",
                params![Uuid::new_v4().to_string(), cid, input.project_id, id, now],
            )?;
        }
        Ok(())
    })?;
    list_tasks(db, Some(&input.project_id))?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::Message("task missing".into()))
}

pub fn update_project_finances(db: &Db, project_id: &str, update: ProjectFinanceUpdate) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE projects SET contracted_budget_minor = ?1 WHERE id = ?2",
            params![update.contracted_budget_minor, project_id],
        )?;
        tx.execute("DELETE FROM revenue_records WHERE project_id = ?1 AND kind = 'recognized'", [project_id])?;
        tx.execute(
            "INSERT INTO revenue_records (id, project_id, kind, amount_minor, currency, recognized_at, notes)
             VALUES (?1, ?2, 'recognized', ?3, COALESCE((SELECT currency FROM projects WHERE id = ?2), 'USD'), ?4, 'manual')",
            params![Uuid::new_v4().to_string(), project_id, update.recognized_revenue_minor, now],
        )?;
        tx.execute("DELETE FROM payments WHERE project_id = ?1", [project_id])?;
        if update.payments_received_minor > 0 {
            tx.execute(
                "INSERT INTO payments (id, project_id, amount_minor, currency, received_at, notes)
                 VALUES (?1, ?2, ?3, COALESCE((SELECT currency FROM projects WHERE id = ?2), 'USD'), ?4, 'manual')",
                params![Uuid::new_v4().to_string(), project_id, update.payments_received_minor, now],
            )?;
        }
        tx.execute(
            "DELETE FROM project_expenses WHERE project_id = ?1 AND category = 'other' AND notes = 'rollup'",
            [project_id],
        )?;
        if update.other_expenses_minor > 0 {
            tx.execute(
                "INSERT INTO project_expenses (id, project_id, category, amount_minor, currency, occurred_at, notes)
                 VALUES (?1, ?2, 'other', ?3, COALESCE((SELECT currency FROM projects WHERE id = ?2), 'USD'), ?4, 'rollup')",
                params![Uuid::new_v4().to_string(), project_id, update.other_expenses_minor, now],
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
    use crate::db::Db;
    use tempfile::tempdir;

    #[test]
    fn create_and_list_project_with_workspace() {
        let dir = tempdir().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
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
