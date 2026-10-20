CREATE TABLE IF NOT EXISTS schema_migrations (
  version INT PRIMARY KEY,
  applied_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS application_settings (
  `key` VARCHAR(128) PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS projects (
  id VARCHAR(36) PRIMARY KEY,
  name VARCHAR(255) NOT NULL,
  client VARCHAR(255) NULL,
  description TEXT NULL,
  git_repository VARCHAR(512) NULL,
  status VARCHAR(64) NOT NULL DEFAULT 'active',
  billing_model VARCHAR(64) NOT NULL DEFAULT 'fixed_price',
  contracted_budget_minor BIGINT NOT NULL DEFAULT 0,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  labor_rate_override_minor BIGINT NULL,
  created_at VARCHAR(64) NOT NULL,
  archived_at VARCHAR(64) NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS project_workspaces (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NOT NULL,
  path VARCHAR(1024) NOT NULL,
  created_at VARCHAR(64) NOT NULL,
  UNIQUE KEY uq_workspace_path (path(768)),
  CONSTRAINT fk_ws_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS tasks (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NOT NULL,
  title VARCHAR(512) NOT NULL,
  description TEXT NULL,
  status VARCHAR(64) NOT NULL DEFAULT 'in_progress',
  github_issue VARCHAR(255) NULL,
  completed_at VARCHAR(64) NULL,
  created_at VARCHAR(64) NOT NULL,
  CONSTRAINT fk_task_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS cursor_conversations (
  id VARCHAR(36) PRIMARY KEY,
  conversation_id VARCHAR(255) NOT NULL,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  workspace_path VARCHAR(1024) NULL,
  first_seen_at VARCHAR(64) NOT NULL,
  last_seen_at VARCHAR(64) NOT NULL,
  UNIQUE KEY uq_conversation (conversation_id),
  CONSTRAINT fk_conv_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_conv_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS cursor_events (
  id VARCHAR(36) PRIMARY KEY,
  fingerprint VARCHAR(128) NOT NULL,
  event_type VARCHAR(128) NOT NULL,
  occurred_at VARCHAR(64) NOT NULL,
  received_at VARCHAR(64) NOT NULL,
  workspace_path VARCHAR(1024) NULL,
  conversation_id VARCHAR(255) NULL,
  generation_id VARCHAR(255) NULL,
  model VARCHAR(255) NULL,
  model_id VARCHAR(255) NULL,
  agent_state VARCHAR(64) NULL,
  source VARCHAR(64) NOT NULL DEFAULT 'cursor_hook',
  payload_json LONGTEXT NOT NULL,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  UNIQUE KEY uq_event_fp (fingerprint),
  KEY idx_cursor_events_occurred (occurred_at),
  KEY idx_cursor_events_conversation (conversation_id),
  CONSTRAINT fk_evt_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_evt_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS activity_sessions (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  conversation_id VARCHAR(255) NULL,
  workspace_path VARCHAR(1024) NULL,
  started_at VARCHAR(64) NOT NULL,
  ended_at VARCHAR(64) NULL,
  status VARCHAR(64) NOT NULL DEFAULT 'open',
  CONSTRAINT fk_sess_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_sess_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS activity_intervals (
  id VARCHAR(36) PRIMARY KEY,
  session_id VARCHAR(36) NULL,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  conversation_id VARCHAR(255) NULL,
  kind VARCHAR(64) NOT NULL,
  started_at VARCHAR(64) NOT NULL,
  ended_at VARCHAR(64) NULL,
  duration_ms BIGINT NULL,
  source_event_id VARCHAR(36) NULL,
  notes TEXT NULL,
  KEY idx_intervals_task (task_id, started_at),
  KEY idx_intervals_kind (kind),
  CONSTRAINT fk_int_session FOREIGN KEY (session_id) REFERENCES activity_sessions(id) ON DELETE SET NULL,
  CONSTRAINT fk_int_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_int_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
  CONSTRAINT fk_int_event FOREIGN KEY (source_event_id) REFERENCES cursor_events(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS task_attributions (
  id VARCHAR(36) PRIMARY KEY,
  target_type VARCHAR(64) NOT NULL,
  target_id VARCHAR(255) NOT NULL,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  source VARCHAR(64) NOT NULL,
  confidence VARCHAR(64) NOT NULL,
  created_at VARCHAR(64) NOT NULL,
  created_by VARCHAR(64) NOT NULL DEFAULT 'system',
  supersedes_id VARCHAR(36) NULL,
  CONSTRAINT fk_attr_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_attr_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS usage_imports (
  id VARCHAR(36) PRIMARY KEY,
  source VARCHAR(64) NOT NULL,
  filename VARCHAR(512) NULL,
  imported_at VARCHAR(64) NOT NULL,
  record_count BIGINT NOT NULL DEFAULT 0,
  content_hash VARCHAR(128) NOT NULL,
  UNIQUE KEY uq_import_hash (content_hash)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS ai_usage_records (
  id VARCHAR(36) PRIMARY KEY,
  import_id VARCHAR(36) NULL,
  provider VARCHAR(64) NOT NULL DEFAULT 'cursor',
  model VARCHAR(255) NULL,
  occurred_at VARCHAR(64) NOT NULL,
  project_id VARCHAR(36) NULL,
  task_id VARCHAR(36) NULL,
  conversation_id VARCHAR(255) NULL,
  input_tokens BIGINT NULL,
  output_tokens BIGINT NULL,
  cached_tokens BIGINT NULL,
  total_tokens BIGINT NULL,
  reported_cost_minor BIGINT NULL,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  billing_source VARCHAR(128) NOT NULL,
  verification_status VARCHAR(64) NOT NULL,
  attribution_method VARCHAR(64) NULL,
  external_key VARCHAR(255) NULL,
  notes TEXT NULL,
  UNIQUE KEY uq_usage_external (external_key),
  CONSTRAINT fk_usage_import FOREIGN KEY (import_id) REFERENCES usage_imports(id) ON DELETE SET NULL,
  CONSTRAINT fk_usage_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_usage_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS labor_rate_history (
  id VARCHAR(36) PRIMARY KEY,
  effective_from VARCHAR(64) NOT NULL,
  monthly_salary_minor BIGINT NOT NULL,
  monthly_hours DOUBLE NOT NULL,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  created_at VARCHAR(64) NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS project_expenses (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NOT NULL,
  task_id VARCHAR(36) NULL,
  category VARCHAR(128) NOT NULL,
  amount_minor BIGINT NOT NULL,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  occurred_at VARCHAR(64) NOT NULL,
  notes TEXT NULL,
  is_subscription_allocation TINYINT NOT NULL DEFAULT 0,
  CONSTRAINT fk_exp_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_exp_task FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS revenue_records (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NOT NULL,
  kind VARCHAR(64) NOT NULL,
  amount_minor BIGINT NOT NULL,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  recognized_at VARCHAR(64) NOT NULL,
  notes TEXT NULL,
  CONSTRAINT fk_rev_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS payments (
  id VARCHAR(36) PRIMARY KEY,
  project_id VARCHAR(36) NOT NULL,
  amount_minor BIGINT NOT NULL,
  currency VARCHAR(16) NOT NULL DEFAULT 'USD',
  received_at VARCHAR(64) NOT NULL,
  notes TEXT NULL,
  CONSTRAINT fk_pay_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS exchange_rates (
  id VARCHAR(36) PRIMARY KEY,
  base_currency VARCHAR(16) NOT NULL,
  quote_currency VARCHAR(16) NOT NULL,
  rate_numeric VARCHAR(64) NOT NULL,
  effective_on VARCHAR(32) NOT NULL,
  UNIQUE KEY uq_fx (base_currency, quote_currency, effective_on)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS integration_status (
  id VARCHAR(36) PRIMARY KEY,
  integration VARCHAR(128) NOT NULL,
  status VARCHAR(64) NOT NULL,
  detail TEXT NULL,
  updated_at VARCHAR(64) NOT NULL,
  UNIQUE KEY uq_integration (integration)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS unassigned_activity (
  id VARCHAR(36) PRIMARY KEY,
  event_id VARCHAR(36) NULL,
  workspace_path VARCHAR(1024) NULL,
  conversation_id VARCHAR(255) NULL,
  reason VARCHAR(255) NOT NULL,
  status VARCHAR(64) NOT NULL DEFAULT 'pending',
  created_at VARCHAR(64) NOT NULL,
  resolved_at VARCHAR(64) NULL,
  resolved_project_id VARCHAR(36) NULL,
  resolved_task_id VARCHAR(36) NULL,
  CONSTRAINT fk_un_event FOREIGN KEY (event_id) REFERENCES cursor_events(id) ON DELETE CASCADE,
  CONSTRAINT fk_un_project FOREIGN KEY (resolved_project_id) REFERENCES projects(id) ON DELETE SET NULL,
  CONSTRAINT fk_un_task FOREIGN KEY (resolved_task_id) REFERENCES tasks(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
