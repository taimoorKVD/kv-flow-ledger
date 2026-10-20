use crate::db::Db;
use crate::error::AppResult;
use crate::finance::{compute_project_financials, LaborRate};
use crate::models::{ActivityEventDto, DashboardSummary, TrackingStatus};
use crate::services::{attribution, hooks, projects, settings};
use crate::time_engine::{compute_breakdown, IntervalKind, TimeInterval};
use chrono::{DateTime, Utc};
use mysql::prelude::*;

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
    let labor = project_labor_cost(db, pid.as_deref(), &currency, work.combined_ms, &rate)?;
    let (ai_tokens, ai_tokens_available, ai_minor) =
        usage_totals(db, pid.as_deref(), &currency)?;
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
        Ok(c
            .query_first("SELECT COUNT(*) FROM projects WHERE status = 'active'")?
            .unwrap_or(0))
    })?;
    let active_tasks: i64 = db.with_conn(|c| {
        Ok(c
            .query_first(
                "SELECT COUNT(*) FROM tasks WHERE status IN ('in_progress','active','review')",
            )?
            .unwrap_or(0))
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
        let rows: Vec<(String, String, String)> = if let Some(pid) = project_id {
            conn.exec(
                "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals
                 WHERE project_id = ? OR task_id IN (SELECT id FROM tasks WHERE project_id = ?)",
                (pid, pid),
            )?
        } else {
            conn.query(
                "SELECT kind, started_at, COALESCE(ended_at, started_at) FROM activity_intervals",
            )?
        };
        let mut intervals = Vec::new();
        for (kind_s, start_s, end_s) in rows {
            intervals.push(map_interval(kind_s, start_s, end_s));
        }
        Ok(compute_breakdown(&intervals))
    })
}

fn map_interval(kind_s: String, start_s: String, end_s: String) -> TimeInterval {
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
    TimeInterval { kind, start, end }
}

fn usage_totals(
    db: &Db,
    project_id: Option<&str>,
    target_currency: &str,
) -> AppResult<(Option<i64>, bool, i64)> {
    let rows: Vec<(Option<i64>, Option<i64>, String)> = db.with_conn(|conn| {
        if let Some(pid) = project_id {
            Ok(conn.exec(
                "SELECT total_tokens, reported_cost_minor, currency FROM ai_usage_records
                 WHERE project_id = ? OR task_id IN (SELECT id FROM tasks WHERE project_id = ?)",
                (pid, pid),
            )?)
        } else {
            Ok(conn.query(
                "SELECT total_tokens, reported_cost_minor, currency FROM ai_usage_records",
            )?)
        }
    })?;
    if rows.is_empty() {
        return Ok((None, false, 0));
    }
    let mut token_sum: Option<i64> = None;
    let mut null_tokens = 0i64;
    let mut cost = 0i64;
    for (tok, c, cur) in rows {
        match tok {
            Some(t) => token_sum = Some(token_sum.unwrap_or(0) + t),
            None => null_tokens += 1,
        }
        if let Some(c) = c {
            cost += crate::services::currency::try_convert(db, c, &cur, target_currency)?.0;
        }
    }
    if token_sum.is_none() && null_tokens > 0 {
        return Ok((None, false, cost));
    }
    Ok((token_sum, token_sum.is_some(), cost))
}

fn other_expenses(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.exec_first(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses
                 WHERE project_id = ? AND is_subscription_allocation = 0",
                (pid,),
            )?
            .unwrap_or(0)
        } else {
            conn.query_first(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses WHERE is_subscription_allocation = 0",
            )?
            .unwrap_or(0)
        };
        Ok(v)
    })
}

fn subscription_allocation(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.exec_first(
                "SELECT COALESCE(SUM(amount_minor),0) FROM project_expenses
                 WHERE project_id = ? AND is_subscription_allocation = 1",
                (pid,),
            )?
            .unwrap_or(0)
        } else {
            0
        };
        Ok(v)
    })
}

fn recognized_revenue(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.exec_first(
                "SELECT COALESCE(SUM(amount_minor),0) FROM revenue_records
                 WHERE project_id = ? AND kind = 'recognized'",
                (pid,),
            )?
            .unwrap_or(0)
        } else {
            0
        };
        Ok(v)
    })
}

fn payments_received(db: &Db, project_id: Option<&str>) -> AppResult<i64> {
    db.with_conn(|conn| {
        let v: i64 = if let Some(pid) = project_id {
            conn.exec_first(
                "SELECT COALESCE(SUM(amount_minor),0) FROM payments WHERE project_id = ?",
                (pid,),
            )?
            .unwrap_or(0)
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
        let last_event_at: Option<String> = conn.query_first(
            "SELECT occurred_at FROM cursor_events ORDER BY received_at DESC LIMIT 1",
        )?;
        let open_agent: bool = conn
            .query_first::<i64, _>(
                "SELECT COUNT(*) FROM activity_intervals WHERE kind = 'agent' AND ended_at IS NULL",
            )?
            .unwrap_or(0)
            > 0;
        let open_dev: bool = conn
            .query_first::<i64, _>(
                "SELECT COUNT(*) FROM activity_intervals WHERE kind = 'developer' AND ended_at IS NULL",
            )?
            .unwrap_or(0)
            > 0;
        let mode = if open_agent {
            "agent"
        } else if open_dev {
            "editor"
        } else {
            "idle"
        }
        .to_string();
        let row: Option<(Option<String>, Option<String>)> = conn.query_first(
            "SELECT task_id, conversation_id FROM activity_intervals
             WHERE ended_at IS NULL ORDER BY started_at DESC LIMIT 1",
        )?;
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
        let rows: Vec<(
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = if let Some(pid) = project_id {
            conn.exec(
                "SELECT id, event_type, occurred_at, workspace_path, conversation_id, project_id, task_id, model
                 FROM cursor_events WHERE project_id = ? ORDER BY occurred_at DESC LIMIT ?",
                (pid, limit),
            )?
        } else {
            conn.exec(
                "SELECT id, event_type, occurred_at, workspace_path, conversation_id, project_id, task_id, model
                 FROM cursor_events ORDER BY occurred_at DESC LIMIT ?",
                (limit,),
            )?
        };
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    event_type,
                    occurred_at,
                    workspace_path,
                    conversation_id,
                    project_id,
                    task_id,
                    model,
                )| ActivityEventDto {
                    id,
                    event_type,
                    occurred_at,
                    workspace_path,
                    conversation_id,
                    project_id,
                    task_id,
                    model,
                },
            )
            .collect())
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
    let labor = project_labor_cost(db, Some(project_id), &p.currency, work.combined_ms, &rate)?;
    let (_, _, ai) = usage_totals(db, Some(project_id), &p.currency)?;
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

/// Prefer project hourly labor override (already in project currency); else global salary rate.
fn project_labor_cost(
    db: &Db,
    project_id: Option<&str>,
    project_currency: &str,
    work_ms: i64,
    global_rate: &LaborRate,
) -> AppResult<i64> {
    if let Some(pid) = project_id {
        if let Some(p) = projects::get_project(db, pid)? {
            if let Some(hourly) = p.labor_rate_override_minor.filter(|v| *v > 0) {
                let hours = (work_ms as f64) / 3_600_000.0;
                return Ok((hours * hourly as f64).round() as i64);
            }
        }
    }
    let labor_native = global_rate.labor_cost_for_ms(work_ms).unwrap_or(0);
    Ok(crate::services::currency::try_convert(
        db,
        labor_native,
        &global_rate.currency,
        project_currency,
    )?
    .0)
}
