# FlowLedger MVP Scope

**Remote:** `git@github.com:taimoorKVD/kv-flow-ledger.git`  
**Design reference:** `prototypes/prototype_4.html`

## In scope

### Application shell
- Tauri 2 desktop app with React + TypeScript + Tailwind UI
- Horizontal top navigation: Overview, Projects & Tasks, Activity Timeline, AI Usage, Financials, Settings
- Light/dark themes; empty/loading/error/disconnected states
- Local SQLite persistence with versioned migrations

### Projects & tasks
- CRUD for projects and tasks with required commercial fields
- Workspace directory ↔ project mapping
- Billing models: fixed-price, hourly, internal
- Recognized revenue, payments received, and contract budget stored **separately**

### Cursor tracking
- Explicit install/uninstall of user-level Cursor hooks
- Localhost ingest of verified hook event types
- Conversation continuity via `conversation_id`
- Unassigned Activity Review for ambiguous mappings

### Time engine
- Developer-active, agent-execution, overlap, idle-excluded, unattributed
- Combined task time = **union** of intervals
- Idle threshold setting; agent execution continues during human idle
- Crash/restart and sleep/wake reconciliation
- Cumulative totals across sessions/days

### AI usage
- Manual entry and CSV import
- Verification status labels (`verified_import`, `manual`, `unavailable`, `estimated`)
- Subscription allocation separate from metered usage
- Idempotent imports

### Financials
- Hourly rate = monthly salary / monthly paid hours
- Labor cost from attributed task work time
- Economic profit vs cash flow distinction
- Integer minor-unit money math
- Burn rate / budget headroom / runway when rate is grounded
- No false completion forecasts without remaining-work estimates

### Tests & docs
- Rust unit tests for time merge + finance
- Vitest for UI/helpers
- Playwright smoke where practical
- Architecture, feasibility, financial specs, verification report

## Out of scope (MVP)

- PostgreSQL, Redis, NestJS, cloud hosting, remote auth
- Enterprise Admin/Analytics API connectors (interface stub only)
- Reading Cursor private databases
- Global keylogging / prompt archival (default off)
- Treating budget as earned revenue
- Fabricated token/cost figures
- Multi-user SaaS

## Definition of done (checklist)

- [ ] App builds and runs locally
- [ ] Projects/tasks persist across restart
- [ ] Real Cursor hook events ingestible when hooks installed
- [ ] Interval union correct under tests
- [ ] Idle excluded; agent counted without double count
- [ ] Unknown workspaces/tasks stay unassigned
- [ ] Missing tokens shown as Unavailable
- [ ] Financial formulas tested and reproducible
- [ ] Dashboard reflects SQLite aggregates
- [ ] Offline operation
- [ ] Limitations documented honestly
