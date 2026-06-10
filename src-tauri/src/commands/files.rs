use std::io::Read;
use std::path::{Path, PathBuf};

use tauri::State;

use crate::engine::paths::{normalize_configured_path, root_contains};
use crate::models::{
    AppError, FileDecayState, FilePreview, FilePreviewContent, RuleMatchExplanation, TrackedFile,
};
use crate::rules::explain_file_against_rules;
use crate::storage::{self, AppState};

#[tauri::command]
pub async fn get_active_files(state: State<'_, AppState>) -> Result<Vec<TrackedFile>, AppError> {
    active_files(&state.db)
}

fn active_files(db: &redb::Database) -> Result<Vec<TrackedFile>, AppError> {
    Ok(storage::tracked::list_tracked_files(db)?
        .into_iter()
        .filter(|file| {
            !matches!(
                file.state,
                FileDecayState::Missing | FileDecayState::Ignored
            )
        })
        .collect())
}

#[tauri::command]
pub async fn explain_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<RuleMatchExplanation>, AppError> {
    validate_path_scope(&state, &path)?;
    let Some(file) = storage::tracked::get_tracked_file(&state.db, &path)? else {
        return Err(AppError::path_not_found(&path));
    };
    let config = storage::get_config(&state.db)?;
    let rules = storage::rules::list_rules(&state.db)?;
    explain_file_against_rules(&file, &config, &rules)
}

#[tauri::command]
pub async fn preview_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<FilePreview, AppError> {
    validate_path_scope(&state, &path)?;
    build_file_preview(Path::new(&path))
}

pub fn build_file_preview(path_ref: &Path) -> Result<FilePreview, AppError> {
    if !path_ref.exists() {
        return Err(AppError::path_not_found(
            path_ref.to_string_lossy().as_ref(),
        ));
    }

    let metadata = std::fs::metadata(path_ref)?;
    let file_name = path_ref
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let extension = path_ref
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let (mime_type, content) = match extension.as_str() {
        "txt" | "md" | "markdown" | "log" | "json" | "csv" => {
            let (snippet, truncated) = read_text_snippet(path_ref, 8192)?;
            (
                Some(text_mime(&extension).to_string()),
                FilePreviewContent::Text { snippet, truncated },
            )
        }
        "png" | "jpg" | "jpeg" | "gif" | "bmp" => {
            let (width, height, format) = image_dimensions(path_ref, &extension)?;
            (
                Some(format!("image/{format}")),
                FilePreviewContent::Image {
                    width,
                    height,
                    format,
                    thumbnail_path: None,
                },
            )
        }
        "pdf" => {
            let (page_count, title) = pdf_metadata(path_ref)?;
            (
                Some(String::from("application/pdf")),
                FilePreviewContent::Pdf { page_count, title },
            )
        }
        _ => (None, FilePreviewContent::Unknown),
    };

    Ok(FilePreview {
        path: path_ref.to_string_lossy().to_string(),
        file_name,
        size_bytes: metadata.len(),
        mime_type,
        content,
    })
}

fn read_text_snippet(path: &Path, max_bytes: usize) -> Result<(String, bool), AppError> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = vec![0; max_bytes + 1];
    let read = file.read(&mut bytes)?;
    let truncated = read > max_bytes;
    bytes.truncate(read.min(max_bytes));
    Ok((String::from_utf8_lossy(&bytes).to_string(), truncated))
}

fn text_mime(extension: &str) -> &'static str {
    match extension {
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        _ => "text/plain",
    }
}

fn image_dimensions(path: &Path, extension: &str) -> Result<(u32, u32, String), AppError> {
    let mut file = std::fs::File::open(path)?;
    let mut bytes = [0_u8; 64];
    let read = file.read(&mut bytes)?;
    let bytes = &bytes[..read];

    match extension {
        "png" if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Ok((
            u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            String::from("png"),
        )),
        "gif"
            if bytes.len() >= 10
                && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) =>
        {
            Ok((
                u16::from_le_bytes([bytes[6], bytes[7]]) as u32,
                u16::from_le_bytes([bytes[8], bytes[9]]) as u32,
                String::from("gif"),
            ))
        }
        "bmp" if bytes.len() >= 26 && bytes.starts_with(b"BM") => Ok((
            u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]),
            u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]),
            String::from("bmp"),
        )),
        "jpg" | "jpeg" => jpeg_dimensions(path),
        _ => Ok((0, 0, extension.to_string())),
    }
}

fn jpeg_dimensions(path: &Path) -> Result<(u32, u32, String), AppError> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(64 * 1024).read_to_end(&mut bytes)?;
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return Ok((0, 0, String::from("jpeg")));
    }

    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]) as u32;
            return Ok((width, height, String::from("jpeg")));
        }
        let length = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if length < 2 {
            break;
        }
        index += 2 + length;
    }

    Ok((0, 0, String::from("jpeg")))
}

fn pdf_metadata(path: &Path) -> Result<(Option<u32>, Option<String>), AppError> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(256 * 1024).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let page_count = text.matches("/Type /Page").count().try_into().ok();
    let title = text
        .split("/Title")
        .nth(1)
        .and_then(|rest| rest.split('(').nth(1))
        .and_then(|rest| rest.split(')').next())
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    Ok((page_count, title))
}

#[tauri::command]
pub async fn open_file_location(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
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

fn validate_path_scope(state: &State<'_, AppState>, path: &str) -> Result<(), AppError> {
    let config = storage::get_config(&state.db)?;
    let path = PathBuf::from(path);

    let in_watch_target = config
        .watch_targets
        .iter()
        .filter(|target| target.enabled)
        .any(|target| root_contains(&target.path, &path));
    let in_safe_folder = root_contains(&config.safe_folder_path, &path);

    if in_watch_target || in_safe_folder {
        Ok(())
    } else {
        let normalized = normalize_configured_path(&path).unwrap_or(path);
        Err(AppError::path_out_of_scope(
            normalized.to_string_lossy().as_ref(),
        ))
    }
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

    use crate::models::{AppConfig, FileDecayState, FilePreviewContent};
    use crate::storage;

    use super::{active_files, build_file_preview};

    #[test]
    fn text_preview_is_bounded_and_marks_truncated() {
        let fixture = Fixture::new();
        let path = fixture.write("notes.txt", &"a".repeat(9000));

        let preview = build_file_preview(&path).expect("preview should build");
        match preview.content {
            FilePreviewContent::Text { snippet, truncated } => {
                assert_eq!(snippet.len(), 8192);
                assert!(truncated);
            }
            other => panic!("expected text preview, got {other:?}"),
        }
    }

    #[test]
    fn png_preview_reads_dimensions_without_thumbnail_payload() {
        let fixture = Fixture::new();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&320_u32.to_be_bytes());
        bytes.extend_from_slice(&200_u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        let path = fixture.write_bytes("image.png", &bytes);

        let preview = build_file_preview(&path).expect("preview should build");
        match preview.content {
            FilePreviewContent::Image {
                width,
                height,
                format,
                thumbnail_path,
            } => {
                assert_eq!(width, 320);
                assert_eq!(height, 200);
                assert_eq!(format, "png");
                assert!(thumbnail_path.is_none());
            }
            other => panic!("expected image preview, got {other:?}"),
        }
    }

    #[test]
    fn pdf_preview_reads_metadata_only() {
        let fixture = Fixture::new();
        let path = fixture.write(
            "brief.pdf",
            "%PDF-1.4\n/Title (Quarterly Brief)\n/Type /Page\n/Type /Page\n%%EOF",
        );

        let preview = build_file_preview(&path).expect("preview should build");
        match preview.content {
            FilePreviewContent::Pdf { page_count, title } => {
                assert_eq!(page_count, Some(2));
                assert_eq!(title, Some(String::from("Quarterly Brief")));
            }
            other => panic!("expected pdf preview, got {other:?}"),
        }
    }

    #[test]
    fn active_files_hide_missing_and_ignored_by_default() {
        let fixture = Fixture::new();
        let fresh = fixture.write("fresh.txt", "fresh");
        let ignored = fixture.write("ignored.txt", "ignored");
        let missing = fixture.root.join("missing.txt");
        let db =
            storage::open_database(fixture.root.join("test.redb")).expect("database should open");

        for (path, state) in [
            (&fresh, FileDecayState::Fresh),
            (&ignored, FileDecayState::Ignored),
            (&missing, FileDecayState::Missing),
        ] {
            let metadata = fs::metadata(path)
                .unwrap_or_else(|_| fs::metadata(&fresh).expect("metadata should exist"));
            let config = AppConfig::default();
            let mut tracked = crate::engine::tracked_file_from_metadata(
                path,
                &metadata,
                None,
                &config,
                config.default_ttl_seconds,
            );
            tracked.state = state;
            storage::tracked::upsert_tracked_file(&db, &tracked).expect("tracked file should save");
        }

        let files = active_files(&db).expect("active files should load");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, fresh.to_string_lossy());
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("shelflife-preview-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("fixture directory should be created");
            Self { root }
        }

        fn write(&self, name: &str, content: &str) -> PathBuf {
            self.write_bytes(name, content.as_bytes())
        }

        fn write_bytes(&self, name: &str, content: &[u8]) -> PathBuf {
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
