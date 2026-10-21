use crate::error::AppResult;
use crate::finance::ProjectFinancials;
use crate::models::*;
use crate::services::{
    attribution, currency, cursor_api, dashboard, hooks, ingest, projects, settings, usage,
};
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppResult<serde_json::Value> {
    Ok(serde_json::json!({
        "name": "FlowLedger",
        "version": env!("CARGO_PKG_VERSION"),
        "db_path": state.db.label(),
        "ingest_listening": *state.ingest_listening.read(),
    }))
}

#[tauri::command]
pub fn list_projects(state: State<'_, AppState>) -> AppResult<Vec<Project>> {
    projects::list_projects(&state.db)
}

#[tauri::command]
pub fn create_project(state: State<'_, AppState>, input: NewProject) -> AppResult<Project> {
    projects::create_project(&state.db, input)
}

#[tauri::command]
pub fn update_project(
    state: State<'_, AppState>,
    project_id: String,
    input: UpdateProject,
) -> AppResult<Project> {
    projects::update_project(&state.db, &project_id, input)
}

#[tauri::command]
pub fn delete_project(state: State<'_, AppState>, project_id: String) -> AppResult<()> {
    projects::delete_project(&state.db, &project_id)
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>, project_id: Option<String>) -> AppResult<Vec<Task>> {
    projects::list_tasks(&state.db, project_id.as_deref())
}

#[tauri::command]
pub fn create_task(state: State<'_, AppState>, input: NewTask) -> AppResult<Task> {
    projects::create_task(&state.db, input)
}

#[tauri::command]
pub fn update_task_title(
    state: State<'_, AppState>,
    task_id: String,
    title: String,
) -> AppResult<Task> {
    projects::update_task_title(&state.db, &task_id, &title)
}

#[tauri::command]
pub fn update_project_finances(
    state: State<'_, AppState>,
    project_id: String,
    update: ProjectFinanceUpdate,
) -> AppResult<()> {
    projects::update_project_finances(&state.db, &project_id, update)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsMap> {
    settings::get_all(&state.db)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    values: HashMap<String, String>,
) -> AppResult<()> {
    settings::set_many(&state.db, &values)
}

#[tauri::command]
pub fn get_dashboard(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> AppResult<DashboardSummary> {
    let listening = *state.ingest_listening.read();
    dashboard::summary(&state.db, project_id.as_deref(), listening)
}

#[tauri::command]
pub fn get_project_financials(
    state: State<'_, AppState>,
    project_id: String,
) -> AppResult<ProjectFinancials> {
    dashboard::project_financials_dto(&state.db, &project_id)
}

#[tauri::command]
pub fn list_activity(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> AppResult<Vec<ActivityEventDto>> {
    dashboard::recent_events(&state.db, project_id.as_deref(), 200)
}

#[tauri::command]
pub fn list_unassigned(state: State<'_, AppState>) -> AppResult<Vec<UnassignedItem>> {
    attribution::list_unassigned(&state.db)
}

#[tauri::command]
pub fn list_unassigned_page(
    state: State<'_, AppState>,
    query: UnassignedQuery,
) -> AppResult<UnassignedPage> {
    attribution::list_unassigned_page(&state.db, &query)
}

#[tauri::command]
pub fn dismiss_noise_unassigned(state: State<'_, AppState>) -> AppResult<i64> {
    attribution::dismiss_noise_unassigned(&state.db)
}

#[tauri::command]
pub fn assign_unassigned(
    state: State<'_, AppState>,
    unassigned_id: String,
    project_id: String,
    task_id: Option<String>,
) -> AppResult<()> {
    attribution::assign_unassigned(
        &state.db,
        &unassigned_id,
        &project_id,
        task_id.as_deref(),
    )
}

#[tauri::command]
pub fn validate_workspace_path(path: String) -> serde_json::Value {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return serde_json::json!({
            "valid": false,
            "exists": false,
            "is_dir": false,
            "normalized": "",
            "detail": "Enter or choose a folder path",
        });
    }
    let normalized = crate::services::projects::normalize_path(trimmed);
    let p = std::path::Path::new(&normalized);
    let exists = p.exists();
    let is_dir = p.is_dir();
    let valid = exists && is_dir;
    serde_json::json!({
        "valid": valid,
        "exists": exists,
        "is_dir": is_dir,
        "normalized": normalized,
        "detail": if valid {
            "Workspace folder verified"
        } else if !exists {
            "Path not found on this machine"
        } else {
            "Path exists but is not a folder"
        },
    })
}

/// Immediate child folders under a parent path (for bulk local project sync).
#[tauri::command]
pub fn list_child_directories(path: String) -> AppResult<Vec<ChildDirectory>> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(crate::error::AppError::Message(
            "Enter or choose a parent folder path".into(),
        ));
    }
    let normalized = crate::services::projects::normalize_path(trimmed);
    let parent = std::path::Path::new(&normalized);
    if !parent.exists() {
        return Err(crate::error::AppError::Message(
            "Path not found on this machine".into(),
        ));
    }
    if !parent.is_dir() {
        return Err(crate::error::AppError::Message(
            "Path exists but is not a folder".into(),
        ));
    }
    let mut children = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let abs = entry.path().to_string_lossy().to_string();
        let path = crate::services::projects::normalize_path(&abs);
        children.push(ChildDirectory { name, path });
    }
    children.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(children)
}

#[tauri::command]
pub fn add_usage_record(
    state: State<'_, AppState>,
    input: UsageRecordInput,
) -> AppResult<usage::UsageRecordDto> {
    usage::add_manual(&state.db, input)
}

#[tauri::command]
pub fn import_usage_csv(
    state: State<'_, AppState>,
    filename: String,
    csv_text: String,
) -> AppResult<usage::ImportResult> {
    usage::import_csv(&state.db, &filename, &csv_text)
}

#[tauri::command]
pub fn sync_cursor_usage(
    state: State<'_, AppState>,
    days: Option<i64>,
    project_id: Option<String>,
) -> AppResult<cursor_api::SyncResult> {
    cursor_api::sync_usage(
        &state.db,
        days.unwrap_or(30),
        project_id.as_deref(),
    )
}

#[tauri::command]
pub fn test_cursor_api_key(state: State<'_, AppState>) -> AppResult<String> {
    cursor_api::test_api_key(&state.db)
}

#[tauri::command]
pub fn list_usage(
    state: State<'_, AppState>,
    project_id: Option<String>,
) -> AppResult<Vec<usage::UsageRecordDto>> {
    usage::list_usage(&state.db, project_id.as_deref())
}

#[tauri::command]
pub fn install_cursor_hooks(state: State<'_, AppState>) -> AppResult<hooks::HookInstallStatus> {
    let port = settings::ingest_port(&state.db)?;
    let token = settings::get(&state.db, "ingest_token")?.unwrap_or_default();
    let resource_dir = state.resource_dir.read().clone();
    hooks::install(&state.db, resource_dir, port, &token)
}

#[tauri::command]
pub fn uninstall_cursor_hooks(state: State<'_, AppState>) -> AppResult<hooks::HookInstallStatus> {
    hooks::uninstall(&state.db)
}

#[tauri::command]
pub fn hooks_status(state: State<'_, AppState>) -> AppResult<hooks::HookInstallStatus> {
    hooks::status(&state.db)
}

#[tauri::command]
pub fn ingest_test_event(
    state: State<'_, AppState>,
    event: HookEventIn,
) -> AppResult<ingest::IngestOutcome> {
    ingest::ingest_event(&state.db, event)
}

#[tauri::command]
pub fn apply_idle_sweep(state: State<'_, AppState>) -> AppResult<()> {
    ingest::apply_idle_timeout(&state.db, chrono::Utc::now())
}

#[tauri::command]
pub fn list_currencies() -> Vec<currency::CurrencyInfo> {
    currency::supported_currencies()
}

#[tauri::command]
pub fn list_exchange_rates(
    state: State<'_, AppState>,
) -> AppResult<Vec<currency::ExchangeRateDto>> {
    currency::list_rates(&state.db)
}

#[tauri::command]
pub fn upsert_exchange_rate(
    state: State<'_, AppState>,
    base_currency: String,
    quote_currency: String,
    rate_numeric: String,
    effective_on: Option<String>,
) -> AppResult<currency::ExchangeRateDto> {
    let on = effective_on.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    currency::upsert_rate(
        &state.db,
        &base_currency,
        &quote_currency,
        &rate_numeric,
        &on,
    )
}
