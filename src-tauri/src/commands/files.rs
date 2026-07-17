use std::path::Path;

use tauri::State;

use crate::engine::paths::PathScope;
use crate::models::{AppError, RuleMatchExplanation, TrackedFile};
use crate::rules::CompiledRuleSet;
use crate::runtime::AppRuntime;
use crate::storage::{self, Database};

#[tauri::command]
pub async fn get_active_files(state: State<'_, AppRuntime>) -> Result<Vec<TrackedFile>, AppError> {
    state.with_database(active_files)
}

fn active_files(db: &Database) -> Result<Vec<TrackedFile>, AppError> {
    storage::tracked::list_tracked_files(db)
}

#[tauri::command]
pub async fn explain_file(
    state: State<'_, AppRuntime>,
    path: String,
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    validate_path_scope(&state, &path)?;
    let Some(file) = state.with_database(|db| storage::tracked::get_tracked_file(db, &path))?
    else {
        return Err(AppError::path_not_found(&path));
    };
    let config = state.with_database(storage::get_config)?;
    let rules = state.with_database(storage::rules::list_rules)?;
    let rule_set = CompiledRuleSet::compile(rules, &config)?;
    Ok(rule_set.explain_file(&file))
}

#[tauri::command]
pub async fn open_file_location(
    state: State<'_, AppRuntime>,
    path: String,
) -> Result<(), AppError> {
    validate_path_scope(&state, &path)?;
    let path_ref = Path::new(&path);
    if !path_ref.exists() {
        return Err(AppError::path_not_found(&path));
    }

    open_location(path_ref)
}

#[cfg(target_os = "windows")]
fn open_location(path: &Path) -> Result<(), AppError> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.to_string_lossy()))
        .spawn()
        .map(|_| ())
        .map_err(AppError::from)
}

#[cfg(not(target_os = "windows"))]
fn open_location(_path: &Path) -> Result<(), AppError> {
    Err(AppError::new(
        "ACTION_FAILED",
        "Open file location is only implemented for Windows v1.",
        true,
    ))
}

fn validate_path_scope(state: &State<'_, AppRuntime>, path: &str) -> Result<(), AppError> {
    let config = state.with_database(storage::get_config)?;
    PathScope::new(&config).ensure_watch_scope(Path::new(path))
}

#[tauri::command]
pub async fn filter_existing_directories(paths: Vec<String>) -> Result<Vec<String>, AppError> {
    Ok(existing_directories(paths))
}

fn existing_directories(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| Path::new(path).is_dir())
        .collect()
}

#[tauri::command]
pub async fn select_directory(
    title: Option<String>,
    default_path: Option<String>,
) -> Result<Option<String>, AppError> {
    let mut dialog = rfd::AsyncFileDialog::new();
    if let Some(title) = &title {
        dialog = dialog.set_title(title);
    }
    if let Some(ref path) = default_path {
        if !path.is_empty() {
            dialog = dialog.set_directory(path);
        }
    }
    let folder = dialog.pick_folder().await;
    Ok(folder.map(|f| f.path().to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use uuid::Uuid;

    use crate::models::{AppConfig, FileDecayState};
    use crate::storage;

    use super::{active_files, existing_directories};

    #[test]
    fn active_files_include_ignored_files_by_default() {
        let fixture = Fixture::new();
        let fresh = fixture.write("fresh.txt", "fresh");
        let ignored = fixture.write("ignored.txt", "ignored");
        let db =
            storage::open_database(fixture.root.join("test.sqlite")).expect("database should open");

        for (path, state) in [
            (&fresh, FileDecayState::Fresh),
            (&ignored, FileDecayState::ManuallyIgnored),
        ] {
            let metadata = fs::metadata(path).expect("metadata should exist");
            let config = AppConfig::default();
            let mut tracked =
                crate::engine::tracked_file_from_metadata(path, &metadata, None, &config, "");
            tracked.state = state;
            storage::tracked::upsert_tracked_file(&db, &tracked).expect("tracked file should save");
        }

        let files = active_files(&db).expect("active files should load");

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, fresh.to_string_lossy());
        assert_eq!(files[1].path, ignored.to_string_lossy());
    }

    #[test]
    fn directory_filter_preserves_only_existing_directories() {
        let fixture = Fixture::new();
        let directory = fixture.root.join("destination");
        fs::create_dir_all(&directory).expect("destination should exist");
        let file = fixture.write("file.txt", "body");
        let missing = fixture.root.join("missing");

        let filtered = existing_directories(vec![
            directory.to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
        ]);

        assert_eq!(filtered, vec![directory.to_string_lossy().into_owned()]);
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-files-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("fixture directory should be created");
            Self { root }
        }

        fn write(&self, name: &str, content: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::write(&path, content).expect("fixture file should be written");
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
