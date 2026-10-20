# Cursor Integration Feasibility Report

**Audit date:** 2026-10-20  
**Installed Cursor version:** 3.25.9  
**Official docs reviewed:** https://cursor.com/docs/hooks  
**Plan constraint:** No Cursor plan upgrade required; Pro-accessible surfaces only.

## 1. Verified capabilities

### 1.1 Official Hooks (supported)

Cursor documents command-based hooks configured in:

- User: `~/.cursor/hooks.json`
- Project: `.cursor/hooks.json`

Relevant events for FlowLedger:

| Event | Feasible for tracking | Notes |
| --- | --- | --- |
| `workspaceOpen` | Yes | Provides `workspace_roots`, `cursor_version`, `user_email`. No conversation fields. |
| `sessionStart` | Yes | `session_id` (= conversation_id), `is_background_agent`, `composer_mode`. Fire-and-forget. |
| `sessionEnd` | Yes | Reason, duration_ms. **Does not mean task completed.** |
| `beforeSubmitPrompt` | Yes (metadata only) | Prompt text available; FlowLedger **does not store prompt content by default**. |
| `preToolUse` / `postToolUse` | Yes | Tool name, duration, model ids — signals agent activity. |
| `subagentStart` / `subagentStop` | Yes | Concurrent agent tracking. |
| `afterFileEdit` | Yes | Developer/agent edit activity signal. |
| `afterAgentResponse` | Yes | Agent still productive. |
| `stop` | Yes | Agent loop ended (`completed` / `aborted` / `error`). **Not task completion.** |
| `preCompact` | Partial | Includes `context_tokens` (context window size, **not billed usage**). |

### 1.2 Common hook payload fields (agent hooks)

Documented fields include:

- `conversation_id`, `generation_id`
- `model`, `model_id`, `model_params`
- `hook_event_name`, `cursor_version`
- `workspace_roots`, `user_email`
- `transcript_path` (path only; we do not read transcripts unless user opts in)

### 1.3 Pro-accessible usage data

| Source | Available on Pro | What FlowLedger will do |
| --- | --- | --- |
| Dashboard → Usage → Export CSV | Yes (user-initiated) | Idempotent CSV import with verification status `imported_csv` |
| Spending page (on-demand $) | Yes (manual observation) | Manual expense / subscription allocation entry |
| Admin / Analytics APIs | **Enterprise only** | Stub connector interface; not used in MVP |
| Hook payloads | Model + context window tokens only | Never treat as billed token consumption |
| Private Cursor DBs / scraping | Unsupported / disallowed | **Not used** |

**Important:** Hooks do **not** expose reliable per-request billed input/output token charges for Pro. Missing usage must display as **Unavailable**, never `0` implied as verified.

## 2. Limitations (honest)

1. Hooks report **agent lifecycle and workspace identity**, not complete OS-level idle detection.
2. Developer idle without agent activity requires a **local activity heuristic** (focus + optional OS idle APIs), not Cursor alone.
3. `sessionEnd` / `stop` ≠ task done.
4. Reopening a chat continues the same `conversation_id` when Cursor provides it — we key continuity on that id.
5. No Enterprise Admin API without plan upgrade — out of MVP scope.
6. Context `context_tokens` from `preCompact` is **not** metered billing.
7. Subscription fee must be allocated separately from metered/on-demand charges to avoid double counting.
8. Hook install mutates **user** `hooks.json` only when the user explicitly installs from Settings; uninstall restores prior content safely.

## 3. FlowLedger integration design (MVP)

```
Cursor Hook Script (Node, ship with app)
  → sanitize (drop prompt body unless opted in)
  → POST http://127.0.0.1:<FLOWLEDGER_PORT>/hooks/ingest
  → Authorization: Bearer <local shared secret>
  → always exit 0 / return {"continue": true} so tracking never blocks Cursor
```

Ingest guarantees:

- Event fingerprint dedupe (`event_type` + ids + timestamp bucket + payload hash)
- Idempotent inserts
- Attribution may leave activity **unassigned**
- Diagnostic logs exclude secrets and prompt text

## 4. What we will not do

- Read Cursor IndexedDB / LevelDB / proprietary stores
- Call undocumented internal HTTP endpoints
- Fabricate token counts from prompt length or agent duration
- Require Enterprise APIs
- Silently map ambiguous workspaces to projects

## 5. Evidence status

| Claim | Status |
| --- | --- |
| Hooks exist in docs for listed events | Verified against official docs |
| Cursor 3.25.9 installed locally | Verified via executable FileVersion |
| Live hook payload capture on this machine | Deferred to Phase 3 integration tests with install + real Cursor session |
| Pro CSV export schema | Documented publicly; importer will be tolerant and label unknown columns |

## 6. Conclusion

**Feasible MVP integration path:** Official Hooks for activity/agent lifecycle + workspace mapping + local time engine + manual/CSV usage import.

**Not feasible without Enterprise or fabrication:** Fully automatic verified dollar billing per request from hooks alone.
