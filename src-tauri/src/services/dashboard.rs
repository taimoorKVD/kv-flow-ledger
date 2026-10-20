use crate::db::Db;
use crate::error::AppResult;
use crate::finance::{compute_project_financials, LaborRate};
use crate::models::{ActivityEventDto, DashboardSummary, TrackingStatus};
use crate::services::{attribution, hooks, projects, settings};
use crate::time_engine::{compute_breakdown, IntervalKind, TimeInterval};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;

pub fn summary(db: &Db, project_id: Option<&str>, ingest_listening: bool) -> AppResult<DashboardSummary> {
    let rate = settings::labor_rate(db)?;
    let port = settings::ingest_port(db)?;
    let hook = hooks::status(db)?;

    let (pid, currency, budget) = if let Some(id) = project_id {
        let p = projects::get_project(db, id)?
            .ok_or_else(|| crate::error::AppError::Message("project not found".into()))?;
        (Some(p.id), p.currency, p.contracted_budget_minor)
    } else {
        let list = projects::list_projects(db)?;
        if let Some(p) = list.first() {
            (
                Some(p.id.clone()),
                p.currency.clone(),
                p.contracted_budget_minor,
            )
        } else {
            (None, rate.currency.clone(), 0)
        }
    };

    let work = project_work_ms(db, pid.as_deref())?;
    let labor = rate.labor_cost_for_ms(work.combined_ms).unwrap_or(0);
    let (ai_tokens, ai_tokens_available, ai_minor) = usage_totals(db, pid.as_deref())?;
    let other = other_expenses(db, pid.as_deref())?;
    let sub = subscription_allocation(db, pid.as_deref())?;
    let recognized = recognized_revenue(db, pid.as_deref())?;
    let payments = payments_received(db, pid.as_deref())?;
    let fin = compute_project_financials(
        &currency,
        budget,
        recognized,
        payments,
        labor,
        ai_minor,
        other,
        sub,
        work.combined_ms,
    );

    let active_projects: i64 = db.with_conn(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM projects WHERE status = 'active'",
            [],
            |r| r.get(0),
        )?)
    })?;
    let active_tasks: i64 = db.with_conn(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status IN ('in_progress','active','review')",
            [],
            |r| r.get(0),
        )?)
    })?;

    let (active_task_id, active_conversation_id, mode, last_event_at) = live_tracking(db)?;

    Ok(DashboardSummary {
        project_id: pid,
        task_work_ms: work.combined_ms,
        ai_tokens,
        ai_tokens_available,
        allocated_cost_minor: fin.total_cost_minor,
        margin_pct: fin.profit_margin_pct,
        labor_minor: labor,
        ai_minor,
        other_minor: other + sub,
        recognized_revenue_minor: recognized,
        profit_minor: fin.economic_profit_minor,
        currency,
        active_projects,
        active_tasks,
        unassigned_count: attribution::unassigned_count(db)?,
        tracking: TrackingStatus {
            hooks_installed: hook.installed,
            ingest_listening,
            ingest_port: port,
            active_project_id: project_id.map(|s| s.to_string()).or_else(|| {
                // fall through
                None
            }),
            active_task_id,
            active_conversation_id,
            mode,
            last_event_at,
            detail: hook.detail,
        },
    })
}

fn project_work_ms(
    db: &Db,
    project_id: Option<&str>,
) -> AppResult<crate::time_engine::TimeBreakdown> {
    db.with_conn(|conn| {
        let mut intervals = Vec::new();
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals
                 WHERE project_id = ?1 OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
            )?;
            let rows = stmt.query_map([pid], map_interval)?;
            for r in rows {
                intervals.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals",
            )?;
            let rows = stmt.query_map([], map_interval)?;
            for r in rows {
                intervals.push(r?);
            }
        }
        Ok(compute_breakdown(&intervals))
    })
}

fn map_interval(r: &rusqlite::Row<'_>) -> rusqlite::Result<TimeInterval> {
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
}

fn usage_totals(db: &Db, project_id: Option<&str>) -> AppResult<(Option<i64>, bool, i64)> {
    db.with_conn(|conn| {
        let (tokens, cost, nulls): (Option<i64>, i64, i64) = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT SUM(total_tokens), COALESCE(SUM(reported_cost_minor),0),
                        SUM(CASE WHEN total_tokens IS NULL THEN 1 ELSE 0 END)
                 FROM ai_usage_records
                 WHERE project_id = ?1 OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
                [pid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
        } else {
            conn.query_row(
                "SELECT SUM(total_tokens), COALESCE(SUM(reported_cost_minor),0),
                        SUM(CASE WHEN total_tokens IS NULL THEN 1 ELSE 0 END)
                 FROM ai_usage_records",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?
        };
        let count: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COUNT(*) FROM ai_usage_records
                 WHERE project_id = ?1 OR task_id IN (SELECT id FROM tasks WHERE project_id = ?1)",
                [pid],
                |r| r.get(0),
            )?
        } else {
            conn.query_row("SELECT COUNT(*) FROM ai_usage_records", [], |r| r.get(0))?
        };
        if count == 0 {
            return Ok((None, false, 0));
        }
        // If any records lack tokens and sum is null → unavailable
        if tokens.is_none() || nulls > 0 && tokens.unwrap_or(0) == 0 {
            // Still report partial sum if some rows have tokens
            if let Some(t) = tokens {
                return Ok((Some(t), true, cost));
            }
            return Ok((None, false, cost));
        }
        Ok((tokens, true, cost))
    })
}

fn other_expenses(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses
                 WHERE project_id = ?1 AND is_subscription_allocation = 0",
                [pid],
                |r| r.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses WHERE is_subscription_allocation = 0",
                [],
                |r| r.get(0),
            )?
        };
        Ok(v)
    })
}

fn subscription_allocation(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses
                 WHERE project_id = ?1 AND is_subscription_allocation = 1",
                [pid],
                |r| r.get(0),
            )?
        } else {
            0
        };
        Ok(v)
    })
}

fn recognized_revenue(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM revenue_records
                 WHERE project_id = ?1 AND kind = 'recognized'",
                [pid],
                |r| r.get(0),
            )?
        } else {
            0
        };
        Ok(v)
    })
}

fn payments_received(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.query_row(
                "SELECT COALESCE(SUM(amount_minor),0) FROM payments WHERE project_id = ?1",
                [pid],
                |r| r.get(0),
            )?
        } else {
            0
        };
        Ok(v)
    })
}

fn live_tracking(
    db: &Db,
) -> AppResult<(Option<String>, Option<String>, String, Option<String>)> {
    db.with_conn(|conn| {
        let last_event_at: Option<String> = conn
            .query_row(
                "SELECT occurred_at FROM cursor_events ORDER BY received_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let open_agent: bool = conn.query_row(
            "SELECT COUNT(*) FROM activity_intervals WHERE kind = 'agent' AND ended_at IS NULL",
            [],
            |r| r.get::<_, i64>(0),
        )? > 0;
        let open_dev: bool = conn.query_row(
            "SELECT COUNT(*) FROM activity_intervals WHERE kind = 'developer' AND ended_at IS NULL",
            [],
            |r| r.get::<_, i64>(0),
        )? > 0;
        let mode = if open_agent {
            "agent"
        } else if open_dev {
            "editor"
        } else {
            "idle"
        }
        .to_string();
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT task_id, conversation_id FROM activity_intervals
                 WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok((
            row.as_ref().and_then(|r| r.0.clone()),
            row.as_ref().and_then(|r| r.1.clone()),
            mode,
            last_event_at,
        ))
    })
}

pub fn recent_events(db: &Db, project_id: Option<&str>, limit: i64) -> AppResult<Vec<ActivityEventDto>> {
    db.with_conn(|conn| {
        let mut out = Vec::new();
        if let Some(pid) = project_id {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, occurred_at, workspace_path, conversation_id, project_id, task_id, model
                 FROM cursor_events WHERE project_id = ?1 ORDER BY occurred_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![pid, limit], map_event)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, event_type, occurred_at, workspace_path, conversation_id, project_id, task_id, model
                 FROM cursor_events ORDER BY occurred_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map([limit], map_event)?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    })
}

fn map_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityEventDto> {
    Ok(ActivityEventDto {
        id: r.get(0)?,
        event_type: r.get(1)?,
        occurred_at: r.get(2)?,
        workspace_path: r.get(3)?,
        conversation_id: r.get(4)?,
        project_id: r.get(5)?,
        task_id: r.get(6)?,
        model: r.get(7)?,
    })
}

pub fn project_financials_dto(
    db: &Db,
    project_id: &str,
) -> AppResult<crate::finance::ProjectFinancials> {
    let p = projects::get_project(db, project_id)?
        .ok_or_else(|| crate::error::AppError::Message("project not found".into()))?;
    let rate: LaborRate = settings::labor_rate(db)?;
    let work = project_work_ms(db, Some(project_id))?;
    let labor = rate.labor_cost_for_ms(work.combined_ms).unwrap_or(0);
    let (_, _, ai) = usage_totals(db, Some(project_id))?;
    let other = other_expenses(db, Some(project_id))?;
    let sub = subscription_allocation(db, Some(project_id))?;
    let recognized = recognized_revenue(db, Some(project_id))?;
    let payments = payments_received(db, Some(project_id))?;
    Ok(compute_project_financials(
        &p.currency,
        p.contracted_budget_minor,
        recognized,
        payments,
        labor,
        ai,
        other,
        sub,
        work.combined_ms,
    ))
}
