# FlowLedger Architecture

**Version:** 0.1.0-mvp  
**Date:** 2026-10-20  
**Stack:** Tauri 2 · Rust · React · TypeScript · Vite · SQLite · Zustand · Apache ECharts  
**Remote:** `git@github.com:taimoorKVD/kv-flow-ledger.git`

## 1. Product summary

FlowLedger is a **local-first** desktop application that:

1. Receives official Cursor Hook events over a local HTTP ingest endpoint.
2. Attributes activity to projects and tasks using an explicit hierarchy.
3. Measures **task work time** as the union of developer-active and agent-execution intervals (no double counting).
4. Stores verified or user-supplied AI usage records (never fabricated).
5. Computes salary-based labor valuation, expenses, recognized revenue, cash receipts, and profitability with decimal-safe arithmetic.

## 2. Audit findings (Phase 0)

| Item | Verified result |
| --- | --- |
| Repository state | Greenfield local folder (`prototypes/` only). GitHub remote exists with LICENSE only. |
| Design reference | `prototypes/prototype_4.html` — horizontal top nav, dark enterprise UI, KPI panels, no permanent sidebar. |
| OS | Windows 10/11 (10.0.26200) |
| Node / npm | Node v24.18.0, npm 11.16.0 |
| Rust | rustc / cargo 1.99.1 (installed during setup) |
| Cursor | 3.25.9 (`%LOCALAPPDATA%\Programs\cursor\Cursor.exe`) |
| WebView2 | Present (Edge WebView2 runtime) |
| MSVC linker | Not present at audit time — Visual Studio Build Tools required for native Tauri builds |

## 3. High-level architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Cursor IDE (3.25.x)                         │
│  ~/.cursor/hooks.json  →  FlowLedger hook scripts (Node)        │
└────────────────────────────┬────────────────────────────────────┘
                             │ POST /hooks/ingest (localhost)
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Tauri 2 Desktop Shell                        │
│  ┌──────────────────────┐   ┌────────────────────────────────┐  │
│  │ React + TS Frontend  │◄─►│ Rust Core (SQLite + engines)   │  │
│  │ · Overview UI        │   │ · Event ingest + dedupe        │  │
│  │ · Projects / Tasks   │   │ · Attribution                  │  │
│  │ · Activity Timeline  │   │ · Time interval merger         │  │
│  │ · AI Usage           │   │ · Usage imports                │  │
│  │ · Financials         │   │ · Financial calculations       │  │
│  │ · Settings           │   │ · Hook install/uninstall       │  │
│  └──────────────────────┘   └───────────────┬────────────────┘  │
│           Tauri events (live updates)        │                  │
│                                              ▼                  │
│                                   SQLite (app data dir)         │
└─────────────────────────────────────────────────────────────────┘
```

## 4. Module map

| Module | Responsibility | Primary location |
| --- | --- | --- |
| A Dashboard | Aggregated KPIs, filters, charts | `src/` UI + `commands/dashboard` |
| B Projects/Tasks | CRUD, workspace maps, billing fields | `src-tauri/src/domain/` + UI |
| C Cursor Integration | Hook scripts, installer, HTTP ingest | `hooks/`, `src-tauri/src/ingest/` |
| D Time Engine | Interval union, idle, crash recovery | `src-tauri/src/time_engine/` |
| E Attribution | Hierarchy + unassigned review | `src-tauri/src/attribution/` |
| F AI Usage | Manual/CSV import, verification labels | `src-tauri/src/usage/` |
| G Financial Engine | Labor, P&L, cash vs economic profit | `src-tauri/src/finance/` |
| H Forecasting | Burn rate / runway when grounded | `src-tauri/src/finance/forecast.rs` |
| I Settings | Rates, idle, privacy, backup | `src-tauri/src/settings/` |

## 5. Data flow

1. Cursor fires a hook → script reads stdin JSON → POSTs sanitized metadata to `127.0.0.1:<port>/hooks/ingest`.
2. Rust validates schema, computes event fingerprint, inserts into `cursor_events` (immutable) inside a transaction.
3. Attribution service resolves project/task or marks `unassigned`.
4. Time engine opens/closes `activity_intervals` (developer / agent / overlap bookkeeping).
5. UI polls / receives Tauri events and re-renders dashboard metrics from SQL aggregates.
6. Usage CSV/manual entries land in `ai_usage_records` with `verification_status` and `import_id` for idempotency.
7. Financial engine reads rates, intervals, usage, expenses, and revenue tables; returns decimal strings (minor units stored as integers).

## 6. Database principles

- SQLite file in the OS app data directory (`flowledger.db`).
- Versioned SQL migrations under `src-tauri/migrations/`.
- Foreign keys ON; parameterized queries only.
- Raw events immutable; corrections via `task_attributions` / adjustment rows.
- Monetary amounts stored as **integer minor units** (e.g. cents) with ISO currency codes.
- Time: wall-clock UTC timestamps for history; monotonic clock for open intervals; sleep/wake reconciliation on resume.

## 7. Frontend architecture

- Vite + React 18 + TypeScript.
- Tailwind CSS with CSS variables matching `prototype_4.html` (dark first; light theme toggle).
- Horizontal top navigation only (no permanent sidebar).
- Zustand for UI session state (selected project, filters, connection status).
- Apache ECharts for trends and expense distribution.
- Accessible components (Radix / shadcn-style primitives).

## 8. Security & privacy

- Localhost-only ingest (bind `127.0.0.1`).
- Optional shared secret header for hook → app authentication.
- No prompt body storage by default (configurable; default off).
- No keystroke logging; no reading of Cursor private databases.
- Secrets (if any) via OS credential store; never logged.
- Export / backup / delete controls in Settings.

## 9. Future SaaS extension points

Without changing domain models:

- Replace SQLite with Postgres adapter.
- Add auth identity on `user_id` columns (nullable in MVP).
- Sync layer over encrypted remote store.
- Multi-seat labor rates and RBAC.

## 10. Non-goals (MVP)

- Cloud hosting, NestJS, Redis, remote auth.
- Fabricated token/cost numbers.
- Reverse-engineering Cursor internals.
- Treating contract budget as recognized revenue.
- Automatic silent attribution of ambiguous activity.
