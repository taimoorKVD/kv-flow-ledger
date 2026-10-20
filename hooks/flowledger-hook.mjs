#!/usr/bin/env node
/**
 * FlowLedger Cursor Hook bridge.
 * Reads hook JSON from stdin, posts sanitized metadata to local ingest.
 * Always exits 0 and never blocks Cursor agent flow.
 */
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import http from "node:http";

const configPath = path.join(os.homedir(), ".cursor", "flowledger", "config.json");

function loadConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath, "utf8"));
  } catch {
    return { port: 17321, token: "", endpoint: "http://127.0.0.1:17321/hooks/ingest" };
  }
}

function readStdin() {
  return new Promise((resolve) => {
    const chunks = [];
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (c) => chunks.push(c));
    process.stdin.on("end", () => resolve(chunks.join("")));
    process.stdin.on("error", () => resolve(""));
  });
}

function postJson(endpoint, token, body) {
  return new Promise((resolve) => {
    try {
      const url = new URL(endpoint);
      const data = Buffer.from(JSON.stringify(body), "utf8");
      const req = http.request(
        {
          hostname: url.hostname,
          port: url.port,
          path: url.pathname,
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "Content-Length": data.length,
            ...(token ? { Authorization: `Bearer ${token}` } : {}),
          },
          timeout: 1500,
        },
        (res) => {
          res.resume();
          resolve();
        }
      );
      req.on("error", () => resolve());
      req.on("timeout", () => {
        req.destroy();
        resolve();
      });
      req.write(data);
      req.end();
    } catch {
      resolve();
    }
  });
}

function sanitize(payload) {
  const clone = { ...payload };
  // Never forward prompt bodies by default
  delete clone.prompt;
  delete clone.text;
  delete clone.attachments;
  delete clone.tool_output;
  delete clone.tool_input;
  return clone;
}

const passthrough = {
  continue: true,
  permission: "allow",
};

async function main() {
  const raw = await readStdin();
  let payload = {};
  try {
    payload = raw ? JSON.parse(raw) : {};
  } catch {
    payload = {};
  }

  const cfg = loadConfig();
  const eventType =
    payload.hook_event_name ||
    process.argv.find((a) => a.startsWith("--event="))?.slice(8) ||
    "unknown";

  const body = {
    event_type: eventType,
    occurred_at: new Date().toISOString(),
    workspace_roots: payload.workspace_roots || null,
    conversation_id: payload.conversation_id || payload.session_id || null,
    generation_id: payload.generation_id || null,
    model: payload.model || null,
    model_id: payload.model_id || null,
    agent_state: null,
    cursor_version: payload.cursor_version || null,
    payload: sanitize(payload),
  };

  await postJson(cfg.endpoint || `http://127.0.0.1:${cfg.port || 17321}/hooks/ingest`, cfg.token || "", body);

  // Respond with permissive JSON for hooks that read stdout
  process.stdout.write(JSON.stringify(passthrough));
}

main().catch(() => {
  process.stdout.write(JSON.stringify(passthrough));
  process.exit(0);
});
