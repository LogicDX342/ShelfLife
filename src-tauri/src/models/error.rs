use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Error)]
#[error("{code}: {message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub recoverable: bool,
    pub details: Option<String>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recoverable,
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recoverable,
            details: Some(details.into()),
        }
    }

    pub fn path_not_found(path: &str) -> Self {
        Self::with_details(
            "PATH_NOT_FOUND",
            "The file path does not exist. No file was changed.",
            true,
            path,
        )
    }

    pub fn path_out_of_scope(path: &str) -> Self {
        Self::with_details(
            "PATH_OUT_OF_SCOPE",
            "The path is outside configured watch targets or the safe folder. No file was changed.",
            true,
            path,
        )
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        let code = if value.kind() == std::io::ErrorKind::NotFound {
            "PATH_NOT_FOUND"
        } else if value.kind() == std::io::ErrorKind::PermissionDenied {
            "PERMISSION_DENIED"
        } else {
            "ACTION_FAILED"
        };
        Self::with_details(
            code,
            "Filesystem operation failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<redb::DatabaseError> for AppError {
    fn from(value: redb::DatabaseError) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Database operation failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<redb::TableError> for AppError {
    fn from(value: redb::TableError) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Database table operation failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<redb::TransactionError> for AppError {
    fn from(value: redb::TransactionError) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Database transaction failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<redb::CommitError> for AppError {
    fn from(value: redb::CommitError) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Database commit failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<redb::StorageError> for AppError {
    fn from(value: redb::StorageError) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Database storage operation failed.",
            true,
            value.to_string(),
        )
    }
}

impl From<bincode::Error> for AppError {
    fn from(value: bincode::Error) -> Self {
        Self::with_details(
            "DATABASE_ERROR",
            "Stored record serialization failed.",
            true,
            value.to_string(),
        )
    }
}
