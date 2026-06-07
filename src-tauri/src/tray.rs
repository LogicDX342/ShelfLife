use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Emitter, Manager};

use crate::storage::AppState;

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open shelflife", true, None::<&str>)?;
    let review = MenuItem::with_id(app, "review", "Review decaying files", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause watching", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume watching", true, None::<&str>)?;
    let reconcile = MenuItem::with_id(
        app,
        "reconcile",
        "Run reconciliation scan",
        true,
        None::<&str>,
    )?;
    let preferences = MenuItem::with_id(app, "preferences", "Preferences", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &review,
            &pause,
            &resume,
            &reconcile,
            &preferences,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("shelflife")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app_handle, event| match event.id().as_ref() {
            "open" => show_main_window(app_handle, None),
            "review" => show_main_window(app_handle, Some("/")),
            "preferences" => show_main_window(app_handle, Some("/settings")),
            "pause" => pause_watching(app_handle),
            "resume" => resume_watching(app_handle),
            "reconcile" => run_reconciliation(app_handle),
            "quit" => app_handle.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app_handle: &AppHandle, route: Option<&str>) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(route) = route {
            let script = format!("window.location.href = '{}';", route);
            let _ = window.eval(&script);
        }
    }
}

fn pause_watching(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    state.set_watching_paused(true);
    let watcher = state.watcher.clone();
    if let Ok(mut watcher) = watcher.lock() {
        *watcher = None;
    };
}

fn resume_watching(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    state.set_watching_paused(false);
    if let Err(error) = crate::engine::watcher::restart_watcher(
        &state,
        crate::commands::watcher_event_sink(app_handle.clone()),
    ) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn run_reconciliation(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    match crate::engine::reconcile_with_report(&state.db) {
        Ok(report) => {
            for path in &report.indexed {
                let _ = app_handle.emit("file_indexed", path);
            }
            for path in &report.updated {
                let _ = app_handle.emit("file_updated", path);
            }
            for path in &report.removed {
                let _ = app_handle.emit("file_removed", path);
            }
            let _ = app_handle.emit("reconciliation_completed", report);
        }
        Err(error) => {
            let _ = app_handle.emit("action_failed", error);
        }
    }
}
