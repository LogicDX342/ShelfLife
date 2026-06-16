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
    } else {
        update_tray_icon(app_handle);
    }
}

fn resume_watching(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    if let Err(error) = crate::engine::watcher::resume_watching(
        &state,
        crate::commands::watcher_event_sink(app_handle.clone()),
    ) {
        let _ = app_handle.emit("action_failed", error);
    } else {
        update_tray_icon(app_handle);
    }
}

fn run_reconciliation(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    crate::commands::config::run_async_reconciliation(app_handle.clone(), state.inner().clone());
}

pub fn update_tray_icon(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let is_watching = if state.is_watching_paused() {
        false
    } else {
        match crate::storage::get_config(&state.db) {
            Ok(config) => config.watch_targets.iter().any(|t| t.enabled),
            Err(_) => false,
        }
    };

    if let Some(tray) = app_handle.tray_by_id("shelflife") {
        if let Some(icon) = app_handle.default_window_icon() {
            if is_watching {
                let _ = tray.set_icon(Some(icon.clone()));
                let _ = tray.set_tooltip(Some("ShelfLife".to_string()));
            } else {
                let gray_icon = to_grayscale(icon);
                let _ = tray.set_icon(Some(gray_icon));
                let _ = tray.set_tooltip(Some("ShelfLife (Paused)".to_string()));
            }
        }
    }
}

fn to_grayscale(image: &tauri::image::Image) -> tauri::image::Image<'static> {
    let width = image.width();
    let height = image.height();
    let rgba = image.rgba();
    let mut gray_rgba = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0] as f32;
        let g = chunk[1] as f32;
        let b = chunk[2] as f32;
        let a = chunk[3];
        let gray = (0.299 * r + 0.587 * g + 0.114 * b).round() as u8;
        gray_rgba.push(gray);
        gray_rgba.push(gray);
        gray_rgba.push(gray);
        gray_rgba.push(a);
    }
    tauri::image::Image::new_owned(gray_rgba, width, height)
}
