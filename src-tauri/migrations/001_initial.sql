PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS application_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  client TEXT,
  description TEXT,
  git_repository TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  billing_model TEXT NOT NULL DEFAULT 'fixed_price',
  contracted_budget_minor INTEGER NOT NULL DEFAULT 0,
  currency TEXT NOT NULL DEFAULT 'USD',
  labor_rate_override_minor INTEGER,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS project_workspaces (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(path)
);

CREATE TABLE IF NOT EXISTS tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'in_progress',
  github_issue TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cursor_conversations (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL UNIQUE,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  workspace_path TEXT,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cursor_events (
  id TEXT PRIMARY KEY,
  fingerprint TEXT NOT NULL UNIQUE,
  event_type TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  received_at TEXT NOT NULL,
  workspace_path TEXT,
  conversation_id TEXT,
  generation_id TEXT,
  model TEXT,
  model_id TEXT,
  agent_state TEXT,
  source TEXT NOT NULL DEFAULT 'cursor_hook',
  payload_json TEXT NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_cursor_events_occurred ON cursor_events(occurred_at);
CREATE INDEX IF NOT EXISTS idx_cursor_events_conversation ON cursor_events(conversation_id);
CREATE INDEX IF NOT EXISTS idx_cursor_events_workspace ON cursor_events(workspace_path);

CREATE TABLE IF NOT EXISTS activity_sessions (
  id TEXT PRIMARY KEY,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  conversation_id TEXT,
  workspace_path TEXT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  status TEXT NOT NULL DEFAULT 'open'
);

CREATE TABLE IF NOT EXISTS activity_intervals (
  id TEXT PRIMARY KEY,
  session_id TEXT REFERENCES activity_sessions(id) ON DELETE SET NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  conversation_id TEXT,
  kind TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_ms INTEGER,
  source_event_id TEXT REFERENCES cursor_events(id) ON DELETE SET NULL,
  notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_intervals_task ON activity_intervals(task_id, started_at);
CREATE INDEX IF NOT EXISTS idx_intervals_kind ON activity_intervals(kind);

CREATE TABLE IF NOT EXISTS task_attributions (
  id TEXT PRIMARY KEY,
  target_type TEXT NOT NULL,
  target_id TEXT NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  source TEXT NOT NULL,
  confidence TEXT NOT NULL,
  created_at TEXT NOT NULL,
  created_by TEXT NOT NULL DEFAULT 'system',
  supersedes_id TEXT REFERENCES task_attributions(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS usage_imports (
  id TEXT PRIMARY KEY,
  source TEXT NOT NULL,
  filename TEXT,
  imported_at TEXT NOT NULL,
  record_count INTEGER NOT NULL DEFAULT 0,
  content_hash TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS ai_usage_records (
  id TEXT PRIMARY KEY,
  import_id TEXT REFERENCES usage_imports(id) ON DELETE SET NULL,
  provider TEXT NOT NULL DEFAULT 'cursor',
  model TEXT,
  occurred_at TEXT NOT NULL,
  project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  conversation_id TEXT,
  input_tokens INTEGER,
  output_tokens INTEGER,
  cached_tokens INTEGER,
  total_tokens INTEGER,
  reported_cost_minor INTEGER,
  currency TEXT NOT NULL DEFAULT 'USD',
  billing_source TEXT NOT NULL,
  verification_status TEXT NOT NULL,
  attribution_method TEXT,
  external_key TEXT,
  notes TEXT,
  UNIQUE(external_key)
);

CREATE TABLE IF NOT EXISTS labor_rate_history (
  id TEXT PRIMARY KEY,
  effective_from TEXT NOT NULL,
  monthly_salary_minor INTEGER NOT NULL,
  monthly_hours REAL NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS project_expenses (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
  category TEXT NOT NULL,
  amount_minor INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  occurred_at TEXT NOT NULL,
  notes TEXT,
  is_subscription_allocation INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS revenue_records (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  amount_minor INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  recognized_at TEXT NOT NULL,
  notes TEXT
);

CREATE TABLE IF NOT EXISTS payments (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  amount_minor INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  received_at TEXT NOT NULL,
  notes TEXT
);

CREATE TABLE IF NOT EXISTS exchange_rates (
  id TEXT PRIMARY KEY,
  base_currency TEXT NOT NULL,
  quote_currency TEXT NOT NULL,
  rate_numeric TEXT NOT NULL,
  effective_on TEXT NOT NULL,
  UNIQUE(base_currency, quote_currency, effective_on)
);

CREATE TABLE IF NOT EXISTS integration_status (
  id TEXT PRIMARY KEY,
  integration TEXT NOT NULL UNIQUE,
  status TEXT NOT NULL,
  detail TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS unassigned_activity (
  id TEXT PRIMARY KEY,
  event_id TEXT REFERENCES cursor_events(id) ON DELETE CASCADE,
  workspace_path TEXT,
  conversation_id TEXT,
  reason TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL,
  resolved_at TEXT,
  resolved_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
  resolved_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL
);
