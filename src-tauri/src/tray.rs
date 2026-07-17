use std::sync::Mutex;

use serde::Deserialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::WebviewUrl;
use tauri::{
    App, AppHandle, Emitter, Manager, Runtime, WebviewWindow, WebviewWindowBuilder, Window,
    WindowEvent,
};

use crate::models::{AppError, CloseBehavior};
use crate::runtime::AppRuntime;

const MAIN_WINDOW_LABEL: &str = "main";

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
            "open" => request_main_window(app_handle, None),
            "review" => request_main_window(app_handle, Some("/")),
            "preferences" => request_main_window(app_handle, Some("/settings")),
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
                request_main_window(tray.app_handle(), None);
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
        let behavior = runtime
            .with_database(crate::storage::get_config)
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
                let _ = close_main_window_to_tray(window.app_handle());
            }
            CloseBehavior::Quit => window.app_handle().exit(0),
        }
    }
}

pub(crate) fn request_main_window(app_handle: &AppHandle, route: Option<&str>) {
    if let Err(error) = show_main_window(app_handle, route) {
        let _ = app_handle.emit("action_failed", error);
    }
}

fn show_main_window(
    app_handle: &AppHandle,
    route: Option<&str>,
) -> Result<WebviewWindow, AppError> {
    let existing_window = app_handle.get_webview_window(MAIN_WINDOW_LABEL);
    let was_existing = existing_window.is_some();
    let window = match existing_window {
        Some(window) => window,
        None => build_main_window(app_handle, route)?,
    };

    window.unminimize().map_err(window_error)?;
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)?;

    if let (true, Some(route)) = (was_existing, route) {
        let mut url = window.url().map_err(window_error)?;
        url.set_path(route);
        url.set_query(None);
        url.set_fragment(None);
        window.navigate(url).map_err(window_error)?;
    }

    app_handle.state::<AppRuntime>().set_window_visible(true);
    Ok(window)
}

fn build_main_window(
    app_handle: &AppHandle,
    route: Option<&str>,
) -> Result<WebviewWindow, AppError> {
    let mut config = app_handle
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == MAIN_WINDOW_LABEL)
        .cloned()
        .ok_or_else(|| {
            AppError::new(
                "WINDOW_CONFIG_MISSING",
                "The main window configuration is missing.",
                false,
            )
        })?;

    if let Some(route) = route {
        config.url = WebviewUrl::App(route.into());
    }

    WebviewWindowBuilder::from_config(app_handle, &config)
        .and_then(WebviewWindowBuilder::build)
        .map_err(window_error)
}

pub(crate) fn close_main_window_to_tray(app_handle: &AppHandle) -> Result<(), AppError> {
    app_handle.state::<AppRuntime>().set_window_visible(false);

    let dropzone_result = crate::dropzone::destroy_dropzone(app_handle);
    let main_result = app_handle
        .get_webview_window(MAIN_WINDOW_LABEL)
        .map(|window| window.destroy().map_err(window_error))
        .unwrap_or(Ok(()));

    main_result.and(dropzone_result)
}

fn window_error(error: tauri::Error) -> AppError {
    AppError::with_details(
        "WINDOW_ERROR",
        "The application window could not be updated.",
        true,
        error.to_string(),
    )
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
