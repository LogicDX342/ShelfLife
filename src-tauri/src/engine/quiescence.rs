use std::fs::Metadata;
use std::path::Path;
use std::time::Duration;

/// Returns true if the path is a transient/partial file that should never be indexed.
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

/// Returns true if the directory is a system-critical directory that must always be
/// skipped during recursive traversal. No user override is possible for these.
pub fn is_system_directory(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
        return false;
    };
    let lower = name.to_lowercase();
    lower == "$recycle.bin" || lower == "system volume information"
}

/// Returns true if the path is hidden and should be skipped by default.
/// On Windows, checks `FILE_ATTRIBUTE_HIDDEN`. On other platforms, checks for
/// a leading dot in the file or directory name.
#[cfg(target_os = "windows")]
pub fn is_hidden_path(_path: &Path, metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(target_os = "windows"))]
pub fn is_hidden_path(path: &Path, _metadata: &Metadata) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Block until the file's size and mtime are stable across two consecutive checks
/// separated by `delay`. Returns `true` if the file stabilised, `false` if it is
/// still changing after `max_attempts` retries or if the file disappeared.
pub fn wait_for_stability_sync(path: &Path, delay: Duration, max_attempts: u32) -> bool {
    if is_transient_path(path) {
        return false;
    }
    for _ in 0..max_attempts {
        let Ok(before) = std::fs::metadata(path) else {
            return false;
        };
        std::thread::sleep(delay);
        let Ok(after) = std::fs::metadata(path) else {
            return false;
        };
        if before.len() == after.len() && before.modified().ok() == after.modified().ok() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn transient_extensions_are_detected() {
        assert!(is_transient_path(Path::new("file.crdownload")));
        assert!(is_transient_path(Path::new("file.part")));
        assert!(is_transient_path(Path::new("file.tmp")));
        assert!(is_transient_path(Path::new("~$document.docx")));
        assert!(!is_transient_path(Path::new("report.pdf")));
    }

    #[test]
    fn system_directory_matches_recycle_bin_case_insensitive() {
        assert!(is_system_directory(Path::new("$RECYCLE.BIN")));
        assert!(is_system_directory(Path::new("$recycle.bin")));
        assert!(is_system_directory(Path::new("System Volume Information")));
        assert!(!is_system_directory(Path::new("Downloads")));
        assert!(!is_system_directory(Path::new(".git")));
    }
}
