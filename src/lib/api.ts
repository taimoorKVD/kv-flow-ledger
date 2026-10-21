import { invoke } from "@tauri-apps/api/core";

export type Project = {
  id: string;
  name: string;
  client?: string | null;
  description?: string | null;
  git_repository?: string | null;
  status: string;
  billing_model: string;
  contracted_budget_minor: number;
  currency: string;
  labor_rate_override_minor?: number | null;
  created_at: string;
  archived_at?: string | null;
  workspaces: string[];
};

export type WorkspacePathCheck = {
  valid: boolean;
  exists: boolean;
  is_dir: boolean;
  normalized: string;
  detail: string;
};

export type Task = {
  id: string;
  project_id: string;
  title: string;
  description?: string | null;
  status: string;
  github_issue?: string | null;
  completed_at?: string | null;
  created_at: string;
  combined_work_ms: number;
  agent_ms: number;
  developer_ms: number;
};

export type DashboardSummary = {
  project_id?: string | null;
  task_work_ms: number;
  ai_tokens?: number | null;
  ai_tokens_available: boolean;
  allocated_cost_minor: number;
  margin_pct?: number | null;
  labor_minor: number;
  ai_minor: number;
  other_minor: number;
  recognized_revenue_minor: number;
  profit_minor: number;
  currency: string;
  active_projects: number;
  active_tasks: number;
  unassigned_count: number;
  tracking: {
    hooks_installed: boolean;
    ingest_listening: boolean;
    ingest_port: number;
    active_project_id?: string | null;
    active_task_id?: string | null;
    active_conversation_id?: string | null;
    mode: string;
    last_event_at?: string | null;
    detail: string;
  };
};

export type ActivityEvent = {
  id: string;
  event_type: string;
  occurred_at: string;
  workspace_path?: string | null;
  conversation_id?: string | null;
  project_id?: string | null;
  task_id?: string | null;
  model?: string | null;
};

export type UsageRecord = {
  id: string;
  model?: string | null;
  occurred_at: string;
  project_id?: string | null;
  task_id?: string | null;
  total_tokens?: number | null;
  reported_cost_minor?: number | null;
  billing_source: string;
  verification_status: string;
  notes?: string | null;
};

export type ProjectFinancials = {
  currency: string;
  contracted_budget_minor: number;
  recognized_revenue_minor: number;
  payments_received_minor: number;
  labor_cost_minor: number;
  ai_cost_minor: number;
  other_expenses_minor: number;
  subscription_allocation_minor: number;
  total_cost_minor: number;
  economic_profit_minor: number;
  cash_flow_minor: number;
  profit_margin_pct?: number | null;
  budget_remaining_minor: number;
  cost_per_work_hour_minor?: number | null;
  hours_until_budget?: number | null;
};

const isTauri = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error("FlowLedger backend unavailable (open via Tauri desktop app)");
  }
  return invoke<T>(cmd, args);
}

export const api = {
  getAppInfo: () => call<Record<string, unknown>>("get_app_info"),
  listProjects: () => call<Project[]>("list_projects"),
  createProject: (input: Record<string, unknown>) => call<Project>("create_project", { input }),
  updateProject: (projectId: string, input: Record<string, unknown>) =>
    call<Project>("update_project", { projectId, input }),
  deleteProject: (projectId: string) => call<void>("delete_project", { projectId }),
  listTasks: (projectId?: string) =>
    call<Task[]>("list_tasks", { projectId: projectId ?? null }),
  createTask: (input: Record<string, unknown>) => call<Task>("create_task", { input }),
  updateTaskTitle: (taskId: string, title: string) =>
    call<Task>("update_task_title", { taskId, title }),
  updateProjectFinances: (
    projectId: string,
    update: {
      contracted_budget_minor: number;
      recognized_revenue_minor: number;
      payments_received_minor: number;
      other_expenses_minor: number;
      labor_rate_override_minor?: number | null;
      currency: string;
    }
  ) => call<void>("update_project_finances", { projectId, update }),
  validateWorkspacePath: (path: string) =>
    call<WorkspacePathCheck>("validate_workspace_path", { path }),
  listCurrencies: () =>
    call<Array<{ code: string; name: string; symbol: string }>>("list_currencies"),
  listExchangeRates: () =>
    call<
      Array<{
        id: string;
        base_currency: string;
        quote_currency: string;
        rate_numeric: string;
        effective_on: string;
      }>
    >("list_exchange_rates"),
  upsertExchangeRate: (input: {
    baseCurrency: string;
    quoteCurrency: string;
    rateNumeric: string;
    effectiveOn?: string;
  }) =>
    call<Record<string, string>>("upsert_exchange_rate", {
      baseCurrency: input.baseCurrency,
      quoteCurrency: input.quoteCurrency,
      rateNumeric: input.rateNumeric,
      effectiveOn: input.effectiveOn ?? null,
    }),
  getSettings: () => call<{ values: Record<string, string> }>("get_settings"),
  saveSettings: (values: Record<string, string>) => call<void>("save_settings", { values }),
  getDashboard: (projectId?: string) =>
    call<DashboardSummary>("get_dashboard", { projectId: projectId ?? null }),
  getProjectFinancials: (projectId: string) =>
    call<ProjectFinancials>("get_project_financials", { projectId }),
  listActivity: (projectId?: string) =>
    call<ActivityEvent[]>("list_activity", { projectId: projectId ?? null }),
  listUnassigned: () => call<Array<Record<string, string>>>("list_unassigned"),
  listUnassignedPage: (query: {
    projectId?: string | null;
    reason?: string | null;
    search?: string | null;
    conversation?: string | null;
    page?: number;
    pageSize?: number;
  }) =>
    call<{
      items: Array<Record<string, string>>;
      total: number;
      page: number;
      page_size: number;
      reasons: string[];
    }>("list_unassigned_page", {
      query: {
        project_id: query.projectId ?? null,
        reason: query.reason ?? null,
        search: query.search ?? null,
        conversation: query.conversation ?? null,
        page: query.page ?? 1,
        page_size: query.pageSize ?? 20,
      },
    }),
  dismissNoiseUnassigned: () => call<number>("dismiss_noise_unassigned"),
  assignUnassigned: (unassignedId: string, projectId: string, taskId?: string | null) =>
    call<void>("assign_unassigned", {
      unassignedId,
      projectId,
      taskId: taskId ?? null,
    }),
  addUsage: (input: Record<string, unknown>) => call<UsageRecord>("add_usage_record", { input }),
  importUsageCsv: (filename: string, csvText: string) =>
    call<{ inserted: number; skipped_duplicate_import: boolean }>("import_usage_csv", {
      filename,
      csvText,
    }),
  syncCursorUsage: (days?: number, projectId?: string | null) =>
    call<{
      fetched: number;
      inserted: number;
      updated: number;
      attributed: number;
      pages: number;
      detail: string;
      source: string;
    }>("sync_cursor_usage", {
      days: days ?? 30,
      projectId: projectId ?? null,
    }),
  testCursorApiKey: () => call<string>("test_cursor_api_key"),
  listUsage: (projectId?: string) =>
    call<UsageRecord[]>("list_usage", { projectId: projectId ?? null }),
  installHooks: () => call<Record<string, unknown>>("install_cursor_hooks"),
  uninstallHooks: () => call<Record<string, unknown>>("uninstall_cursor_hooks"),
  hooksStatus: () => call<Record<string, unknown>>("hooks_status"),
  ingestTestEvent: (event: Record<string, unknown>) =>
    call<Record<string, unknown>>("ingest_test_event", { event }),
};
