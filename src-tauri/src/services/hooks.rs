use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::services::settings;
use mysql::prelude::*;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn hooks_json_path() -> AppResult<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Message("home dir not found".into()))?;
    Ok(home.join(".cursor").join("hooks.json"))
}

fn hook_script_path(app_dir: &PathBuf) -> PathBuf {
    app_dir.join("hooks").join("flowledger-hook.mjs")
}

pub fn status(db: &Db) -> AppResult<HookInstallStatus> {
    let path = hooks_json_path()?;
    let installed = if path.exists() {
        let text = fs::read_to_string(&path).unwrap_or_default();
        text.contains("flowledger-hook")
    } else {
        false
    };
    // Keep ~/.cursor/flowledger/config.json aligned with current ingest port/token.
    if installed {
        let _ = sync_runtime_config(db);
    }
    let detail = settings::get(db, "hooks_install_detail")?.unwrap_or_default();
    Ok(HookInstallStatus {
        installed,
        hooks_path: path.display().to_string(),
        detail,
    })
}

/// Rewrite hook bridge config so auth/port never drift from FlowLedger settings.
pub fn sync_runtime_config(db: &Db) -> AppResult<()> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Message("home dir not found".into()))?;
    let dest_dir = home.join(".cursor").join("flowledger");
    fs::create_dir_all(&dest_dir)?;
    let port = settings::ingest_port(db)?;
    let token = settings::get(db, "ingest_token")?.unwrap_or_default();
    let cfg = json!({
        "port": port,
        "token": token,
        "endpoint": format!("http://127.0.0.1:{port}/hooks/ingest")
    });
    fs::write(dest_dir.join("config.json"), serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HookInstallStatus {
    pub installed: bool,
    pub hooks_path: String,
    pub detail: String,
}

/// Explicit, reversible install into ~/.cursor/hooks.json with backup.
pub fn install(db: &Db, resource_dir: PathBuf, ingest_port: u16, token: &str) -> AppResult<HookInstallStatus> {
    let cursor_dir = hooks_json_path()?
        .parent()
        .ok_or_else(|| AppError::Message("invalid hooks path".into()))?
        .to_path_buf();
    fs::create_dir_all(&cursor_dir)?;

    // Ensure hook script is available under ~/.cursor/flowledger/
    let dest_dir = cursor_dir.join("flowledger");
    fs::create_dir_all(&dest_dir)?;
    let script_src = hook_script_path(&resource_dir);
    let fallback_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("hooks")
        .join("flowledger-hook.mjs");
    let src = if script_src.exists() {
        script_src
    } else {
        fallback_src
    };
    if !src.exists() {
        return Err(AppError::Message(format!(
            "hook script missing at {}",
            src.display()
        )));
    }
    let script_dest = dest_dir.join("flowledger-hook.mjs");
    fs::copy(&src, &script_dest)?;

    // Write env config for the hook (keep in sync with app settings)
    let cfg = json!({
        "port": ingest_port,
        "token": token,
        "endpoint": format!("http://127.0.0.1:{ingest_port}/hooks/ingest")
    });
    fs::write(dest_dir.join("config.json"), serde_json::to_string_pretty(&cfg)?)?;
    let _ = sync_runtime_config(db);

    let hooks_path = hooks_json_path()?;
    let backup_path = cursor_dir.join("hooks.json.flowledger.bak");
    let mut root: Value = if hooks_path.exists() {
        let existing = fs::read_to_string(&hooks_path)?;
        if !backup_path.exists() {
            fs::write(&backup_path, &existing)?;
        }
        serde_json::from_str(&existing).unwrap_or_else(|_| json!({ "version": 1, "hooks": {} }))
    } else {
        json!({ "version": 1, "hooks": {} })
    };

    let script_quoted = format!("node \"{}\"", script_dest.display());

    let events = [
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

    let hooks = root
        .as_object_mut()
        .ok_or_else(|| AppError::Message("hooks.json root must be object".into()))?
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| AppError::Message("hooks key must be object".into()))?;

    for ev in events {
        // Pass --event so type is known even if stdin payload is incomplete on Windows.
        let entry = json!([{ "command": format!("{script_quoted} --event={ev}") }]);
        hooks_obj.insert(ev.to_string(), entry);
    }
    if root.get("version").is_none() {
        root.as_object_mut()
            .unwrap()
            .insert("version".into(), json!(1));
    }

    fs::write(&hooks_path, serde_json::to_string_pretty(&root)?)?;
    settings::set(db, "hooks_installed", "true")?;
    settings::set(
        db,
        "hooks_install_detail",
        &format!("Installed at {}", hooks_path.display()),
    )?;
    let now = chrono::Utc::now().to_rfc3339();
    let detail = hooks_path.display().to_string();
    db.with_conn(|conn| {
        conn.exec_drop(
            "INSERT INTO integration_status (id, integration, status, detail, updated_at)
             VALUES ('cursor_hooks', 'cursor_hooks', 'installed', ?, ?)
             ON DUPLICATE KEY UPDATE status = 'installed', detail = VALUES(detail), updated_at = VALUES(updated_at)",
            (&detail, &now),
        )?;
        Ok(())
    })?;

    status(db)
}

pub fn uninstall(db: &Db) -> AppResult<HookInstallStatus> {
    let hooks_path = hooks_json_path()?;
    let cursor_dir = hooks_path
        .parent()
        .ok_or_else(|| AppError::Message("invalid hooks path".into()))?
        .to_path_buf();
    let backup_path = cursor_dir.join("hooks.json.flowledger.bak");

    if hooks_path.exists() {
        let text = fs::read_to_string(&hooks_path)?;
        if text.contains("flowledger-hook") {
            if backup_path.exists() {
                fs::copy(&backup_path, &hooks_path)?;
            } else {
                // Remove only our commands by rewriting empty hooks object carefully
                let mut root: Value =
                    serde_json::from_str(&text).unwrap_or_else(|_| json!({ "version": 1, "hooks": {} }));
                if let Some(hooks) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    let keys: Vec<String> = hooks.keys().cloned().collect();
                    for k in keys {
                        if let Some(arr) = hooks.get(&k).and_then(|v| v.as_array()) {
                            let filtered: Vec<Value> = arr
                                .iter()
                                .filter(|item| {
                                    !item
                                        .get("command")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .contains("flowledger-hook")
                                })
                                .cloned()
                                .collect();
                            if filtered.is_empty() {
                                hooks.remove(&k);
                            } else {
                                hooks.insert(k, Value::Array(filtered));
                            }
                        }
                    }
                }
                fs::write(&hooks_path, serde_json::to_string_pretty(&root)?)?;
            }
        }
    }

    settings::set(db, "hooks_installed", "false")?;
    settings::set(db, "hooks_install_detail", "Uninstalled")?;
    let now = chrono::Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.exec_drop(
            "UPDATE integration_status SET status = 'not_installed', detail = 'Uninstalled', updated_at = ?
             WHERE integration = 'cursor_hooks'",
            (&now,),
        )?;
        Ok(())
    })?;
    status(db)
}
