# FlowLedger

Local-first desktop app for **Cursor developer activity**, **AI usage**, and **project profitability**.

Remote: `git@github.com:taimoorKVD/kv-flow-ledger.git`

## Features (MVP)

- Tauri 2 + React + TypeScript + SQLite
- Horizontal enterprise UI (see `prototypes/prototype_4.html`)
- Official Cursor Hooks ingest over localhost
- Task work time as **union** of developer + agent intervals (no double counting)
- Unassigned activity review
- Manual + CSV AI usage import (never fabricates tokens)
- Salary-based labor valuation, revenue, expenses, economic vs cash metrics

## Prerequisites (Windows)

- Node.js 20+
- Rust stable (`rustup`)
- Visual Studio 2022 Build Tools with **Desktop development with C++**
- WebView2 (usually preinstalled)
- Cursor IDE (for live hook events)

## Setup

```bash
npm install
npm run tauri dev
```

Production build:

```bash
npm run tauri build
```

## Tests

```bash
# Frontend unit tests
npm test

# Rust unit tests (time engine, finance, ingest)
cd src-tauri && cargo test
```

## Cursor hooks

1. Start FlowLedger (ingest listens on `127.0.0.1:17321` by default).
2. Open **Settings → Install hooks**.
3. This writes `~/.cursor/hooks.json` (with backup) and copies `hooks/flowledger-hook.mjs`.
4. Restart Cursor / reload window so hooks load.
5. Uninstall from Settings to restore the backup.

Hooks never block Cursor (`continue: true` / `permission: allow`) and do not store prompt bodies by default.

## AI usage / billing honesty

| Source | Status |
| --- | --- |
| Cursor Hooks | Activity + model ids; **not** billed token charges |
| Dashboard Usage CSV export | Supported via import |
| Enterprise Admin API | Out of MVP scope |
| Missing tokens | Displayed as **Unavailable** |

## Documentation

- `docs/ARCHITECTURE.md`
- `docs/CURSOR_INTEGRATION_FEASIBILITY.md`
- `docs/MVP_SCOPE.md`
- `docs/IMPLEMENTATION_PLAN.md`
- `docs/FINANCIAL_SPEC.md`

## Privacy

All data stays on disk in the OS app data directory. No source code, prompts, or chat transcripts are uploaded.
