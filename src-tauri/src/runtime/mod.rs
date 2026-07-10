pub mod diagnostics;
#[cfg(debug_assertions)]
pub mod mock;
pub mod reconciliation;
pub mod rule_scheduler;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tauri::{App, AppHandle, Manager};

use crate::engine;
use crate::models::AppError;
use crate::storage::{self, Database};

#[derive(Clone)]
pub struct AppRuntime {
    pub db: Arc<Database>,
    watcher: Arc<Mutex<Option<engine::watcher::ShelflifeDebouncer>>>,
    watching_paused: Arc<AtomicBool>,
    pub(crate) reconciliation_active: Arc<AtomicBool>,
    pub(crate) rule_execution_active: Arc<AtomicBool>,
    rule_scheduler_wake: Arc<(Mutex<bool>, Condvar)>,
}

impl AppRuntime {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            watcher: Arc::new(Mutex::new(None)),
            watching_paused: Arc::new(AtomicBool::new(false)),
            reconciliation_active: Arc::new(AtomicBool::new(false)),
            rule_execution_active: Arc::new(AtomicBool::new(false)),
            rule_scheduler_wake: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub fn is_watching_paused(&self) -> bool {
        self.watching_paused.load(Ordering::Relaxed)
    }

    pub fn is_reconciliation_active(&self) -> bool {
        self.reconciliation_active.load(Ordering::Relaxed)
    }

    pub fn wake_rule_scheduler(&self) {
        let (lock, wake) = &*self.rule_scheduler_wake;
        if let Ok(mut pending) = lock.lock() {
            *pending = true;
            wake.notify_all();
        }
    }

    pub(crate) fn wait_for_rule_scheduler_wake(&self, timeout: Option<Duration>) -> bool {
        let (lock, wake) = &*self.rule_scheduler_wake;
        let Ok(mut pending) = lock.lock() else {
            return false;
        };

        if *pending {
            *pending = false;
            return true;
        }

        match timeout {
            Some(timeout) => match wake.wait_timeout(pending, timeout) {
                Ok((mut guard, result)) => {
                    let was_woken = *guard || !result.timed_out();
                    *guard = false;
                    was_woken
                }
                Err(_) => false,
            },
            None => match wake.wait(pending) {
                Ok(mut guard) => {
                    let was_woken = *guard;
                    *guard = false;
                    was_woken
                }
                Err(_) => false,
            },
        }
    }

    pub fn sync_after_config_change(&self, app_handle: &AppHandle) -> Result<(), AppError> {
        let config = storage::get_config(&self.db).inspect_err(|error| {
            diagnostics::record_error("configuration", error);
        })?;
        crate::dropzone::sync_dropzone_monitor(app_handle, config.dropzone_enabled).inspect_err(
            |error| {
                diagnostics::record_error("dropzone", error);
            },
        )?;
        self.restart_watcher(app_handle)?;
        crate::tray::update_tray_icon(app_handle);
        reconciliation::run_async_reconciliation(app_handle.clone(), self.clone());
        diagnostics::record_event("runtime", "configuration synchronized");
        Ok(())
    }

    pub fn restart_watcher(&self, app_handle: &AppHandle) -> Result<(), AppError> {
        let config = storage::get_config(&self.db).inspect_err(|error| {
            diagnostics::record_error("watcher", error);
        })?;
        let mut watcher = self.watcher.lock().map_err(|_| {
            let error = AppError::new("WATCHER_ERROR", "Watcher state could not be locked.", true);
            diagnostics::record_error("watcher", &error);
            error
        })?;

        *watcher = None;
        if self.is_watching_paused() {
            diagnostics::record_event("watcher", "restart skipped while paused");
            return Ok(());
        }

        let started_watcher = engine::watcher::start_watcher(
            &config.watch_targets,
            reconciliation::watcher_event_sink(app_handle.clone(), self.clone()),
        )
        .inspect_err(|error| {
            diagnostics::record_error("watcher", error);
        })?;
        *watcher = Some(started_watcher);
        diagnostics::record_event("watcher", "started");
        Ok(())
    }

    pub fn pause_watching(&self, app_handle: &AppHandle) -> Result<(), AppError> {
        self.watching_paused.store(true, Ordering::Relaxed);
        self.wake_rule_scheduler();
        let mut watcher = self.watcher.lock().map_err(|_| {
            let error = AppError::new("WATCHER_ERROR", "Watcher state could not be locked.", true);
            diagnostics::record_error("watcher", &error);
            error
        })?;
        *watcher = None;
        crate::tray::update_tray_icon(app_handle);
        diagnostics::record_event("watcher", "paused");
        Ok(())
    }

    pub fn resume_watching(&self, app_handle: &AppHandle) -> Result<(), AppError> {
        self.watching_paused.store(false, Ordering::Relaxed);
        self.wake_rule_scheduler();
        self.restart_watcher(app_handle)?;
        crate::tray::update_tray_icon(app_handle);
        diagnostics::record_event("watcher", "resumed");
        Ok(())
    }
}

pub fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    diagnostics::record_event("runtime", "startup initiated");
    engine::executor::init_trash_support();
    let db = open_runtime_database(app).map_err(|error| {
        diagnostics::record_error("startup", &error);
        Box::new(error) as Box<dyn std::error::Error>
    })?;
    let runtime = AppRuntime::new(db);
    app.manage(runtime.clone());

    let config = storage::get_config(&runtime.db).map_err(|error| {
        diagnostics::record_error("startup", &error);
        Box::new(error) as Box<dyn std::error::Error>
    })?;
    crate::dropzone::sync_dropzone_monitor(app.handle(), config.dropzone_enabled).map_err(
        |error| {
            diagnostics::record_error("dropzone", &error);
            Box::new(error) as Box<dyn std::error::Error>
        },
    )?;
    crate::tray::setup(app).map_err(|error| {
        diagnostics::record_failure("tray", "TRAY_SETUP_FAILED", "System tray setup failed.");
        Box::new(error) as Box<dyn std::error::Error>
    })?;
    runtime
        .restart_watcher(app.handle())
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
    crate::tray::update_tray_icon(app.handle());
    reconciliation::run_async_reconciliation(app.handle().clone(), runtime.clone());
    reconciliation::start_periodic_reconciliation(app.handle().clone(), runtime.clone());
    rule_scheduler::start_periodic_rule_execution(app.handle().clone(), runtime);
    diagnostics::record_event("runtime", "startup completed");
    Ok(())
}

fn open_runtime_database(app: &App) -> Result<Arc<Database>, AppError> {
    #[cfg(debug_assertions)]
    if mock::is_mock_mode() {
        let workspace = mock::reset_mock_workspace(app).map_err(|error| {
            AppError::with_details(
                "DATABASE_ERROR",
                "Mock workspace could not be prepared.",
                false,
                error.to_string(),
            )
        })?;
        let db = storage::open_database(&workspace.db_path)?;
        mock::seed_mock_workspace(&db, &workspace).map_err(|error| {
            AppError::with_details(
                "DATABASE_ERROR",
                "Mock workspace could not be seeded.",
                false,
                error.to_string(),
            )
        })?;
        return Ok(db);
    }

    let app_data_dir = app.path().app_data_dir().map_err(|error| {
        AppError::with_details(
            "APP_DATA_PATH_ERROR",
            "The application data directory could not be accessed.",
            false,
            error.to_string(),
        )
    })?;
    storage::open_database(app_data_dir.join("shelflife.sqlite"))
}
