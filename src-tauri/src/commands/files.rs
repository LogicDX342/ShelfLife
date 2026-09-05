use std::collections::BTreeMap;
use std::path::Path;

use tauri::State;

use crate::engine::paths::PathScope;
use crate::models::{AppError, RuleMatchExplanation, TrackedFile};
use crate::rules::CompiledRuleSet;
use crate::runtime::AppRuntime;
use crate::storage;

#[tauri::command]
pub async fn get_active_files(state: State<'_, AppRuntime>) -> Result<Vec<TrackedFile>, AppError> {
    state.with_database(storage::tracked::list_tracked_files)
}

#[tauri::command]
pub async fn explain_files(
    state: State<'_, AppRuntime>,
    paths: Vec<String>,
) -> Result<BTreeMap<String, Vec<RuleMatchExplanation>>, AppError> {
    state.with_database(|db| build_file_explanations(db, paths))
}

fn build_file_explanations(
    db: &storage::Database,
    mut paths: Vec<String>,
) -> Result<BTreeMap<String, Vec<RuleMatchExplanation>>, AppError> {
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }

    let config = storage::get_config(db)?;
    let scope = PathScope::new(&config);
    // A watch target can change before reconciliation removes its tracked files.
    paths.retain(|path| scope.is_in_enabled_watch_target(Path::new(path)));
    if paths.is_empty() {
        return Ok(BTreeMap::new());
    }

    let files = storage::tracked::list_tracked_files_by_paths(db, &paths)?;
    let rules = storage::rules::list_rules(db)?;
    let rule_set = CompiledRuleSet::compile(rules, &config)?;
    Ok(files
        .into_iter()
        .map(|file| {
            let explanations = rule_set.explain_file(&file);
            (file.path, explanations)
        })
        .collect())
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
    use crate::storage::test_util::{path_string, Fixture as StorageFixture};

    use super::{build_file_explanations, existing_directories};

    #[test]
    fn batch_explanations_return_found_requested_file_paths() {
        let fixture = StorageFixture::new("shelflife-file-explanations");
        let archive = fixture.write_watch_file("archive.zip", "archive");
        let notes = fixture.write_watch_file("notes.txt", "notes");
        let omitted = fixture.write_watch_file("omitted.zip", "omitted");
        fixture.save_config();
        fixture.track_file(&archive);
        fixture.track_file(&notes);
        fixture.track_file(&omitted);
        storage::rules::save_rule(&fixture.db, &fixture.rule()).expect("rule should save");

        let archive_path = path_string(&archive);
        let notes_path = path_string(&notes);
        let missing_path = path_string(&fixture.watch.join("missing.zip"));
        let explanations = build_file_explanations(
            &fixture.db,
            vec![
                archive_path.clone(),
                notes_path.clone(),
                missing_path.clone(),
            ],
        )
        .expect("batch explanations should build");

        assert_eq!(explanations.len(), 2);
        assert!(explanations[&archive_path][0].proposed_action.is_some());
        assert!(explanations[&notes_path][0].proposed_action.is_none());
        assert!(!explanations.contains_key(&missing_path));
        assert!(!explanations.contains_key(&path_string(&omitted)));
    }

    #[test]
    fn batch_explanations_skip_paths_outside_enabled_watch_targets() {
        let fixture = StorageFixture::new("shelflife-file-explanations-scope");
        let active = fixture.write_watch_file("active.zip", "active");
        let disabled = fixture.write_outside_file("disabled.zip", "disabled");
        let outside = fixture.write_file(&fixture.safe.join("outside.zip"), "outside");
        for path in [&active, &disabled, &outside] {
            fixture.track_file(path);
        }

        let mut disabled_target = fixture.watch_target(false);
        disabled_target.id = String::from("disabled");
        disabled_target.path = path_string(&fixture.outside);
        disabled_target.enabled = false;
        fixture.save_config_with_targets(vec![fixture.watch_target(false), disabled_target]);
        storage::rules::save_rule(&fixture.db, &fixture.rule()).expect("rule should save");

        let active_path = path_string(&active);
        let disabled_path = path_string(&disabled);
        let outside_path = path_string(&outside);
        let explanations = build_file_explanations(
            &fixture.db,
            vec![
                active_path.clone(),
                disabled_path.clone(),
                outside_path.clone(),
            ],
        )
        .expect("out-of-scope files should not fail the batch");

        assert_eq!(explanations.len(), 1);
        assert!(explanations[&active_path][0].proposed_action.is_some());
        assert!(!explanations.contains_key(&disabled_path));
        assert!(!explanations.contains_key(&outside_path));

        fixture.save_config_without_watch_targets();
        let explanations = build_file_explanations(&fixture.db, vec![active_path])
            .expect("a batch with no in-scope files should succeed");
        assert!(explanations.is_empty());
    }

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

        let files = storage::tracked::list_tracked_files(&db).expect("active files should load");

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
