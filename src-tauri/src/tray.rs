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
    if let Err(error) = crate::engine::watcher::pause_watching(&state) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn resume_watching(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    if let Err(error) = crate::engine::watcher::resume_watching(
        &state,
        crate::commands::watcher_event_sink(app_handle.clone()),
    ) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn run_reconciliation(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    crate::commands::config::run_async_reconciliation(app_handle.clone(), state.inner().clone());
}
