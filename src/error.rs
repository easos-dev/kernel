use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, KernelError>;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("plugin already exists: {0}")]
    AlreadyExists(String),
    #[error("protected plugin cannot be changed: {0}")]
    Protected(String),
    #[error("dependency error: {0}")]
    Dependency(String),
    #[error("operation conflict: {0}")]
    Conflict(String),
    #[error("daemon unavailable: {0}")]
    Unavailable(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl KernelError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidData(_) | Self::Json(_) => "INVALID_DATA",
            Self::NotFound(_) => "NOT_FOUND",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",
            Self::Protected(_) => "PROTECTED_PLUGIN",
            Self::Dependency(_) => "DEPENDENCY_ERROR",
            Self::Conflict(_) => "CONFLICT",
            Self::Unavailable(_) => "UNAVAILABLE",
            Self::Io(_) => "IO_ERROR",
            Self::Internal(_) => "INTERNAL",
        }
    }
}
