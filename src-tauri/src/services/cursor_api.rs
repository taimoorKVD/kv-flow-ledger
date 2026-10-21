//! Sync Cursor usage + cost into local `ai_usage_records`.
//!
//! Default path is **this laptop’s signed-in Cursor user** (not team-member rollups):
//! 1. **Pro / Pro+ dashboard session** — `POST https://cursor.com/api/dashboard/get-filtered-usage-events`
//!    with `WorkosCursorSessionToken` from the local Cursor IDE login (or a pasted session token).
//!    Events carry `conversationId`, which we map onto local projects/tasks created by hooks.
//! 2. **Team Admin API** (fallback) — `POST https://api.cursor.com/teams/filtered-usage-events`
//!    with Basic auth (`crsr_…` Team Admin key). Used only when no local session is available.
//!
//! Personal/user/agent API keys (`crsr_…`) validate on `/v1/me` but cannot call `/teams/*` or
//! return billed usage by themselves. Keep Cursor Desktop signed in on this machine for sync.

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::services::settings;
use base64::Engine;
use mysql::params;
use mysql::prelude::*;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

const TEAM_USAGE_URL: &str = "https://api.cursor.com/teams/filtered-usage-events";
const DASHBOARD_USAGE_URL: &str = "https://cursor.com/api/dashboard/get-filtered-usage-events";
const V1_ME_URL: &str = "https://api.cursor.com/v1/me";
const AUTH_ME_URL: &str = "https://cursor.com/api/auth/me";

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub fetched: i64,
    pub inserted: i64,
    pub updated: i64,
    pub attributed: i64,
    pub pages: u32,
    pub detail: String,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub source: String,
    pub detail: String,
    pub key_kind: String,
}

#[derive(Debug, Deserialize)]
struct UsageEventsResponse {
    #[serde(default, rename = "totalUsageEventsCount")]
    total_usage_events_count: i64,
    #[serde(default)]
    pagination: Option<Pagination>,
    #[serde(default, rename = "usageEvents")]
    usage_events: Vec<UsageEvent>,
    /// Dashboard (Pro) response shape
    #[serde(default, rename = "usageEventsDisplay")]
    usage_events_display: Vec<UsageEvent>,
}

impl UsageEventsResponse {
    fn events(&self) -> &[UsageEvent] {
        if !self.usage_events.is_empty() {
            &self.usage_events
        } else {
            &self.usage_events_display
        }
    }
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
    /// Some payloads use camelCase alternate keys.
    #[serde(default, rename = "composerId")]
    composer_id: Option<String>,
    model: Option<String>,
    kind: Option<String>,
    #[serde(default, rename = "isChargeable")]
    is_chargeable: Option<bool>,
    #[serde(default, rename = "chargedCents")]
    charged_cents: Option<f64>,
    #[serde(default, rename = "tokenUsage")]
    token_usage: Option<TokenUsage>,
    /// Present on Team Admin events — used only if we ever filter team data.
    #[serde(default, rename = "owningUser")]
    owning_user: Option<String>,
    #[serde(default)]
    email: Option<String>,
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

#[derive(Debug, Clone)]
enum AuthMode {
    TeamBasic { api_key: String },
    DashboardSession { cookie: String, user_id: Option<i64> },
}

/// Pull usage for the last `days` days.
/// Prefers this machine’s Cursor IDE session (local agent/chat work) over Team Admin
/// member rollups. Events are attributed to projects/tasks via `conversationId` links
/// written by local hooks — not by team member identity.
pub fn sync_usage(
    db: &Db,
    days: i64,
    default_project_id: Option<&str>,
) -> AppResult<SyncResult> {
    let auth = resolve_auth(db)?;
    let source = match &auth {
        AuthMode::TeamBasic { .. } => "team_admin_api",
        AuthMode::DashboardSession { .. } => "dashboard_session",
    };

    let days = days.clamp(1, 90);
    let end_ms = chrono::Utc::now().timestamp_millis();
    let start_ms = end_ms - days * 24 * 60 * 60 * 1000;

    let mut fetched = 0i64;
    let mut inserted = 0i64;
    let mut updated = 0i64;
    let mut attributed = 0i64;
    let mut page: u32 = 1;
    let mut pages_done = 0u32;
    let billing = match &auth {
        AuthMode::TeamBasic { .. } => "cursor_admin_api",
        AuthMode::DashboardSession { .. } => "cursor_dashboard",
    };

    loop {
        let resp = fetch_usage_page(&auth, start_ms, end_ms, page)?;
        pages_done += 1;
        let events = resp.events();
        fetched += events.len() as i64;

        for ev in events {
            match upsert_event(db, ev, default_project_id, billing)? {
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
            .unwrap_or_else(|| {
                // Dashboard often omits pagination; keep going while a full page returns.
                events.len() as u32 >= 1000
            });
        let num_pages = resp.pagination.as_ref().map(|p| p.num_pages).unwrap_or(1);
        if !has_next || page >= num_pages.max(1) || page >= 50 || events.is_empty() {
            break;
        }
        page += 1;
    }

    // Re-link any previously imported rows whose conversation now maps to a local task.
    let relinked = reattribute_usage_from_conversations(db)?;
    attributed += relinked;

    let now = chrono::Utc::now().to_rfc3339();
    let scope = match source {
        "dashboard_session" => "this laptop’s Cursor user",
        _ => "Team Admin API (fallback)",
    };
    let detail = format!(
        "Synced {fetched} events via {source} ({scope}) — {inserted} new, {updated} updated, {attributed} linked to local agent/tasks · {days}d"
    );
    settings::set(db, "cursor_usage_last_sync_at", &now)?;
    settings::set(db, "cursor_usage_last_sync_detail", &detail)?;

    Ok(SyncResult {
        fetched,
        inserted,
        updated,
        attributed,
        pages: pages_done,
        detail,
        source: source.into(),
    })
}

pub fn test_api_key(db: &Db) -> AppResult<String> {
    let probe = probe_connection(db)?;
    if probe.ok {
        Ok(format!("{} — {}", probe.source, probe.detail))
    } else {
        Err(AppError::Message(format!(
            "{} ({})",
            probe.detail, probe.key_kind
        )))
    }
}

pub fn probe_connection(db: &Db) -> AppResult<ProbeResult> {
    let api_key = normalize_api_key(&settings::get(db, "cursor_api_key")?.unwrap_or_default());

    // 1) If a crsr_ key is present, see whether it is a Team Admin key or a user/agent key.
    let mut key_kind = "none".to_string();
    if !api_key.is_empty() {
        if !api_key.starts_with("crsr_") {
            return Ok(ProbeResult {
                ok: false,
                source: "none".into(),
                detail: "API key should start with crsr_.".into(),
                key_kind: "invalid_format".into(),
            });
        }
        match probe_v1_me(&api_key) {
            Ok(label) => {
                key_kind = format!("user_or_agent_key ({label})");
            }
            Err(e) => {
                key_kind = format!("key_unrecognized ({e})");
            }
        }
        match fetch_usage_page(
            &AuthMode::TeamBasic {
                api_key: api_key.clone(),
            },
            chrono::Utc::now().timestamp_millis() - 86_400_000,
            chrono::Utc::now().timestamp_millis(),
            1,
        ) {
            Ok(resp) => {
                return Ok(ProbeResult {
                    ok: true,
                    source: "team_admin_api".into(),
                    detail: format!(
                        "Team Admin API OK — {} event(s) in last 24h",
                        resp.total_usage_events_count
                    ),
                    key_kind: "team_admin".into(),
                });
            }
            Err(e) => {
                // Expected for Pro Plus user keys — fall through to session.
                let msg = e.to_string();
                if !msg.to_ascii_lowercase().contains("invalid team")
                    && !msg.contains("401")
                    && !msg.contains("403")
                {
                    // Network / parse issues — still try session, but keep note.
                    key_kind = format!("{key_kind}; team_api: {msg}");
                }
            }
        }
    }

    // 2) Dashboard session (Pro / Pro+)
    match resolve_dashboard_session(db) {
        Ok((cookie, user_id)) => {
            match fetch_usage_page(
                &AuthMode::DashboardSession {
                    cookie: cookie.clone(),
                    user_id,
                },
                chrono::Utc::now().timestamp_millis() - 86_400_000,
                chrono::Utc::now().timestamp_millis(),
                1,
            ) {
                Ok(resp) => Ok(ProbeResult {
                    ok: true,
                    source: "dashboard_session".into(),
                    detail: format!(
                        "Local Cursor session OK — {} event(s) in last 24h (this laptop’s usage, linked to agent/tasks via conversation ids). {}",
                        resp.total_usage_events_count,
                        if api_key.is_empty() {
                            String::new()
                        } else {
                            "User/agent crsr_ keys cannot fetch usage by themselves; FlowLedger uses your IDE session for local agent/task costs.".into()
                        }
                    ),
                    key_kind,
                }),
                Err(e) => Ok(ProbeResult {
                    ok: false,
                    source: "dashboard_session".into(),
                    detail: format!("Dashboard session found but usage fetch failed: {e}"),
                    key_kind,
                }),
            }
        }
        Err(e) => Ok(ProbeResult {
            ok: false,
            source: "none".into(),
            detail: format!(
                "Team Admin API unavailable for this key, and no Cursor IDE session was found ({e}). Sign in to Cursor Desktop, or paste a WorkosCursorSessionToken under Settings."
            ),
            key_kind,
        }),
    }
}

fn resolve_auth(db: &Db) -> AppResult<AuthMode> {
    // Prefer local IDE session so usage is this laptop’s agent/chat work, attributed
    // by conversation → task — not team-wide member spend from Admin API.
    if let Ok((cookie, user_id)) = resolve_dashboard_session(db) {
        let end_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = end_ms - 3_600_000;
        if fetch_usage_page(
            &AuthMode::DashboardSession {
                cookie: cookie.clone(),
                user_id,
            },
            start_ms,
            end_ms,
            1,
        )
        .is_ok()
        {
            return Ok(AuthMode::DashboardSession { cookie, user_id });
        }
    }

    let api_key = normalize_api_key(&settings::get(db, "cursor_api_key")?.unwrap_or_default());
    if !api_key.is_empty() && api_key.starts_with("crsr_") {
        let end_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = end_ms - 3_600_000;
        if fetch_usage_page(
            &AuthMode::TeamBasic {
                api_key: api_key.clone(),
            },
            start_ms,
            end_ms,
            1,
        )
        .is_ok()
        {
            return Ok(AuthMode::TeamBasic { api_key });
        }
        return Err(AppError::Message(
            "Your crsr_ key is a user/agent key — it cannot fetch billed usage. Sign in to Cursor Desktop on this laptop (or paste a session token) so FlowLedger can sync local agent/task usage.".into(),
        ));
    }

    Err(AppError::Message(
        "No Cursor session found on this laptop. Sign in to Cursor Desktop, or paste a WorkosCursorSessionToken under Settings. (User/agent API keys alone cannot pull usage.)".into(),
    ))
}

fn resolve_dashboard_session(db: &Db) -> AppResult<(String, Option<i64>)> {
    // Prefer explicit override from Settings.
    let pasted = settings::get(db, "cursor_session_token")?.unwrap_or_default();
    let pasted = pasted.trim().to_string();
    if !pasted.is_empty() {
        let cookie = normalize_session_cookie(&pasted)?;
        let user_id = fetch_auth_me_user_id(&cookie).ok();
        return Ok((cookie, user_id));
    }

    // Auto: read JWT from local Cursor IDE state DB (same machine, signed-in user).
    let jwt = read_local_cursor_access_token()?;
    let sub = jwt_sub(&jwt)?;
    let cookie = format!("{sub}::{jwt}");
    let user_id = fetch_auth_me_user_id(&cookie).ok();
    Ok((cookie, user_id))
}

fn normalize_session_cookie(raw: &str) -> AppResult<String> {
    let s = raw.trim();
    // Already "sub::jwt" or "sub%3A%3Ajwt"
    if s.contains("::") {
        return Ok(s.replace("%3A%3A", "::"));
    }
    if s.contains("%3A%3A") {
        return Ok(s.replace("%3A%3A", "::"));
    }
    // Bare JWT — derive sub
    if s.starts_with("eyJ") {
        let sub = jwt_sub(s)?;
        return Ok(format!("{sub}::{s}"));
    }
    Err(AppError::Message(
        "Session token should be a Cursor JWT (eyJ…) or WorkosCursorSessionToken value (sub::jwt)".into(),
    ))
}

fn read_local_cursor_access_token() -> AppResult<String> {
    let path = cursor_state_vscdb_path()?;
    if !path.exists() {
        return Err(AppError::Message(format!(
            "Cursor state DB not found at {}",
            path.display()
        )));
    }
    let conn = rusqlite::Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| AppError::Message(format!("Open Cursor state DB failed: {e}")))?;
    let token: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| {
            AppError::Message(
                "No cursorAuth/accessToken in Cursor state DB — sign in to Cursor Desktop first".into(),
            )
        })?;
    let token = token.trim().trim_matches('"').to_string();
    if token.is_empty() {
        return Err(AppError::Message("Empty Cursor access token".into()));
    }
    Ok(token)
}

fn cursor_state_vscdb_path() -> AppResult<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA")
            .map_err(|_| AppError::Message("APPDATA not set".into()))?;
        Ok(PathBuf::from(appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().ok_or_else(|| AppError::Message("home dir not found".into()))?;
        Ok(home
            .join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let home = dirs::home_dir().ok_or_else(|| AppError::Message("home dir not found".into()))?;
        Ok(home
            .join(".config")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    }
}

fn jwt_sub(jwt: &str) -> AppResult<String> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() < 2 {
        return Err(AppError::Message("Invalid JWT".into()));
    }
    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{payload}=="),
        3 => format!("{payload}="),
        _ => payload.to_string(),
    };
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(&padded))
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&padded))
        .map_err(|e| AppError::Message(format!("JWT decode failed: {e}")))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::Message(format!("JWT JSON failed: {e}")))?;
    v.get("sub")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Message("JWT missing sub claim".into()))
}

fn probe_v1_me(api_key: &str) -> AppResult<String> {
    let agent = http_agent();
    let resp = agent
        .get(V1_ME_URL)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Accept", "application/json")
        .call()
        .map_err(|e| AppError::Message(format!("v1/me failed: {e}")))?;
    let status = resp.status();
    let text = resp.into_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(AppError::Message(format!("v1/me HTTP {status}")));
    }
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let label = v
        .get("email")
        .or_else(|| v.get("name"))
        .or_else(|| v.get("userId"))
        .and_then(|x| x.as_str())
        .unwrap_or("ok");
    Ok(label.to_string())
}

fn fetch_auth_me_user_id(cookie: &str) -> AppResult<i64> {
    let agent = http_agent();
    let cookie_header = format!(
        "WorkosCursorSessionToken={}",
        cookie.replace("::", "%3A%3A")
    );
    let resp = agent
        .get(AUTH_ME_URL)
        .set("Cookie", &cookie_header)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| AppError::Message(format!("auth/me failed: {e}")))?;
    let status = resp.status();
    let text = resp.into_string().unwrap_or_default();
    if !(200..300).contains(&status) {
        return Err(AppError::Message(format!(
            "auth/me HTTP {status}: {}",
            text.chars().take(120).collect::<String>()
        )));
    }
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Message(format!("auth/me parse: {e}")))?;
    v.get("id")
        .and_then(|x| x.as_i64().or_else(|| x.as_u64().map(|u| u as i64)))
        .ok_or_else(|| AppError::Message("auth/me missing id".into()))
}

fn fetch_usage_page(
    auth: &AuthMode,
    start_ms: i64,
    end_ms: i64,
    page: u32,
) -> AppResult<UsageEventsResponse> {
    let agent = http_agent();
    match auth {
        AuthMode::TeamBasic { api_key } => {
            let basic = base64::engine::general_purpose::STANDARD.encode(format!("{api_key}:"));
            let body = json!({
                "startDate": start_ms,
                "endDate": end_ms,
                "page": page,
                "pageSize": 1000
            });
            let resp = agent
                .post(TEAM_USAGE_URL)
                .set("Authorization", &format!("Basic {basic}"))
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .send_json(body)
                .map_err(map_http_err)?;
            parse_usage_response(resp)
        }
        AuthMode::DashboardSession { cookie, user_id } => {
            let cookie_header = format!(
                "WorkosCursorSessionToken={}",
                cookie.replace("::", "%3A%3A")
            );
            // Dashboard accepts epoch ms as numbers or strings; include userId when known.
            let mut body = json!({
                "teamId": 0,
                "startDate": start_ms.to_string(),
                "endDate": end_ms.to_string(),
                "page": page,
                "pageSize": 1000
            });
            if let Some(uid) = user_id {
                body["userId"] = json!(uid);
            }
            let resp = agent
                .post(DASHBOARD_USAGE_URL)
                .set("Cookie", &cookie_header)
                .set("Origin", "https://cursor.com")
                .set("Content-Type", "application/json")
                .set("Accept", "application/json")
                .send_json(body)
                .map_err(map_http_err)?;
            parse_usage_response(resp)
        }
    }
}

fn parse_usage_response(resp: ureq::Response) -> AppResult<UsageEventsResponse> {
    let status = resp.status();
    let text = resp
        .into_string()
        .map_err(|e| AppError::Message(format!("Cursor read failed: {e}")))?;
    if status == 401 || status == 403 {
        return Err(AppError::Message(auth_failure_message(status, &text)));
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
    serde_json::from_str(&text)
        .map_err(|e| AppError::Message(format!("Cursor usage JSON parse failed: {e}")))
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(60))
        .build()
}

fn map_http_err(e: ureq::Error) -> AppError {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if code == 401 || code == 403 {
                return AppError::Message(auth_failure_message(code, &body));
            }
            AppError::Message(format!(
                "Cursor HTTP {code}: {}",
                body.chars().take(240).collect::<String>()
            ))
        }
        other => AppError::Message(format!("Cursor request failed: {other}")),
    }
}

fn normalize_api_key(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c| c == '"' || c == '\'' || c == '`')
        .trim()
        .replace(['\r', '\n', ' ', '\t'], "")
}

fn auth_failure_message(status: u16, body: &str) -> String {
    let lower = body.to_ascii_lowercase();
    let hint = if lower.contains("invalid team") || lower.contains("team api key") {
        "User/agent API keys cannot call /teams/*. FlowLedger will use your Cursor IDE login session for Pro/Pro+ usage instead."
    } else if lower.contains("not_authenticated") || lower.contains("unauthorized") {
        "Session expired — sign in to Cursor Desktop again, or paste a fresh session token."
    } else {
        "Authentication failed talking to Cursor."
    };
    let snippet = body.chars().take(120).collect::<String>();
    if snippet.trim().is_empty() {
        format!("Cursor {status}: {hint}")
    } else {
        format!("Cursor {status}: {hint} ({snippet})")
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
    billing_source: &str,
) -> AppResult<UpsertOutcome> {
    let ts_ms = parse_timestamp_ms(ev.timestamp.as_ref())?;
    let occurred = chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let conversation_id = ev
        .conversation_id
        .as_ref()
        .or(ev.composer_id.as_ref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let model = ev.model.clone();
    // Silence unused team-member fields (kept for forward-compatible parse).
    let _ = (&ev.owning_user, &ev.email);

    let input_tokens = ev.token_usage.as_ref().and_then(|t| t.input_tokens);
    let output_tokens = ev.token_usage.as_ref().and_then(|t| t.output_tokens);
    let cached_tokens = ev.token_usage.as_ref().and_then(|t| {
        Some(t.cache_read_tokens.unwrap_or(0) + t.cache_write_tokens.unwrap_or(0)).filter(|&n| n > 0)
    });
    let total_tokens = match (input_tokens, output_tokens, cached_tokens) {
        (None, None, None) => None,
        _ => Some(
            input_tokens.unwrap_or(0) + output_tokens.unwrap_or(0) + cached_tokens.unwrap_or(0),
        ),
    };

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

    if total_tokens.is_none() && reported_cost_minor.is_none() {
        return Ok(UpsertOutcome::Skipped);
    }

    let external_key = event_external_key(
        ts_ms,
        model.as_deref(),
        conversation_id.as_deref(),
        total_tokens,
        reported_cost_minor,
    );

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
                :cost, 'USD', :billing, :verification, :attribution, :external_key, :notes)
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
                billing_source = VALUES(billing_source),
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
                "billing" => billing_source,
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

/// Attach project/task on usage rows once local hooks have recorded the conversation.
fn reattribute_usage_from_conversations(db: &Db) -> AppResult<i64> {
    db.with_conn_mut(|conn| {
        conn.exec_drop(
            "UPDATE ai_usage_records u
             INNER JOIN cursor_conversations c
               ON c.conversation_id = u.conversation_id
              AND c.project_id IS NOT NULL
             SET
               u.project_id = COALESCE(u.project_id, c.project_id),
               u.task_id = COALESCE(u.task_id, c.task_id),
               u.attribution_method = 'conversation_mapping'
             WHERE IFNULL(u.conversation_id, '') != ''
               AND (
                 u.project_id IS NULL
                 OR (u.task_id IS NULL AND c.task_id IS NOT NULL)
               )",
            (),
        )?;
        Ok(conn.affected_rows() as i64)
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
