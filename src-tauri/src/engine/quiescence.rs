use std::path::Path;
use std::time::Duration;

use crate::models::AppError;

pub fn is_transient_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return true;
    };

    let lower = file_name.to_lowercase();
    lower.ends_with(".crdownload")
        || lower.ends_with(".part")
        || lower.ends_with(".tmp")
        || lower.ends_with(".download")
        || lower.ends_with(".swp")
        || lower.ends_with(".lock")
        || lower == ".ds_store"
        || lower == "thumbs.db"
        || lower.starts_with("~$")
}

#[allow(dead_code)]
pub async fn wait_until_stable(path: &Path, delay: Duration) -> Result<(), AppError> {
    if is_transient_path(path) {
        return Err(AppError::new(
            "FILE_NOT_STABLE",
            "The file is a transient or partial file. No file was changed.",
            true,
        ));
    }

    let before = std::fs::metadata(path)?;
    tokio::time::sleep(delay).await;
    let after = std::fs::metadata(path)?;

    if before.len() == after.len() && before.modified().ok() == after.modified().ok() {
        Ok(())
    } else {
        Err(AppError::new(
            "FILE_NOT_STABLE",
            "The file is still changing. No file was changed.",
            true,
        ))
    }
}
