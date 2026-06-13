use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, Window, WindowEvent};

use crate::models::CloseBehavior;
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

    let mut tray = TrayIconBuilder::with_id("shelflife")
        .menu(&menu)
        .tooltip("ShelfLife")
        .show_menu_on_left_click(false)
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
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle(), None);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;

    Ok(())
}

pub fn hide_window_on_close(window: &Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let state = window.state::<AppState>();
        let behavior = crate::storage::get_config(&state.db)
            .map(|config| config.close_behavior)
            .unwrap_or(CloseBehavior::Ask);

        match behavior {
            CloseBehavior::Ask => {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.emit("close_behavior_requested", ());
            }
            CloseBehavior::HideToTray => {
                let _ = window.hide();
            }
            CloseBehavior::Quit => window.app_handle().exit(0),
        }
    }
}

fn show_main_window(app_handle: &AppHandle, route: Option<&str>) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.unminimize();
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
