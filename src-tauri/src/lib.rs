mod commands;
mod engine;
mod models;
mod rules;
mod storage;
mod tray;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .on_window_event(tray::hide_window_on_close)
        .setup(|app| {
            engine::executor::init_trash_support();
            let db_path = app.path().app_data_dir()?.join("shelflife.redb");
            let db = storage::open_database(db_path)
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            let state = storage::AppState::new(db);
            app.manage(state.clone());
            tray::setup(app).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            engine::watcher::restart_watcher(
                &state,
                commands::watcher_event_sink(app.handle().clone(), state.clone()),
            )
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            tray::update_tray_icon(app.handle());
            let db = state.db.clone();
            let app_handle = app.handle().clone();
            let state_clone = state.clone();
            std::thread::spawn(move || {
                state_clone
                    .reconciliation_active
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                let _ = app_handle.emit("reconciliation_started", ());

                let app = app_handle.clone();
                let progress_emitter = move |path: &str, current: usize, total: usize| {
                    let _ = app.emit(
                        "reconciliation_progress",
                        (path.to_string(), current, total),
                    );
                };

                let report = crate::engine::reconciliation::reconcile_with_report_with_progress(
                    &db,
                    Some(&progress_emitter),
                );

                state_clone
                    .reconciliation_active
                    .store(false, std::sync::atomic::Ordering::SeqCst);

                match report {
                    Ok(rep) => {
                        commands::emit_reconciliation_report(&app_handle, &rep);
                        state_clone.wake_rule_scheduler();
                        commands::run_async_expired_rule_execution(
                            app_handle.clone(),
                            state_clone.clone(),
                        );
                    }
                    Err(error) => {
                        let _ = app_handle.emit("action_failed", error);
                    }
                }
            });
            commands::start_periodic_reconciliation(app.handle().clone(), state.clone());
            commands::start_periodic_rule_execution(app.handle().clone(), state);
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
            commands::resolve_close_request,
            commands::update_watch_targets,
            commands::run_reconciliation_scan,
            commands::is_reconciliation_active,
            commands::pause_watching,
            commands::resume_watching,
            commands::select_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
