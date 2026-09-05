use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::engine;
use crate::runtime::AppRuntime;

const PERIODIC_RECONCILIATION_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const RESOURCE_LIMIT_RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub fn run_async_reconciliation(app_handle: AppHandle, runtime: AppRuntime) {
    if runtime.reconciliation_active.swap(true, Ordering::SeqCst) {
        return;
    }

    let app_handle_clone = app_handle.clone();
    let runtime_clone = runtime.clone();

    let _ = app_handle.emit("reconciliation_started", ());

    tauri::async_runtime::spawn(async move {
        let app = app_handle_clone.clone();
        let progress_emitter = move |current: usize, total: usize| {
            let _ = app.emit("reconciliation_progress", (current, total));
        };

        let result = runtime_clone.run_exclusive_engine_operation(|db| {
            engine::reconciliation::reconcile_with_report_with_progress(db, Some(&progress_emitter))
        });

        runtime_clone
            .reconciliation_active
            .store(false, Ordering::SeqCst);

        match result {
            Ok(_) => {
                emit_reconciliation_completed(&app_handle_clone);
                runtime_clone.wake_rule_scheduler();
                crate::runtime::rule_scheduler::run_async_expired_rule_execution(
                    app_handle_clone,
                    runtime_clone,
                );
            }
            Err(error) => {
                let _ = app_handle_clone.emit("action_failed", error);
            }
        }
    });
}

pub fn start_periodic_reconciliation(app_handle: AppHandle, runtime: AppRuntime) {
    thread::spawn(move || {
        let mut wait_for = PERIODIC_RECONCILIATION_INTERVAL;

        loop {
            thread::sleep(wait_for);
            if runtime.is_watching_paused() {
                wait_for = PERIODIC_RECONCILIATION_INTERVAL;
                continue;
            }

            if crate::runtime::resource_limits::is_system_cpu_usage_high() {
                wait_for = RESOURCE_LIMIT_RETRY_INTERVAL;
                continue;
            }

            run_async_reconciliation(app_handle.clone(), runtime.clone());
            wait_for = PERIODIC_RECONCILIATION_INTERVAL;
        }
    });
}

pub fn emit_reconciliation_completed(app_handle: &AppHandle) {
    let _ = app_handle.emit("reconciliation_completed", ());
}

pub fn watcher_event_sink(
    app_handle: AppHandle,
    runtime: AppRuntime,
) -> engine::watcher::WatcherEventSink {
    Arc::new(move |event| match event {
        engine::watcher::WatcherEvent::PathsReady(paths) => {
            match runtime.run_exclusive_engine_operation(
                |db| -> Result<_, crate::models::AppError> {
                    let (_, arrival_candidates) =
                        engine::reconciliation::reconcile_paths(db, paths)?;
                    engine::execute_arrival_automatic_rules(db, &arrival_candidates)
                },
            ) {
                Ok(arrival_report) => {
                    emit_reconciliation_completed(&app_handle);
                    crate::runtime::rule_scheduler::emit_rule_execution_report(
                        &app_handle,
                        &arrival_report,
                    );
                    runtime.wake_rule_scheduler();
                    crate::runtime::rule_scheduler::run_async_expired_rule_execution(
                        app_handle.clone(),
                        runtime.clone(),
                    );
                }
                Err(error) => {
                    let _ = app_handle.emit("action_failed", error);
                }
            }
        }
        engine::watcher::WatcherEvent::Error(error) => {
            let _ = app_handle.emit("action_failed", error);
        }
    })
}
