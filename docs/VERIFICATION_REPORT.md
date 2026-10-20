# FlowLedger MVP Verification Report

**Date:** 2026-10-20  
**Remote:** https://github.com/taimoorKVD/kv-flow-ledger  
**Cursor audited:** 3.25.9  
**Design reference:** `prototypes/prototype_4.html`

## Environment

| Check | Result |
| --- | --- |
| OS | Windows 10.0.26200 |
| Node | v24.18.0 |
| Rust | 1.99.1 |
| WebView2 | Present |
| VS Build Tools (MSVC) | Installed (17.14 / MSVC 14.44) |

## Automated tests (executed)

| Suite | Command | Result |
| --- | --- | --- |
| Vitest format helpers | `npm test` | **4/4 passed** |
| TypeScript | `npx tsc -b` | **passed** |
| Vite production build | `npx vite build` | **passed** |
| Rust unit tests | `cargo test` (with vcvars64) | **15/15 passed** |

### Rust scenarios covered

1. Interval union: developer 10m + agent 15m + overlap 5m → combined 20m  
2. Identical overlap does not double-count  
3. Sleep reconciliation cuts at heartbeat  
4. Merge overlapping same-kind intervals  
5. Hourly labor rate from salary  
6. Economic profit vs cash flow distinction  
7. Margin N/A when revenue is zero  
8. Money format / FX convert helpers  
9. SQLite migration + default settings  
10. Project + workspace mapping persistence  
11. Duplicate hook event rejection  
12. Unknown workspace → unassigned queue  
13. Agent/developer ingest interval tracking  
14. Manual usage with null tokens stays Unavailable  
15. CSV import idempotency by content hash  

## Integration evidence

| Claim | Status |
| --- | --- |
| Official Hooks docs match wired events | Verified against https://cursor.com/docs/hooks |
| Hook installer/uninstaller implemented | Code complete (`Settings → Install hooks`) |
| Localhost ingest server | Implemented on `127.0.0.1:17321` |
| Live Cursor payload capture on this machine | **Not yet evidenced** — requires Install hooks + Cursor reload + real session |

## Honest limitations

1. Hooks do not expose Pro billed token charges; CSV/manual import required.  
2. Enterprise Admin/Analytics APIs intentionally unused.  
3. Developer idle without editor heartbeats uses threshold heuristics.  
4. `sessionEnd` / `stop` never imply task completion.  
5. Missing token fields display **Unavailable**, never fabricated zeros.

## Definition of done

- [x] Source on GitHub (`kv-flow-ledger`)
- [x] Architecture / feasibility / MVP / plan / financial / hooks docs
- [x] SQLite schema + migrations
- [x] Time union + finance engines with passing tests
- [x] Enterprise UI from prototype_4 (top nav, no permanent sidebar)
- [x] Hook bridge + reversible install
- [x] Frontend production build
- [x] Rust tests passing under MSVC
- [ ] Installer artifact from `tauri build` (run locally after this report)
- [ ] Live Cursor session evidence recorded

## How to run

```bash
npm install
npm run tauri dev
```

Then Settings → Install hooks → restart Cursor.
