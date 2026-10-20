mod commands;
mod db;
mod error;
mod finance;
mod http_server;
mod models;
mod services;
mod state;
mod time_engine;

use db::Db;
use services::settings;
use state::AppState;
use std::path::PathBuf;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from(".").join("flowledger-data"));
            std::fs::create_dir_all(&data_dir)?;
            let db_url = std::env::var("FLOWLEDGER_DATABASE_URL")
                .unwrap_or_else(|_| db::DEFAULT_DATABASE_URL.to_string());
            let db = Db::open(&db_url)?;

            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf());

            let port = settings::ingest_port(&db).unwrap_or(17321);
            let mut token = settings::get(&db, "ingest_token")?.unwrap_or_default();
            if token.is_empty() {
                token = uuid::Uuid::new_v4().to_string();
                let _ = settings::set(&db, "ingest_token", &token);
            }

            let state = AppState::new(db, resource_dir);
            let db_arc = state.db.clone();
            *state.ingest_listening.write() = true;
            http_server::start_ingest_server(db_arc, port, token);

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_projects,
            commands::create_project,
            commands::list_tasks,
            commands::create_task,
            commands::update_task_title,
            commands::update_project_finances,
            commands::get_settings,
            commands::save_settings,
            commands::get_dashboard,
            commands::get_project_financials,
            commands::list_activity,
            commands::list_unassigned,
            commands::assign_unassigned,
            commands::add_usage_record,
            commands::import_usage_csv,
            commands::list_usage,
            commands::install_cursor_hooks,
            commands::uninstall_cursor_hooks,
            commands::hooks_status,
            commands::ingest_test_event,
            commands::apply_idle_sweep,
            commands::list_currencies,
            commands::list_exchange_rates,
            commands::upsert_exchange_rate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FlowLedger");
}
