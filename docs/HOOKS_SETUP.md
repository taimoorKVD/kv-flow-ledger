# Cursor Hooks Setup / Uninstall

## Install (recommended via app)

1. Launch FlowLedger.
2. Open **Settings**.
3. Click **Install hooks**.
4. Confirm `~/.cursor/hooks.json` references `flowledger-hook.mjs`.
5. Fully restart Cursor (or reload the window).

FlowLedger:

- Backs up the previous file to `~/.cursor/hooks.json.flowledger.bak` (first install only).
- Writes `~/.cursor/flowledger/config.json` with the local ingest endpoint and token.
- Registers observe-only hooks that always allow actions to continue.

## Manual uninstall

1. In FlowLedger Settings click **Uninstall hooks**, **or**
2. Restore `~/.cursor/hooks.json.flowledger.bak` over `hooks.json`, **or**
3. Remove any hook command entries containing `flowledger-hook`.

## Events wired

`workspaceOpen`, `sessionStart`, `sessionEnd`, `beforeSubmitPrompt`, `preToolUse`, `postToolUse`, `subagentStart`, `subagentStop`, `afterFileEdit`, `afterAgentResponse`, `stop`.

## Privacy

Prompt text, tool inputs/outputs, and attachments are stripped in the hook bridge before POST.
