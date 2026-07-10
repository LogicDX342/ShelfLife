use crate::models::AppError;

const DIAGNOSTIC_TARGET: &str = "shelflife";

/// Writes a lifecycle event that contains no user-managed data.
pub fn record_event(subsystem: &str, event: &str) {
    log::info!(target: DIAGNOSTIC_TARGET, "subsystem={subsystem} event={event}");
}

/// Writes a safe summary of a backend failure.
///
/// `AppError::details` may contain filesystem paths or other user-managed values, so it is
/// intentionally never included in diagnostic logs.
pub fn record_error(subsystem: &str, error: &AppError) {
    log::error!(target: DIAGNOSTIC_TARGET, "subsystem={subsystem} {}", format_error(error));
}

/// Writes a fixed failure summary for errors that are not represented by `AppError`.
pub fn record_failure(subsystem: &str, code: &str, message: &str) {
    log::error!(
        target: DIAGNOSTIC_TARGET,
        "subsystem={subsystem} code={code} recoverable=false message={message}"
    );
}

fn safe_message(code: &str) -> &'static str {
    match code {
        "DATABASE_ERROR" => "Database operation failed.",
        "WATCHER_ERROR" => "File watching failed.",
        "PERMISSION_DENIED" => "A filesystem permission was denied.",
        "DROPZONE_ERROR" => "Dropzone operation failed.",
        "RULE_EXECUTION_ERROR" => "Automatic rule execution failed.",
        "APP_DATA_PATH_ERROR" => "The application data directory could not be accessed.",
        _ => "A backend operation failed.",
    }
}

fn format_error(error: &AppError) -> String {
    format!(
        "code={} recoverable={} message={}",
        error.code,
        error.recoverable,
        safe_message(&error.code),
    )
}

#[cfg(test)]
mod tests {
    use super::{format_error, safe_message};
    use crate::models::AppError;

    #[test]
    fn error_details_are_not_used_for_diagnostic_messages() {
        let error = AppError::with_details(
            "DATABASE_ERROR",
            "Database operation failed.",
            true,
            r"C:\Users\Avery\Documents\private-budget.xlsx",
        );

        let record = format_error(&error);

        assert_eq!(
            record,
            "code=DATABASE_ERROR recoverable=true message=Database operation failed."
        );
        assert!(!record.contains("Avery"));
        assert!(!record.contains("private-budget"));
    }

    #[test]
    fn unknown_codes_use_a_generic_message() {
        assert_eq!(
            safe_message("UNRECOGNIZED_FAILURE"),
            "A backend operation failed."
        );
    }
}
