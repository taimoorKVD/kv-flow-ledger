use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub client: Option<String>,
    pub description: Option<String>,
    pub git_repository: Option<String>,
    pub status: String,
    pub billing_model: String,
    pub contracted_budget_minor: i64,
    pub currency: String,
    pub labor_rate_override_minor: Option<i64>,
    pub created_at: String,
    pub archived_at: Option<String>,
    pub workspaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProject {
    pub name: String,
    pub client: Option<String>,
    pub description: Option<String>,
    pub git_repository: Option<String>,
    pub billing_model: String,
    pub contracted_budget_minor: i64,
    pub currency: String,
    pub workspaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: String,
    pub client: Option<String>,
    pub description: Option<String>,
    pub git_repository: Option<String>,
    pub billing_model: String,
    pub contracted_budget_minor: i64,
    pub currency: String,
    pub workspaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub github_issue: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub combined_work_ms: i64,
    pub agent_ms: i64,
    pub developer_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEventIn {
    pub event_type: String,
    pub occurred_at: Option<String>,
    pub workspace_roots: Option<Vec<String>>,
    pub conversation_id: Option<String>,
    pub generation_id: Option<String>,
    pub model: Option<String>,
    pub model_id: Option<String>,
    pub agent_state: Option<String>,
    pub cursor_version: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingStatus {
    pub hooks_installed: bool,
    pub ingest_listening: bool,
    pub ingest_port: u16,
    pub active_project_id: Option<String>,
    pub active_task_id: Option<String>,
    pub active_conversation_id: Option<String>,
    pub mode: String,
    pub last_event_at: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub project_id: Option<String>,
    pub task_work_ms: i64,
    pub ai_tokens: Option<i64>,
    pub ai_tokens_available: bool,
    pub allocated_cost_minor: i64,
    pub margin_pct: Option<f64>,
    pub labor_minor: i64,
    pub ai_minor: i64,
    pub other_minor: i64,
    pub recognized_revenue_minor: i64,
    /// Contract P/L when contracted payment is set; null means N/A.
    pub profit_minor: Option<i64>,
    pub currency: String,
    pub active_projects: i64,
    pub active_tasks: i64,
    pub unassigned_count: i64,
    pub tracking: TrackingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecordInput {
    pub task_id: Option<String>,
    pub project_id: Option<String>,
    pub model: Option<String>,
    pub occurred_at: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub reported_cost_minor: Option<i64>,
    pub currency: String,
    pub billing_source: String,
    pub verification_status: String,
    pub notes: Option<String>,
    pub external_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsMap {
    pub values: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEventDto {
    pub id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub workspace_path: Option<String>,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnassignedItem {
    pub id: String,
    pub workspace_path: Option<String>,
    pub conversation_id: Option<String>,
    pub reason: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnassignedQuery {
    /// When set, only rows relevant to this project (workspace map or conversation link).
    pub project_id: Option<String>,
    pub reason: Option<String>,
    pub search: Option<String>,
    /// all | with_conversation | without_conversation
    pub conversation: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnassignedPage {
    pub items: Vec<UnassignedItem>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildDirectory {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFinanceUpdate {
    /// Total contracted / agreed payment for the project.
    pub contracted_budget_minor: i64,
    pub recognized_revenue_minor: i64,
    /// Cash actually received.
    pub payments_received_minor: i64,
    /// Optional other cash expenses (0 = none).
    pub other_expenses_minor: i64,
    /// Optional project hourly labor rate in project-currency minor units.
    /// When set, overrides the global Settings salary rate for this project.
    #[serde(default)]
    pub labor_rate_override_minor: Option<i64>,
    /// ISO currency code (USD, PKR, …). All project amounts are treated in this currency.
    pub currency: String,
}
