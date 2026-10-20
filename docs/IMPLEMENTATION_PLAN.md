# FlowLedger Implementation Plan

**Execution order is mandatory.** Each phase: inspect → implement → test → report blockers.

## Phase 1 — Foundation
- Scaffold Tauri 2 + Vite React TS + Tailwind
- SQLite + migration runner
- App shell with top navigation matching `prototype_4.html`
- Health command + localhost ingest stub
- **Exit:** `cargo test` / `npm test` baseline green; app launches (or documented MSVC blocker)

## Phase 2 — Core Data
- Projects, workspaces, tasks, settings tables + commands
- Frontend CRUD pages
- Persistence round-trip tests

## Phase 3 — Cursor Tracking
- Hook scripts + hooks.json installer/uninstaller
- Event ingest API with dedupe
- Conversation mapping + unassigned queue
- Simulated + (when available) live hook evidence

## Phase 4 — Time Engine
- Interval open/close/merge (union)
- Idle rules + agent overlap
- Sleep/wake + crash recovery
- Deterministic clock tests for scenarios 1–14

## Phase 5 — AI Usage
- Manual + CSV importer
- Verification labels; subscription allocation
- Idempotent import tests

## Phase 6 — Financial Engine
- Labor rates, expenses, revenue, payments
- Economic vs cash metrics
- Multi-currency + exchange rates
- Forecast helpers with N/A when ungounded

## Phase 7 — Analytics UI
- Full dashboard from prototype_4
- Charts (ECharts), filters, detail views
- Live status from tracking engine

## Phase 8 — MVP Verification
- Integration/regression suite
- Privacy audit notes
- README, FINANCIAL_SPEC, VERIFICATION_REPORT
- Build/installer verification
- Push to `git@github.com:taimoorKVD/kv-flow-ledger.git` when requested

## Technical defaults (no blocking questions)

| Decision | Default |
| --- | --- |
| Package manager | npm |
| DB | SQLite via `rusqlite` + SQL migrations |
| Money | i64 minor units + currency code |
| Ingest port | `17321` (configurable) |
| Hook scope | User `~/.cursor/hooks.json` with backup |
| Idle default | 5 minutes |
| Prompt storage | Off |
| Git remote | `origin` → `git@github.com:taimoorKVD/kv-flow-ledger.git` |

## Known environment blockers

1. **MSVC Build Tools** required on Windows for Tauri/Rust linking — install if missing.
2. Live Cursor hook verification requires user approval to modify `~/.cursor/hooks.json`.
