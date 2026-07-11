use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::engine::{self, RuleExecutionReport};
use crate::runtime::AppRuntime;

const MIN_AUTO_RULE_EXECUTION_INTERVAL: Duration = Duration::from_secs(5);

pub fn run_async_expired_rule_execution(app_handle: AppHandle, runtime: AppRuntime) {
    if runtime.rule_execution_active.swap(true, Ordering::SeqCst) {
        return;
    }

    let db = runtime.db.clone();
    let app_handle_clone = app_handle.clone();
    let runtime_clone = runtime.clone();

    tauri::async_runtime::spawn(async move {
        let result = runtime_clone
            .run_exclusive_engine_operation(|| engine::execute_expired_automatic_rules(&db));

        runtime_clone
            .rule_execution_active
            .store(false, Ordering::SeqCst);

        match result {
            Ok(report) => {
                emit_rule_execution_report(&app_handle_clone, &report);
                if !report.entries.is_empty() && report.failures.is_empty() {
                    runtime_clone.wake_rule_scheduler();
                }
            }
            Err(error) => {
                let _ = app_handle_clone.emit("action_failed", error);
            }
        }
    });
}

pub fn start_periodic_rule_execution(app_handle: AppHandle, runtime: AppRuntime) {
    thread::spawn(move || loop {
        if runtime.is_watching_paused() {
            runtime.wait_for_rule_scheduler_wake(None);
            continue;
        }

        let wait_for = match engine::next_automatic_rule_execution_delay(
            &runtime.db,
            MIN_AUTO_RULE_EXECUTION_INTERVAL,
        ) {
            Ok(delay) => delay,
            Err(error) => {
                let _ = app_handle.emit("action_failed", error);
                Some(MIN_AUTO_RULE_EXECUTION_INTERVAL)
            }
        };

        if runtime.wait_for_rule_scheduler_wake(wait_for) {
            continue;
        }

        if runtime.is_watching_paused() || wait_for.is_none() {
            continue;
        }

        run_async_expired_rule_execution(app_handle.clone(), runtime.clone());
    });
}

pub fn emit_rule_execution_report(app_handle: &AppHandle, report: &RuleExecutionReport) {
    for entry in &report.entries {
        let _ = app_handle.emit("audit_updated", entry);
        if !matches!(entry.undo_status, crate::models::UndoStatus::Failed { .. }) {
            let _ = app_handle.emit("action_completed", entry);
        }
    }

    for error in &report.failures {
        let _ = app_handle.emit("action_failed", error);
    }
}
