mod commands;
mod dropzone;
mod engine;
mod models;
mod rules;
mod runtime;
mod storage;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

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
        .setup(runtime::setup)
        .invoke_handler(tauri::generate_handler![
            commands::get_active_files,
            commands::explain_file,
            commands::preview_file,
            commands::open_file_location,
            commands::execute_triage_action,
            commands::execute_bulk_triage_action,
            commands::undo_audit_entry,
            commands::list_audit_entries,
            commands::preview_dropzone_files,
            commands::execute_dropzone_ingest,
            commands::execute_dropzone_rule_group,
            commands::hide_dropzone,
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
            commands::update_tray_labels,
            commands::select_directory,
            commands::open_external_url
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                #[cfg(debug_assertions)]
                if runtime::mock::is_mock_mode() {
                    runtime::mock::cleanup_mock_data(_app_handle);
                }
            }
        });
}
