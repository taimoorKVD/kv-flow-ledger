//! Sync verified usage + cost from Cursor Admin API into local `ai_usage_records`.
//!
//! Primary source: `POST https://api.cursor.com/teams/filtered-usage-events`
//! (Teams / Enterprise Admin API key). Events include `chargedCents`, token
//! breakdown, and `conversationId` for project attribution via hook-tracked chats.

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::services::settings;
use base64::Engine;
use mysql::prelude::*;
use mysql::params;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const USAGE_EVENTS_URL: &str = "https://api.cursor.com/teams/filtered-usage-events";

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub fetched: i64,
    pub inserted: i64,
    pub updated: i64,
    pub attributed: i64,
    pub pages: u32,
    pub detail: String,
}

#[derive(Debug, Deserialize)]
struct UsageEventsResponse {
    #[serde(default, rename = "totalUsageEventsCount")]
    total_usage_events_count: i64,
    #[serde(default)]
    pagination: Option<Pagination>,
    #[serde(default, rename = "usageEvents")]
    usage_events: Vec<UsageEvent>,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    #[serde(default, rename = "numPages")]
    num_pages: u32,
    #[serde(default, rename = "hasNextPage")]
    has_next_page: bool,
}

#[derive(Debug, Deserialize)]
struct UsageEvent {
    timestamp: Option<serde_json::Value>,
    #[serde(default, rename = "conversationId")]
    conversation_id: Option<String>,
    model: Option<String>,
    kind: Option<String>,
    #[serde(default, rename = "isChargeable")]
    is_chargeable: Option<bool>,
    #[serde(default, rename = "chargedCents")]
    charged_cents: Option<f64>,
    #[serde(default, rename = "tokenUsage")]
    token_usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct TokenUsage {
    #[serde(default, rename = "inputTokens")]
    input_tokens: Option<i64>,
    #[serde(default, rename = "outputTokens")]
    output_tokens: Option<i64>,
    #[serde(default, rename = "cacheWriteTokens")]
    cache_write_tokens: Option<i64>,
    #[serde(default, rename = "cacheReadTokens")]
    cache_read_tokens: Option<i64>,
    #[serde(default, rename = "totalCents")]
    total_cents: Option<f64>,
}

/// Pull Cursor usage events for the last `days` days and upsert into the local ledger.
/// When `default_project_id` is set, events without a known conversation mapping are
/// attributed to that project (so AI allocation is project-wise even before a chat link exists).
pub fn sync_usage(
    db: &Db,
    days: i64,
    default_project_id: Option<&str>,
) -> AppResult<SyncResult> {
    let api_key = settings::get(db, "cursor_api_key")?
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err(AppError::Message(
            "Add a Cursor Admin API key in Settings to sync usage and costs.".into(),
        ));
    }

    let days = days.clamp(1, 90);
    let end_ms = chrono::Utc::now().timestamp_millis();
    let start_ms = end_ms - days * 24 * 60 * 60 * 1000;

    let mut fetched = 0i64;
    let mut inserted = 0i64;
    let mut updated = 0i64;
    let mut attributed = 0i64;
    let mut page: u32 = 1;
    let mut pages_done = 0u32;

    loop {
        let body = json!({
            "startDate": start_ms,
            "endDate": end_ms,
            "page": page,
            "pageSize": 1000
        });
        let resp = fetch_page(&api_key, &body)?;
        pages_done += 1;
        fetched += resp.usage_events.len() as i64;

        for ev in &resp.usage_events {
            let outcome = upsert_event(db, ev, default_project_id)?;
            match outcome {
                UpsertOutcome::Inserted { assigned } => {
                    inserted += 1;
                    if assigned {
                        attributed += 1;
                    }
                }
                UpsertOutcome::Updated { assigned } => {
                    updated += 1;
                    if assigned {
                        attributed += 1;
                    }
                }
                UpsertOutcome::Skipped => {}
            }
        }

        let has_next = resp
            .pagination
            .as_ref()
            .map(|p| p.has_next_page)
            .unwrap_or(false);
        let num_pages = resp.pagination.as_ref().map(|p| p.num_pages).unwrap_or(1);
        if !has_next || page >= num_pages || page >= 50 {
            break;
        }
        page += 1;
    }

    let now = chrono::Utc::now().to_rfc3339();
    settings::set(db, "cursor_usage_last_sync_at", &now)?;
    settings::set(
        db,
        "cursor_usage_last_sync_detail",
        &format!(
            "Synced {fetched} events ({inserted} new, {updated} updated, {attributed} attributed) over {days}d"
        ),
    )?;

    Ok(SyncResult {
        fetched,
        inserted,
        updated,
        attributed,
        pages: pages_done,
        detail: format!(
            "Cursor API: {fetched} events from last {days} day(s). {inserted} inserted, {updated} updated."
        ),
    })
}

fn fetch_page(api_key: &str, body: &serde_json::Value) -> AppResult<UsageEventsResponse> {
    let auth = base64::engine::general_purpose::STANDARD.encode(format!("{api_key}:"));
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(60))
        .build();

    let response = agent
        .post(USAGE_EVENTS_URL)
        .set("Authorization", &format!("Basic {auth}"))
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(body.clone())
        .map_err(|e| map_http_err(e))?;

    let status = response.status();
    let text = response
        .into_string()
        .map_err(|e| AppError::Message(format!("Cursor API read failed: {e}")))?;

    if status == 401 {
        return Err(AppError::Message(
            "Cursor API key rejected (401). Check the key in Settings.".into(),
        ));
    }
    if status == 403 {
        return Err(AppError::Message(
            "Cursor API returned 403 — Admin API keys require a Teams/Enterprise admin key from cursor.com/dashboard → API Keys.".into(),
        ));
    }
    if status == 429 {
        return Err(AppError::Message(
            "Cursor API rate limited. Try again in a minute.".into(),
        ));
    }
    if !(200..300).contains(&status) {
        return Err(AppError::Message(format!(
            "Cursor API error {status}: {}",
            text.chars().take(240).collect::<String>()
        )));
    }

    serde_json::from_str(&text).map_err(|e| {
        AppError::Message(format!("Cursor API response parse failed: {e}"))
    })
}

fn map_http_err(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if code == 401 {
                return AppError::Message(
                    "Cursor API key rejected (401). Check the key in Settings.".into(),
                );
            }
            if code == 403 {
                return AppError::Message(
                    "Cursor API returned 403 — need a Teams/Enterprise Admin API key.".into(),
                );
            }
            AppError::Message(format!(
                "Cursor API HTTP {code}: {}",
                body.chars().take(240).collect::<String>()
            ))
        }
        other => AppError::Message(format!("Cursor API request failed: {other}")),
    }
}

enum UpsertOutcome {
    Inserted { assigned: bool },
    Updated { assigned: bool },
    Skipped,
}

fn upsert_event(
    db: &Db,
    ev: &UsageEvent,
    default_project_id: Option<&str>,
) -> AppResult<UpsertOutcome> {
    let ts_ms = parse_timestamp_ms(ev.timestamp.as_ref())?;
    let occurred = chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let conversation_id = ev
        .conversation_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let model = ev.model.clone();

    let input_tokens = ev.token_usage.as_ref().and_then(|t| t.input_tokens);
    let output_tokens = ev.token_usage.as_ref().and_then(|t| t.output_tokens);
    let cached_tokens = ev.token_usage.as_ref().and_then(|t| {
        Some(t.cache_read_tokens.unwrap_or(0) + t.cache_write_tokens.unwrap_or(0))
            .filter(|&n| n > 0)
    });
    let total_tokens = match (input_tokens, output_tokens, cached_tokens) {
        (None, None, None) => None,
        _ => Some(
            input_tokens.unwrap_or(0)
                + output_tokens.unwrap_or(0)
                + cached_tokens.unwrap_or(0),
        ),
    };

    // Prefer Cursor's billed `chargedCents` (reconciles with dashboard spend).
    // Fall back to model `totalCents` when billed amount is absent.
    let reported_cost_minor = ev
        .charged_cents
        .filter(|c| c.is_finite() && *c > 0.0)
        .map(|c| c.round() as i64)
        .or_else(|| {
            ev.token_usage
                .as_ref()
                .and_then(|t| t.total_cents)
                .filter(|c| c.is_finite() && *c > 0.0)
                .map(|c| c.round() as i64)
        });

    // Skip completely empty rows (no tokens and no cost).
    if total_tokens.is_none() && reported_cost_minor.is_none() {
        return Ok(UpsertOutcome::Skipped);
    }

    let external_key = event_external_key(ts_ms, model.as_deref(), conversation_id.as_deref(), total_tokens, reported_cost_minor);

    let (mapped_project, mapped_task) = if let Some(cid) = conversation_id.as_deref() {
        lookup_conversation(db, cid)?
    } else {
        (None, None)
    };
    let attribution = if mapped_project.is_some() {
        "conversation_mapping"
    } else if default_project_id.is_some() {
        "default_project"
    } else {
        "import"
    };
    let project_id = mapped_project.or_else(|| default_project_id.map(|s| s.to_string()));
    let task_id = mapped_task;
    let assigned = project_id.is_some();

    let verification = if ev.is_chargeable.unwrap_or(false) || reported_cost_minor.is_some() {
        "cursor_api"
    } else {
        "cursor_api_included"
    };
    let notes = ev.kind.clone();

    let id = Uuid::new_v4().to_string();
    let existed: bool = db.with_conn(|conn| {
        let row: Option<String> = conn.exec_first(
            "SELECT id FROM ai_usage_records WHERE external_key = ?",
            (&external_key,),
        )?;
        Ok(row.is_some())
    })?;

    db.with_conn(|conn| {
        conn.exec_drop(
            "INSERT INTO ai_usage_records (
                id, provider, model, occurred_at, project_id, task_id, conversation_id,
                input_tokens, output_tokens, cached_tokens, total_tokens,
                reported_cost_minor, currency, billing_source, verification_status,
                attribution_method, external_key, notes
             ) VALUES (:id, 'cursor', :model, :occurred, :project_id, :task_id, :conversation_id,
                :input_tokens, :output_tokens, :cached_tokens, :total_tokens,
                :cost, 'USD', 'cursor_admin_api', :verification, :attribution, :external_key, :notes)
             ON DUPLICATE KEY UPDATE
                model = VALUES(model),
                occurred_at = VALUES(occurred_at),
                conversation_id = COALESCE(VALUES(conversation_id), conversation_id),
                project_id = COALESCE(VALUES(project_id), project_id),
                task_id = COALESCE(VALUES(task_id), task_id),
                input_tokens = COALESCE(VALUES(input_tokens), input_tokens),
                output_tokens = COALESCE(VALUES(output_tokens), output_tokens),
                cached_tokens = COALESCE(VALUES(cached_tokens), cached_tokens),
                total_tokens = COALESCE(VALUES(total_tokens), total_tokens),
                reported_cost_minor = COALESCE(VALUES(reported_cost_minor), reported_cost_minor),
                verification_status = VALUES(verification_status),
                notes = COALESCE(VALUES(notes), notes)",
            params! {
                "id" => &id,
                "model" => &model,
                "occurred" => &occurred,
                "project_id" => &project_id,
                "task_id" => &task_id,
                "conversation_id" => &conversation_id,
                "input_tokens" => input_tokens,
                "output_tokens" => output_tokens,
                "cached_tokens" => cached_tokens,
                "total_tokens" => total_tokens,
                "cost" => reported_cost_minor,
                "verification" => verification,
                "attribution" => attribution,
                "external_key" => &external_key,
                "notes" => &notes,
            },
        )?;
        Ok(())
    })?;

    Ok(if existed {
        UpsertOutcome::Updated { assigned }
    } else {
        UpsertOutcome::Inserted { assigned }
    })
}

fn lookup_conversation(
    db: &Db,
    conversation_id: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    db.with_conn(|conn| {
        let row: Option<(Option<String>, Option<String>)> = conn.exec_first(
            "SELECT project_id, task_id FROM cursor_conversations WHERE conversation_id = ?",
            (conversation_id,),
        )?;
        Ok(row.unwrap_or((None, None)))
    })
}

fn parse_timestamp_ms(v: Option<&serde_json::Value>) -> AppResult<i64> {
    let Some(v) = v else {
        return Ok(chrono::Utc::now().timestamp_millis());
    };
    match v {
        serde_json::Value::Number(n) => Ok(n.as_i64().unwrap_or(0)),
        serde_json::Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| AppError::Message(format!("bad timestamp: {s}"))),
        _ => Ok(chrono::Utc::now().timestamp_millis()),
    }
}

fn event_external_key(
    ts_ms: i64,
    model: Option<&str>,
    conversation_id: Option<&str>,
    total_tokens: Option<i64>,
    cost_minor: Option<i64>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ts_ms.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(model.unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(conversation_id.unwrap_or("").as_bytes());
    hasher.update(b"|");
    hasher.update(total_tokens.unwrap_or(0).to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(cost_minor.unwrap_or(0).to_string().as_bytes());
    format!("cursor_api:{}", hex::encode(hasher.finalize()))
}

pub fn test_api_key(db: &Db) -> AppResult<String> {
    let api_key = settings::get(db, "cursor_api_key")?
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Err(AppError::Message("No Cursor API key saved".into()));
    }
    let end_ms = chrono::Utc::now().timestamp_millis();
    let start_ms = end_ms - 24 * 60 * 60 * 1000;
    let body = json!({
        "startDate": start_ms,
        "endDate": end_ms,
        "page": 1,
        "pageSize": 1
    });
    let resp = fetch_page(&api_key, &body)?;
    Ok(format!(
        "Connected — {} usage event(s) visible in the last 24h",
        resp.total_usage_events_count
    ))
}
