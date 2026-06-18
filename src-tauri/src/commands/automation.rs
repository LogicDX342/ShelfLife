use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::engine::{self, RuleExecutionReport};
use crate::storage::AppState;

const MIN_AUTO_RULE_EXECUTION_INTERVAL: Duration = Duration::from_secs(5);

pub fn run_async_expired_rule_execution(app_handle: AppHandle, state: AppState) {
    if state.rule_execution_active.swap(true, Ordering::SeqCst) {
        return;
    }
    if state.reconciliation_active.load(Ordering::Relaxed) {
        state.rule_execution_active.store(false, Ordering::SeqCst);
        return;
    }

    let db = state.db.clone();
    let app_handle_clone = app_handle.clone();
    let state_clone = state.clone();

    tauri::async_runtime::spawn(async move {
        let state_for_retry = state_clone.clone();
        let result = engine::execute_expired_automatic_rules(&db, |path, rule_id| {
            state_for_retry.automatic_rule_retry_after(path, rule_id)
        });

        state_clone
            .rule_execution_active
            .store(false, Ordering::SeqCst);

        match result {
            Ok(report) => {
                update_retry_state(&state_clone, &report);
                emit_rule_execution_report(&app_handle_clone, &report, &state_clone);
                state_clone.wake_rule_scheduler();
            }
            Err(error) => {
                let _ = app_handle_clone.emit("action_failed", error);
            }
        }
    });
}

pub fn start_periodic_rule_execution(app_handle: AppHandle, state: AppState) {
    thread::spawn(move || loop {
        if state.is_watching_paused() {
            state.wait_for_rule_scheduler_wake(None);
            continue;
        }

        let wait_for = match engine::next_automatic_rule_execution_delay(
            &state.db,
            MIN_AUTO_RULE_EXECUTION_INTERVAL,
            |path, rule_id| state.automatic_rule_retry_after(path, rule_id),
        ) {
            Ok(delay) => delay,
            Err(error) => {
                let _ = app_handle.emit("action_failed", error);
                Some(MIN_AUTO_RULE_EXECUTION_INTERVAL)
            }
        };

        if state.wait_for_rule_scheduler_wake(wait_for) {
            continue;
        }

        if state.is_watching_paused() || wait_for.is_none() {
            continue;
        }

        run_async_expired_rule_execution(app_handle.clone(), state.clone());
    });
}

fn update_retry_state(state: &AppState, report: &RuleExecutionReport) {
    let now = engine::now_seconds();
    for success in &report.successes {
        state.clear_automatic_rule_failure(&success.path, &success.rule_id);
    }
    for failure in &report.failures {
        state.record_automatic_rule_failure(&failure.path, &failure.rule_id, now);
    }
}

pub fn emit_rule_execution_report(
    app_handle: &AppHandle,
    report: &RuleExecutionReport,
    state: &AppState,
) {
    for entry in &report.entries {
        let _ = app_handle.emit("audit_updated", entry);
        if !matches!(entry.undo_status, crate::models::UndoStatus::Failed { .. }) {
            let _ = app_handle.emit("action_completed", entry);
        }
    }

    for failure in &report.failures {
        let mut error = failure.error.clone();
        let retry_after = state
            .automatic_rule_retry_after(&failure.path, &failure.rule_id)
            .map(|value| value.to_string())
            .unwrap_or_else(|| String::from("unknown"));
        error.details = Some(format!(
            "path={} rule_id={} next_retry_at={}",
            failure.path, failure.rule_id, retry_after
        ));
        let _ = app_handle.emit("action_failed", error);
    }
}
