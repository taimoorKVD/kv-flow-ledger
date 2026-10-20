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

const home = os.homedir();
const configPath = path.join(home, ".cursor", "flowledger", "config.json");
const debugPath = path.join(home, ".cursor", "flowledger", "last-hook.json");

function loadConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath, "utf8"));
  } catch {
    return { port: 17321, token: "", endpoint: "http://127.0.0.1:17321/hooks/ingest" };
  }
}

/** Prefer sync stdin — async listeners can miss piped input on Windows. */
function readStdin() {
  try {
    if (process.stdin.isTTY) return "";
    const buf = fs.readFileSync(0);
    if (!buf || buf.length === 0) return "";
    // UTF-16 LE (common for some Windows pipes)
    if (buf.length >= 2 && buf[0] === 0xff && buf[1] === 0xfe) {
      return buf.toString("utf16le");
    }
    // UTF-8 BOM
    if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) {
      return buf.slice(3).toString("utf8");
    }
    // Heuristic: NUL every other byte → UTF-16 LE without BOM
    if (buf.length >= 8 && buf[1] === 0x00 && buf[3] === 0x00 && buf[5] === 0x00) {
      return buf.toString("utf16le");
    }
    return buf.toString("utf8");
  } catch {
    return "";
  }
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
          timeout: 2000,
        },
        (res) => {
          res.resume();
          resolve(res.statusCode || 0);
        }
      );
      req.on("error", () => resolve(0));
      req.on("timeout", () => {
        req.destroy();
        resolve(0);
      });
      req.write(data);
      req.end();
    } catch {
      resolve(0);
    }
  });
}

function sanitize(payload) {
  const clone = { ...payload };
  delete clone.prompt;
  delete clone.text;
  delete clone.attachments;
  delete clone.tool_output;
  delete clone.tool_input;
  return clone;
}

function firstString(...vals) {
  for (const v of vals) {
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  return null;
}

function asStringArray(v) {
  if (Array.isArray(v)) return v.filter((x) => typeof x === "string" && x.trim());
  if (typeof v === "string" && v.trim()) return [v.trim()];
  return null;
}

function argvEvent() {
  const flag = process.argv.find((a) => a.startsWith("--event="));
  return flag ? flag.slice(8) : null;
}

/** Cursor sometimes wraps/truncates stdin; recover a JSON object when possible. */
function parseHookPayload(raw) {
  const text = String(raw || "")
    .replace(/^\uFEFF/, "")
    .trim();
  if (!text) return { ok: false, payload: {}, raw_len: 0 };

  const attempts = [];
  attempts.push(text);
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start >= 0 && end > start) attempts.push(text.slice(start, end + 1));
  for (const line of text.split(/\r?\n/).reverse()) {
    const t = line.trim();
    if (t.startsWith("{") && t.endsWith("}")) attempts.push(t);
  }

  for (const candidate of attempts) {
    try {
      const payload = JSON.parse(candidate);
      if (payload && typeof payload === "object" && !Array.isArray(payload)) {
        return { ok: true, payload, raw_len: text.length };
      }
    } catch {
      /* try next */
    }
  }

  return {
    ok: false,
    payload: { _parse_error: true, _raw_len: text.length, _raw_head: text.slice(0, 240) },
    raw_len: text.length,
  };
}

const passthrough = {
  continue: true,
  permission: "allow",
};

async function main() {
  const raw = readStdin();
  const parsed = parseHookPayload(raw);
  const payload = parsed.payload;

  const cfg = loadConfig();
  const eventType =
    firstString(payload.hook_event_name, payload.hookEventName, argvEvent()) || "unknown";

  const workspaceRoots =
    asStringArray(payload.workspace_roots) ||
    asStringArray(payload.workspaceRoots) ||
    asStringArray(payload.workspace_root) ||
    asStringArray(payload.cwd) ||
    null;

  const conversationId = firstString(
    payload.conversation_id,
    payload.conversationId,
    payload.session_id,
    payload.sessionId,
    payload.composer_id,
    payload.composerId
  );

  const body = {
    event_type: eventType,
    occurred_at: new Date().toISOString(),
    workspace_roots: workspaceRoots,
    conversation_id: conversationId,
    generation_id: firstString(payload.generation_id, payload.generationId),
    model: firstString(payload.model),
    model_id: firstString(payload.model_id, payload.modelId),
    agent_state: null,
    cursor_version: firstString(payload.cursor_version, payload.cursorVersion),
    payload: sanitize(payload),
  };

  try {
    fs.mkdirSync(path.dirname(debugPath), { recursive: true });
    fs.writeFileSync(
      debugPath,
      JSON.stringify(
        {
          at: body.occurred_at,
          event_type: body.event_type,
          conversation_id: body.conversation_id,
          workspace_roots: body.workspace_roots,
          raw_len: parsed.raw_len,
          parse_ok: parsed.ok,
          keys: parsed.ok ? Object.keys(payload).slice(0, 40) : [],
          raw_head: parsed.ok ? undefined : payload._raw_head,
        },
        null,
        2
      )
    );
    if (!parsed.ok && raw) {
      fs.writeFileSync(
        path.join(path.dirname(debugPath), "last-hook-raw.txt"),
        String(raw).slice(0, 8000),
        "utf8"
      );
    }
  } catch {
    /* ignore debug write failures */
  }

  // Never ingest unparsable stdin — that flooded insufficient_signal rows.
  const useful =
    parsed.ok &&
    (body.conversation_id ||
      (body.workspace_roots && body.workspace_roots.length > 0) ||
      body.event_type !== "unknown");

  let httpStatus = null;
  if (useful) {
    httpStatus = await postJson(
      cfg.endpoint || `http://127.0.0.1:${cfg.port || 17321}/hooks/ingest`,
      cfg.token || "",
      body
    );
  }

  try {
    const prev = JSON.parse(fs.readFileSync(debugPath, "utf8"));
    prev.http_status = httpStatus;
    prev.posted = Boolean(useful);
    fs.writeFileSync(debugPath, JSON.stringify(prev, null, 2));
  } catch {
    /* ignore */
  }

  process.stdout.write(JSON.stringify(passthrough));
}

main().catch(() => {
  process.stdout.write(JSON.stringify(passthrough));
  process.exit(0);
});
