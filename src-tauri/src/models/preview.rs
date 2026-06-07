use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FilePreviewContent {
    Text {
        snippet: String,
        truncated: bool,
    },
    Image {
        width: u32,
        height: u32,
        format: String,
        thumbnail_path: Option<String>,
    },
    Pdf {
        page_count: Option<u32>,
        title: Option<String>,
    },
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FilePreview {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub mime_type: Option<String>,
    pub content: FilePreviewContent,
}
