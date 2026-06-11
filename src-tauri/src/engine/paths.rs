use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn root_contains(root: &str, path: impl AsRef<Path>) -> bool {
    let Some(root) = normalize_configured_path(Path::new(root)) else {
        return false;
    };
    let Some(path) = normalize_configured_path(path.as_ref()) else {
        return false;
    };
    path.starts_with(root)
}

/// Returns true if `path` is a direct child of `root` (its parent == root).
/// Used for non-recursive watch targets where only top-level files are in scope.
pub fn root_directly_contains(root: &str, path: impl AsRef<Path>) -> bool {
    let Some(parent) = path.as_ref().parent() else {
        return false;
    };
    let Some(norm_parent) = normalize_configured_path(parent) else {
        return false;
    };
    let Some(norm_root) = normalize_configured_path(Path::new(root)) else {
        return false;
    };
    norm_parent == norm_root
}

pub fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
    let mut suffix = Vec::new();
    let mut cursor = path.to_path_buf();

    loop {
        if let Ok(canonical) = cursor.canonicalize() {
            let mut normalized = canonical;
            for component in suffix.iter().rev() {
                normalized.push(component);
            }
            return Some(normalized);
        }

        let component = cursor.file_name()?.to_os_string();
        if component == OsStr::new(".") || component == OsStr::new("..") {
            return None;
        }
        suffix.push(component);
        cursor = cursor.parent()?.to_path_buf();
    }
}
