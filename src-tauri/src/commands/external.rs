use crate::models::AppError;

const ALLOWED_EXTERNAL_URLS: [&str; 2] = [
    "https://github.com/LogicDX342/ShelfLife",
    "https://github.com/LogicDX342/ShelfLife/issues/new",
];

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), AppError> {
    if !ALLOWED_EXTERNAL_URLS.contains(&url.as_str()) {
        return Err(AppError::with_details(
            "ACTION_FAILED",
            "External link is not allowed.",
            true,
            url,
        ));
    }

    open_url(&url)
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) -> Result<(), AppError> {
    std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(AppError::from)
}

#[cfg(not(target_os = "windows"))]
fn open_url(_url: &str) -> Result<(), AppError> {
    Err(AppError::new(
        "ACTION_FAILED",
        "External links are only implemented for Windows v1.",
        true,
    ))
}
