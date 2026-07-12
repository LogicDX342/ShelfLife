use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager, Runtime, Window, WindowEvent};

use crate::models::{AppError, CloseBehavior};
use crate::runtime::AppRuntime;

#[derive(Clone, Debug, Deserialize)]
pub struct TrayLabels {
    pub open: String,
    pub review: String,
    pub pause: String,
    pub resume: String,
    pub reconcile: String,
    pub preferences: String,
    pub quit: String,
    pub tooltip: String,
    pub tooltip_paused: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            open: "Open ShelfLife".to_string(),
            review: "Review decaying files".to_string(),
            pause: "Pause watching".to_string(),
            resume: "Resume watching".to_string(),
            reconcile: "Run reconciliation scan".to_string(),
            preferences: "Preferences".to_string(),
            quit: "Quit".to_string(),
            tooltip: "ShelfLife".to_string(),
            tooltip_paused: "ShelfLife (Paused)".to_string(),
        }
    }
}

pub fn setup(app: &mut App) -> tauri::Result<()> {
    let labels = TrayLabels::default();
    app.manage(Mutex::new(labels.clone()));
    let menu = build_menu(app, &labels)?;

    let mut tray = TrayIconBuilder::with_id("shelflife")
        .menu(&menu)
        .tooltip(&labels.tooltip)
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

fn build_menu<R: Runtime, M: Manager<R>>(
    manager: &M,
    labels: &TrayLabels,
) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(manager, "open", &labels.open, true, None::<&str>)?;
    let review = MenuItem::with_id(manager, "review", &labels.review, true, None::<&str>)?;
    let pause = MenuItem::with_id(manager, "pause", &labels.pause, true, None::<&str>)?;
    let resume = MenuItem::with_id(manager, "resume", &labels.resume, true, None::<&str>)?;
    let reconcile = MenuItem::with_id(manager, "reconcile", &labels.reconcile, true, None::<&str>)?;
    let preferences = MenuItem::with_id(
        manager,
        "preferences",
        &labels.preferences,
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(manager, "quit", &labels.quit, true, None::<&str>)?;

    Menu::with_items(
        manager,
        &[
            &open,
            &review,
            &pause,
            &resume,
            &reconcile,
            &preferences,
            &quit,
        ],
    )
}

pub fn update_tray_labels(app_handle: &AppHandle, labels: TrayLabels) -> Result<(), AppError> {
    let labels_state = app_handle.state::<Mutex<TrayLabels>>();
    {
        let mut stored_labels = labels_state.lock().map_err(|_| {
            AppError::new(
                "ACTION_FAILED",
                "Tray labels state could not be locked.",
                true,
            )
        })?;
        *stored_labels = labels.clone();
    }

    let menu = build_menu(app_handle, &labels).map_err(|error| {
        AppError::with_details(
            "ACTION_FAILED",
            "Tray menu update failed.",
            true,
            error.to_string(),
        )
    })?;

    if let Some(tray) = app_handle.tray_by_id("shelflife") {
        tray.set_menu(Some(menu)).map_err(|error| {
            AppError::with_details(
                "ACTION_FAILED",
                "Tray menu update failed.",
                true,
                error.to_string(),
            )
        })?;
        update_tray_icon(app_handle);
    }

    Ok(())
}

pub fn hide_window_on_close(window: &Window, event: &WindowEvent) {
    if window.label() != "main" {
        return;
    }

    if let WindowEvent::CloseRequested { api, .. } = event {
        api.prevent_close();
        let runtime = window.state::<AppRuntime>();
        let behavior = crate::storage::get_config(&runtime.db)
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
                let runtime = window.state::<AppRuntime>();
                runtime.set_window_visible(false);
                let _ = window.hide();
            }
            CloseBehavior::Quit => window.app_handle().exit(0),
        }
    }
}

fn show_main_window(app_handle: &AppHandle, route: Option<&str>) {
    let runtime = app_handle.state::<AppRuntime>();
    runtime.set_window_visible(true);
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
    let runtime = app_handle.state::<AppRuntime>();
    if let Err(error) = runtime.pause_watching(app_handle) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn resume_watching(app_handle: &AppHandle) {
    let runtime = app_handle.state::<AppRuntime>();
    if let Err(error) = runtime.resume_watching(app_handle) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn run_reconciliation(app_handle: &AppHandle) {
    let runtime = app_handle.state::<AppRuntime>();
    crate::runtime::reconciliation::run_async_reconciliation(
        app_handle.clone(),
        runtime.inner().clone(),
    );
}

pub fn update_tray_icon(app_handle: &AppHandle) {
    let runtime = app_handle.state::<AppRuntime>();
    let is_paused = runtime.is_watching_paused();

    if let Some(tray) = app_handle.tray_by_id("shelflife") {
        let labels = app_handle
            .try_state::<Mutex<TrayLabels>>()
            .and_then(|labels| labels.lock().ok().map(|labels| labels.clone()))
            .unwrap_or_default();

        if let Some(icon) = app_handle.default_window_icon() {
            if is_paused {
                let gray_icon = to_grayscale(icon);
                let _ = tray.set_icon(Some(gray_icon));
                let _ = tray.set_tooltip(Some(labels.tooltip_paused));
            } else {
                let _ = tray.set_icon(Some(icon.clone()));
                let _ = tray.set_tooltip(Some(labels.tooltip));
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
