import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const script = path.join(os.homedir(), ".cursor", "flowledger", "flowledger-hook.mjs");
const events = [
  "workspaceOpen",
  "sessionStart",
  "sessionEnd",
  "beforeSubmitPrompt",
  "preToolUse",
  "postToolUse",
  "subagentStart",
  "subagentStop",
  "afterFileEdit",
  "afterAgentResponse",
  "afterAgentThought",
  "stop",
];
const hooks = {};
for (const ev of events) {
  hooks[ev] = [{ command: `node "${script}" --event=${ev}` }];
}
const out = path.join(os.homedir(), ".cursor", "hooks.json");
fs.writeFileSync(out, JSON.stringify({ version: 1, hooks }, null, 2));
console.log("wrote", out, Object.keys(hooks).length, "events");
