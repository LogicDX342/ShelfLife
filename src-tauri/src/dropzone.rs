use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Position, WebviewWindow, WebviewWindowBuilder,
};

use crate::engine::ShakeDetector;
use crate::models::AppError;

const DROPZONE_LABEL: &str = "dropzone";
const DROPZONE_WIDTH: i32 = 360;
const DROPZONE_HEIGHT: i32 = 300;
const CURSOR_OFFSET_X: i32 = 24;
const CURSOR_OFFSET_Y: i32 = -180;

static MONITOR: OnceLock<Mutex<Option<DropzoneMonitor>>> = OnceLock::new();
static AWAITING_DROP: AtomicBool = AtomicBool::new(false);

struct DropzoneMonitor {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

impl DropzoneMonitor {
    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.handle.join();
    }
}

pub fn sync_dropzone_monitor(app_handle: &AppHandle, enabled: bool) -> Result<(), AppError> {
    if enabled {
        start_monitor(app_handle.clone());
    } else {
        stop_monitor();
        destroy_dropzone(app_handle)?;
    }
    Ok(())
}

pub fn record_dropzone_drop() {
    AWAITING_DROP.store(false, Ordering::Relaxed);
}

fn start_monitor(app_handle: AppHandle) {
    let monitor = MONITOR.get_or_init(|| Mutex::new(None));
    let Ok(mut current) = monitor.lock() else {
        return;
    };
    if current.is_some() {
        return;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let handle = thread::spawn(move || monitor_cursor(app_handle, stop_thread));
    *current = Some(DropzoneMonitor { stop, handle });
}

fn stop_monitor() {
    let monitor = MONITOR.get_or_init(|| Mutex::new(None));
    let Ok(mut current) = monitor.lock() else {
        return;
    };
    if let Some(monitor) = current.take() {
        monitor.stop();
    }
}

#[cfg(target_os = "windows")]
fn monitor_cursor(app_handle: AppHandle, stop: Arc<AtomicBool>) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let started = Instant::now();
    let mut detector = ShakeDetector::new();
    let mut release_started_at: Option<Instant> = None;
    let mut dropzone_prepared = false;
    let mut dropzone_shown = false;

    while !stop.load(Ordering::Relaxed) {
        let left_button_down =
            unsafe { (GetAsyncKeyState(VK_LBUTTON as i32) & 0x8000u16 as i16) != 0 };
        let mut point = POINT { x: 0, y: 0 };
        let has_point = unsafe { GetCursorPos(&mut point) != 0 };
        let shake_eligible = left_button_down && shell_drag_image_is_visible();

        if shake_eligible && !dropzone_prepared && !dropzone_shown {
            prepare_dropzone(&app_handle);
            dropzone_prepared = true;
        }

        if left_button_down {
            release_started_at = None;
        } else if dropzone_prepared || AWAITING_DROP.load(Ordering::Relaxed) {
            let released_at = release_started_at.get_or_insert_with(Instant::now);
            if released_at.elapsed() >= Duration::from_millis(350) {
                let _ = destroy_dropzone(&app_handle);
                AWAITING_DROP.store(false, Ordering::Relaxed);
                detector.reset();
                release_started_at = None;
                dropzone_prepared = false;
                dropzone_shown = false;
            }
        } else if dropzone_shown {
            // The frontend accepted the drop and cleared AWAITING_DROP. Keep the
            // dropzone open while the user chooses what to do with the files.
            detector.reset();
            release_started_at = None;
            dropzone_shown = false;
        }

        if has_point
            && detector.update(
                shake_eligible,
                point.x,
                point.y,
                started.elapsed().as_millis() as u64,
            )
        {
            show_dropzone_near_cursor(&app_handle, point.x, point.y);
            dropzone_prepared = false;
            dropzone_shown = true;
        }

        thread::sleep(Duration::from_millis(16));
    }
}

#[cfg(target_os = "windows")]
fn shell_drag_image_is_visible() -> bool {
    use windows_sys::core::w;
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;

    // Explorer displays this top-level drag image only while a shell drag is active.
    unsafe { !FindWindowW(w!("SysDragImage"), std::ptr::null()).is_null() }
}

#[cfg(not(target_os = "windows"))]
fn monitor_cursor(_app_handle: AppHandle, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(250));
    }
}

fn prepare_dropzone(app_handle: &AppHandle) {
    let main_thread_handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        if let Err(error) = get_or_create_dropzone(&main_thread_handle) {
            let _ = main_thread_handle.emit("action_failed", error);
        }
    });
}

fn show_dropzone_near_cursor(app_handle: &AppHandle, cursor_x: i32, cursor_y: i32) {
    AWAITING_DROP.store(true, Ordering::Relaxed);
    let main_thread_handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        show_dropzone_on_main_thread(&main_thread_handle, cursor_x, cursor_y)
    });
}

fn show_dropzone_on_main_thread(app_handle: &AppHandle, cursor_x: i32, cursor_y: i32) {
    let window = match get_or_create_dropzone(app_handle) {
        Ok(window) => window,
        Err(error) => {
            let _ = app_handle.emit("action_failed", error);
            return;
        }
    };

    let (x, y) = clamp_to_monitor(
        &window,
        cursor_x + CURSOR_OFFSET_X,
        cursor_y + CURSOR_OFFSET_Y,
    );
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    let _ = window.show();
    let _ = window.set_focus();
}

fn get_or_create_dropzone(app_handle: &AppHandle) -> Result<WebviewWindow, AppError> {
    if let Some(window) = app_handle.get_webview_window(DROPZONE_LABEL) {
        return Ok(window);
    }

    let config = app_handle
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == DROPZONE_LABEL)
        .ok_or_else(|| {
            AppError::new(
                "WINDOW_CONFIG_MISSING",
                "The dropzone window configuration is missing.",
                false,
            )
        })?;

    WebviewWindowBuilder::from_config(app_handle, config)
        .and_then(WebviewWindowBuilder::build)
        .map_err(dropzone_window_error)
}

pub(crate) fn destroy_dropzone(app_handle: &AppHandle) -> Result<(), AppError> {
    if let Some(window) = app_handle.get_webview_window(DROPZONE_LABEL) {
        window.destroy().map_err(dropzone_window_error)?;
    }
    Ok(())
}

fn dropzone_window_error(error: tauri::Error) -> AppError {
    AppError::with_details(
        "ACTION_FAILED",
        "The dropzone window could not be updated.",
        true,
        error.to_string(),
    )
}

fn clamp_to_monitor(
    window: &tauri::WebviewWindow,
    preferred_x: i32,
    preferred_y: i32,
) -> (i32, i32) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return (preferred_x, preferred_y);
    };
    let position = monitor.position();
    let size = monitor.size();
    let min_x = position.x;
    let min_y = position.y;
    let max_x = min_x + size.width as i32 - DROPZONE_WIDTH;
    let max_y = min_y + size.height as i32 - DROPZONE_HEIGHT;

    (
        preferred_x.clamp(min_x, max_x.max(min_x)),
        preferred_y.clamp(min_y, max_y.max(min_y)),
    )
}
