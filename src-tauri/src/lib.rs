mod commands;
mod engine;
mod models;
mod rules;
mod storage;
mod tray;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let db_path = app.path().app_data_dir()?.join("shelflife.redb");
            let db = storage::open_database(db_path)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            let state = storage::AppState::new(db);
            app.manage(state.clone());
            tray::setup(app).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            engine::watcher::restart_watcher(
                &state,
                commands::watcher_event_sink(app.handle().clone()),
            )
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            let _ = engine::reconcile(&state.db);
            commands::start_periodic_reconciliation(app.handle().clone(), state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_active_files,
            commands::explain_file,
            commands::preview_file,
            commands::open_file_location,
            commands::execute_triage_action,
            commands::execute_bulk_triage_action,
            commands::undo_audit_entry,
            commands::list_audit_entries,
            commands::list_rules,
            commands::save_rule,
            commands::test_rule,
            commands::delete_rule,
            commands::get_config,
            commands::save_config,
            commands::update_watch_targets,
            commands::run_reconciliation_scan,
            commands::pause_watching,
            commands::resume_watching
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
