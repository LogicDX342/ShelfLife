use tauri::{AppHandle, Manager, State};

use crate::models::{AppConfig, AppError, CloseBehavior, WatchTarget};
use crate::runtime::AppRuntime;
use crate::storage;

#[tauri::command]
pub async fn get_config(state: State<'_, AppRuntime>) -> Result<AppConfig, AppError> {
    storage::get_config(&state.db)
}

#[tauri::command]
pub async fn is_reconciliation_active(state: State<'_, AppRuntime>) -> Result<bool, AppError> {
    Ok(state.is_reconciliation_active())
}

#[tauri::command]
pub async fn save_config(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    config: AppConfig,
) -> Result<AppConfig, AppError> {
    validate_config(&config)?;
    storage::save_config(&state.db, &config)?;
    state.sync_after_config_change(&app_handle)?;
    Ok(config)
}

#[tauri::command]
pub async fn resolve_close_request(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    behavior: CloseBehavior,
    remember: bool,
) -> Result<(), AppError> {
    if remember {
        let mut config = storage::get_config(&state.db)?;
        config.close_behavior = behavior.clone();
        storage::save_config(&state.db, &config)?;
    }

    match behavior {
        CloseBehavior::Ask | CloseBehavior::HideToTray => {
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        CloseBehavior::Quit => {
            app_handle.exit(0);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn update_watch_targets(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
    targets: Vec<WatchTarget>,
) -> Result<(), AppError> {
    let mut config = storage::get_config(&state.db)?;
    config.watch_targets = targets;
    validate_config(&config)?;
    storage::save_config(&state.db, &config)?;
    state.sync_after_config_change(&app_handle)?;
    Ok(())
}

#[tauri::command]
pub async fn run_reconciliation_scan(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    crate::runtime::reconciliation::run_async_reconciliation(app_handle, state.inner().clone());
    Ok(())
}

#[tauri::command]
pub async fn pause_watching(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    state.pause_watching(&app_handle)
}

#[tauri::command]
pub async fn resume_watching(
    app_handle: AppHandle,
    state: State<'_, AppRuntime>,
) -> Result<(), AppError> {
    state.resume_watching(&app_handle)
}

fn validate_config(config: &AppConfig) -> Result<(), AppError> {
    let safe_folder = validate_safe_folder(&config.safe_folder_path)?;

    let mut seen_roots = Vec::new();
    for target in config.watch_targets.iter().filter(|target| target.enabled) {
        let canonical = validate_watch_target_path(&target.path)?;
        if paths_overlap(&canonical, &safe_folder) {
            return Err(AppError::with_details(
                "PATH_OUT_OF_SCOPE",
                "Safe folder and watch targets cannot overlap. Choose separate folders.",
                true,
                target.path.clone(),
            ));
        }
        if seen_roots
            .iter()
            .any(|root: &std::path::PathBuf| root == &canonical)
        {
            return Err(AppError::with_details(
                "PATH_OUT_OF_SCOPE",
                "Duplicate watch target was rejected. No configuration was changed.",
                true,
                target.path.clone(),
            ));
        }
        if seen_roots.iter().any(|root| {
            canonical.starts_with(root.as_path()) || root.starts_with(canonical.as_path())
        }) {
            return Err(AppError::with_details(
                "PATH_OUT_OF_SCOPE",
                "Overlapping watch target was rejected. Remove the existing parent or child target first.",
                true,
                target.path.clone(),
            ));
        }
        seen_roots.push(canonical);
    }

    Ok(())
}

fn validate_watch_target_path(path: &str) -> Result<std::path::PathBuf, AppError> {
    let canonical = std::path::PathBuf::from(path)
        .canonicalize()
        .map_err(|error| {
            AppError::with_details(
                "PATH_NOT_FOUND",
                "Watch target must be an existing folder. No configuration was changed.",
                true,
                error.to_string(),
            )
        })?;

    if !canonical.is_dir() {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Watch target must be a folder. No configuration was changed.",
            true,
            canonical.to_string_lossy(),
        ));
    }
    if is_sensitive_root(&canonical) {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Sensitive system or home-root folder was rejected. No configuration was changed.",
            true,
            canonical.to_string_lossy(),
        ));
    }

    Ok(canonical)
}

fn validate_safe_folder(path: &str) -> Result<std::path::PathBuf, AppError> {
    let path = std::path::PathBuf::from(path);
    if path.as_os_str().is_empty() {
        return Err(AppError::new(
            "PATH_OUT_OF_SCOPE",
            "Safe folder path is required. No configuration was changed.",
            true,
        ));
    }

    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "PATH_OUT_OF_SCOPE",
            "Safe folder must have a parent folder. No configuration was changed.",
            true,
        )
    })?;

    if parent.exists() && is_sensitive_root(&parent.canonicalize()?) {
        return Err(AppError::with_details(
            "PATH_OUT_OF_SCOPE",
            "Safe folder parent cannot be a sensitive root. No configuration was changed.",
            true,
            parent.to_string_lossy(),
        ));
    }

    if path.exists() {
        return Ok(path.canonicalize()?);
    }

    if parent.exists() {
        let file_name = path.file_name().ok_or_else(|| {
            AppError::new(
                "PATH_OUT_OF_SCOPE",
                "Safe folder must have a folder name. No configuration was changed.",
                true,
            )
        })?;
        return Ok(parent.canonicalize()?.join(file_name));
    }

    Ok(path)
}

fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    path_contains_or_equals(left, right) || path_contains_or_equals(right, left)
}

fn path_contains_or_equals(parent: &std::path::Path, child: &std::path::Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let parent = normalize_windows_path(parent);
        let child = normalize_windows_path(child);
        let parent_prefix = format!("{parent}\\");
        child == parent || child.starts_with(&parent_prefix)
    }

    #[cfg(not(target_os = "windows"))]
    {
        child == parent || child.starts_with(parent)
    }
}

#[cfg(target_os = "windows")]
fn normalize_windows_path(path: &std::path::Path) -> String {
    let mut normalized = path.to_string_lossy().replace('/', "\\").to_lowercase();
    while normalized.ends_with('\\') && !is_windows_root_path(&normalized) {
        normalized.pop();
    }
    normalized
}

#[cfg(target_os = "windows")]
fn is_windows_root_path(path: &str) -> bool {
    path.len() == 3 && path.as_bytes()[1] == b':' && path.as_bytes()[2] == b'\\'
}

fn is_sensitive_root(path: &std::path::Path) -> bool {
    if let Ok(home) = std::env::var("USERPROFILE") {
        if path == std::path::Path::new(&home) {
            return true;
        }
    }

    let normalized = path.to_string_lossy().to_lowercase();
    normalized.ends_with("\\windows")
        || normalized.ends_with("\\program files")
        || normalized.ends_with("\\program files (x86)")
        || normalized.ends_with("\\programdata")
        || normalized.ends_with("\\appdata")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use crate::models::{AppConfig, WatchTarget};

    use super::validate_config;

    #[test]
    fn rejects_missing_watch_target() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("missing"),
                path: root.join("missing").to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: root.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        let error = validate_config(&config).expect_err("missing path should be rejected");
        assert_eq!(error.code, "PATH_NOT_FOUND");
    }

    #[test]
    fn accepts_existing_watch_target_and_safe_folder_parent() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let watch = root.join("watch");
        fs::create_dir_all(&watch).expect("watch dir should exist");
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("watch"),
                path: watch.to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: root.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        validate_config(&config).expect("config should validate");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_overlapping_enabled_watch_targets() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let watch = root.join("watch");
        let nested = watch.join("nested");
        fs::create_dir_all(&nested).expect("nested watch dir should exist");
        let config = AppConfig {
            watch_targets: vec![
                WatchTarget {
                    id: String::from("watch"),
                    path: watch.to_string_lossy().to_string(),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                },
                WatchTarget {
                    id: String::from("nested"),
                    path: nested.to_string_lossy().to_string(),
                    enabled: true,
                    recursive: false,
                    default_ttl_seconds: None,
                    ignore_patterns: Vec::new(),
                    include_hidden_patterns: Vec::new(),
                    rule_ids: Vec::new(),
                },
            ],
            safe_folder_path: root.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        let error = validate_config(&config).expect_err("overlap should be rejected");
        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_safe_folder_inside_watch_target() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let watch = root.join("watch");
        fs::create_dir_all(&watch).expect("watch dir should exist");
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("watch"),
                path: watch.to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: watch.join("safe").to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        let error = validate_config(&config).expect_err("safe folder overlap should be rejected");
        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_watch_target_inside_safe_folder() {
        let root = std::env::temp_dir().join(format!("shelflife-config-{}", Uuid::new_v4()));
        let safe = root.join("safe");
        let watch = safe.join("watch");
        fs::create_dir_all(&watch).expect("watch dir should exist");
        let config = AppConfig {
            watch_targets: vec![WatchTarget {
                id: String::from("watch"),
                path: watch.to_string_lossy().to_string(),
                enabled: true,
                recursive: false,
                default_ttl_seconds: None,
                ignore_patterns: Vec::new(),
                include_hidden_patterns: Vec::new(),
                rule_ids: Vec::new(),
            }],
            safe_folder_path: safe.to_string_lossy().to_string(),
            ..AppConfig::default()
        };

        let error = validate_config(&config).expect_err("watch target overlap should be rejected");
        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
        let _ = fs::remove_dir_all(root);
    }
}
