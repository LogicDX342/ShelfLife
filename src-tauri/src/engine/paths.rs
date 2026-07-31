use std::path::{Path, PathBuf};

use crate::models::{AppConfig, AppError, WatchTarget};

pub struct PathScope<'a> {
    config: &'a AppConfig,
}

impl<'a> PathScope<'a> {
    pub fn new(config: &'a AppConfig) -> Self {
        Self { config }
    }

    pub fn watch_target_for_path(&self, path: &Path) -> Option<&'a WatchTarget> {
        self.config
            .watch_targets
            .iter()
            .find(|target| target.enabled && target_contains_path(target, path))
    }

    pub fn is_in_enabled_watch_target(&self, path: &Path) -> bool {
        self.watch_target_for_path(path).is_some()
    }

    pub fn ensure_watch_scope(&self, path: &Path) -> Result<(), AppError> {
        if self.is_in_enabled_watch_target(path) {
            return Ok(());
        }

        Err(AppError::path_out_of_scope(path.to_string_lossy().as_ref()))
    }

    pub fn is_tracked_path_active(&self, path: &Path, watch_target_id: &str) -> bool {
        self.config.watch_targets.iter().any(|target| {
            target.enabled && target.id == watch_target_id && target_contains_path(target, path)
        })
    }

    pub fn validate_move_destination(&self, folder: &Path) -> Result<(), AppError> {
        if folder.as_os_str().is_empty() {
            return Err(AppError::new(
                "MOVE_DESTINATION_REQUIRED",
                "Move destination folder is required. No file was changed.",
                true,
            ));
        }

        if self.is_inside_enabled_watch_root(folder) {
            return Err(AppError::with_details(
                "MOVE_DESTINATION_WATCHED",
                "Move destination folder must be outside all enabled watch targets.",
                true,
                folder.to_string_lossy().to_string(),
            ));
        }

        Ok(())
    }

    fn is_inside_enabled_watch_root(&self, path: &Path) -> bool {
        self.config
            .watch_targets
            .iter()
            .filter(|target| target.enabled)
            .any(|target| root_contains(&target.path, path))
    }
}

pub fn validate_config_paths(config: &AppConfig) -> Result<(), AppError> {
    if let Some(destination) = config.default_move_destination.as_deref() {
        PathScope::new(config).validate_move_destination(Path::new(destination))?;
    }

    let mut seen_roots: Vec<PathBuf> = Vec::new();
    for target in config.watch_targets.iter().filter(|target| target.enabled) {
        let canonical = validate_watch_target_path(&target.path)?;
        if seen_roots
            .iter()
            .any(|root| paths_overlap(root, &canonical))
        {
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

fn target_contains_path(target: &WatchTarget, path: &Path) -> bool {
    let Some(norm_root) = normalize_configured_path(Path::new(&target.path)) else {
        return false;
    };
    let Some(norm_path) = normalize_configured_path(path) else {
        return false;
    };
    if target.recursive {
        path_contains_or_equals(&norm_root, &norm_path)
    } else {
        path_equals(&norm_root, &norm_path)
            || norm_path
                .parent()
                .is_some_and(|p| path_equals(&norm_root, p))
    }
}

pub(crate) fn root_contains(root: &str, path: impl AsRef<Path>) -> bool {
    let Some(root) = normalize_configured_path(Path::new(root)) else {
        return false;
    };
    let Some(path) = normalize_configured_path(path.as_ref()) else {
        return false;
    };
    path_contains_or_equals(&root, &path)
}

fn normalize_configured_path(path: &Path) -> Option<PathBuf> {
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
        suffix.push(component);
        cursor = cursor.parent()?.to_path_buf();
    }
}

fn validate_watch_target_path(path: &str) -> Result<PathBuf, AppError> {
    let canonical = PathBuf::from(path).canonicalize().map_err(|error| {
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

fn paths_overlap(left: &Path, right: &Path) -> bool {
    path_contains_or_equals(left, right) || path_contains_or_equals(right, left)
}

fn path_contains_or_equals(parent: &Path, child: &Path) -> bool {
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

fn path_equals(left: &Path, right: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        normalize_windows_path(left) == normalize_windows_path(right)
    }

    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}

#[cfg(target_os = "windows")]
fn normalize_windows_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('/', "\\").to_lowercase();
    while normalized.ends_with('\\') {
        if normalized.len() == 3 && normalized.as_bytes()[1] == b':' {
            break;
        }
        normalized.pop();
    }
    normalized
}

fn is_sensitive_root(path: &Path) -> bool {
    if let Ok(home) = std::env::var("USERPROFILE") {
        if path_equals(path, Path::new(&home)) {
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
    use std::path::{Path, PathBuf};

    use uuid::Uuid;

    use crate::models::{AppConfig, WatchTarget};

    use super::{validate_config_paths, PathScope};

    #[test]
    fn rejects_missing_watch_target() {
        let root = temp_root("shelflife-config");
        let config = config_with_target(
            &root,
            WatchTarget {
                id: String::from("missing"),
                path: path_string(&root.join("missing")),
                enabled: true,
                recursive: false,
                ignore_patterns: Vec::new(),
            },
        );

        let error = validate_config_paths(&config).expect_err("missing path should be rejected");

        assert_eq!(error.code, "PATH_NOT_FOUND");
    }

    #[test]
    fn accepts_existing_watch_target_without_default_move_destination() {
        let fixture = Fixture::new();
        let config = config_with_target(&fixture.root, fixture.watch_target(false));

        validate_config_paths(&config).expect("config should validate");
    }

    #[test]
    fn rejects_overlapping_enabled_watch_targets() {
        let fixture = Fixture::new();
        let nested = fixture.watch.join("nested");
        fs::create_dir_all(&nested).expect("nested watch dir should exist");
        let mut config = config_with_target(&fixture.root, fixture.watch_target(false));
        config.watch_targets.push(WatchTarget {
            id: String::from("nested"),
            path: path_string(&nested),
            enabled: true,
            recursive: false,
            ignore_patterns: Vec::new(),
        });

        let error = validate_config_paths(&config).expect_err("overlap should be rejected");

        assert_eq!(error.code, "PATH_OUT_OF_SCOPE");
    }

    #[test]
    fn rejects_default_move_destination_inside_watch_target() {
        let fixture = Fixture::new();
        let mut config = config_with_target(&fixture.root, fixture.watch_target(false));
        config.default_move_destination = Some(path_string(&fixture.watch.join("sorted")));

        let error = validate_config_paths(&config)
            .expect_err("default destination inside a watch target should be rejected");

        assert_eq!(error.code, "MOVE_DESTINATION_WATCHED");
    }

    #[test]
    fn accepts_default_move_destination_containing_watch_target() {
        let fixture = Fixture::new();
        let mut config = config_with_target(&fixture.root, fixture.watch_target(false));
        config.default_move_destination = Some(path_string(&fixture.root));

        validate_config_paths(&config)
            .expect("normal move validation allows a destination containing a watch target");
    }

    #[test]
    fn recursive_targets_include_descendants_and_direct_targets_do_not() {
        let fixture = Fixture::new();
        let subdir = fixture.watch.join("sub");
        fs::create_dir_all(&subdir).expect("subdir should be created");
        let nested = subdir.join("nested.txt");

        let direct_config = config_with_target(&fixture.root, fixture.watch_target(false));
        let recursive_config = config_with_target(&fixture.root, fixture.watch_target(true));

        assert!(PathScope::new(&direct_config).is_in_enabled_watch_target(&fixture.watch));
        assert!(PathScope::new(&direct_config)
            .is_in_enabled_watch_target(&fixture.watch.join("root.txt")));
        assert!(!PathScope::new(&direct_config).is_in_enabled_watch_target(&nested));
        assert!(PathScope::new(&recursive_config).is_in_enabled_watch_target(&nested));
    }

    #[test]
    fn containment_does_not_match_partial_sibling_names() {
        let fixture = Fixture::new();
        let config = config_with_target(&fixture.root, fixture.watch_target(true));
        let sibling = fixture.root.join("watch-other").join("file.txt");

        assert!(!PathScope::new(&config).is_in_enabled_watch_target(&sibling));
    }

    #[test]
    fn uncreated_child_paths_normalize_through_existing_parent() {
        let fixture = Fixture::new();
        let config = config_with_target(&fixture.root, fixture.watch_target(true));
        let uncreated_child = fixture.watch.join("future").join("file.txt");

        assert!(PathScope::new(&config).is_in_enabled_watch_target(&uncreated_child));
    }

    #[test]
    fn move_destination_rejects_enabled_watch_target_scope() {
        let fixture = Fixture::new();
        let config = config_with_target(&fixture.root, fixture.watch_target(false));
        let error = PathScope::new(&config)
            .validate_move_destination(&fixture.watch.join("archive"))
            .expect_err("watch destination should be rejected");

        assert_eq!(error.code, "MOVE_DESTINATION_WATCHED");
    }

    #[test]
    fn move_destination_rejects_nested_folder_inside_non_recursive_watch_root() {
        let fixture = Fixture::new();
        let config = config_with_target(&fixture.root, fixture.watch_target(false));
        let error = PathScope::new(&config)
            .validate_move_destination(&fixture.watch.join("archive").join("deep"))
            .expect_err("nested watch destination should be rejected");

        assert_eq!(error.code, "MOVE_DESTINATION_WATCHED");
    }

    #[test]
    fn tracked_path_active_requires_owning_enabled_target() {
        let fixture = Fixture::new();
        let disabled = WatchTarget {
            enabled: false,
            ..fixture.watch_target(false)
        };
        let config = config_with_target(&fixture.root, disabled);
        let scope = PathScope::new(&config);

        assert!(!scope.is_tracked_path_active(&fixture.watch.join("file.txt"), "watch"));
        assert!(!scope.is_tracked_path_active(&fixture.root.join("safe").join("file.txt"), "watch"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_scope_matching_is_case_insensitive() {
        let fixture = Fixture::new();
        let mixed_case_file = fixture.watch.join("Case.TXT");
        fs::write(&mixed_case_file, "body").expect("file should be written");
        let mut config = config_with_target(&fixture.root, fixture.watch_target(true));
        config.watch_targets[0].path = config.watch_targets[0].path.to_ascii_uppercase();
        let upper_file = PathBuf::from(path_string(&mixed_case_file).to_ascii_uppercase());

        assert!(PathScope::new(&config).is_in_enabled_watch_target(&upper_file));
    }

    struct Fixture {
        root: PathBuf,
        watch: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = temp_root("shelflife-paths");
            let watch = root.join("watch");
            fs::create_dir_all(&watch).expect("watch directory should be created");
            Self { root, watch }
        }

        fn watch_target(&self, recursive: bool) -> WatchTarget {
            WatchTarget {
                id: String::from("watch"),
                path: path_string(&self.watch),
                enabled: true,
                recursive,
                ignore_patterns: Vec::new(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn config_with_target(_root: &Path, target: WatchTarget) -> AppConfig {
        AppConfig {
            watch_targets: vec![target],
            default_move_destination: None,
            ..AppConfig::default()
        }
    }

    fn temp_root(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}", Uuid::new_v4()))
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }
}
