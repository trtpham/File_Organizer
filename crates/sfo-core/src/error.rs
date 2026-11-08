//! Error type shared by every core module.

use std::path::PathBuf;

/// Errors raised by the core engine.
#[derive(Debug, thiserror::Error)]
pub enum SfoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Source directory does not exist: {}", .0.display())]
    NotFound(PathBuf),

    #[error("Source path is not a directory: {}", .0.display())]
    NotADirectory(PathBuf),

    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Result alias for core operations.
pub type Result<T> = std::result::Result<T, SfoError>;
