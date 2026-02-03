use thiserror::Error;

#[derive(Error, Debug)]
pub enum StormError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("HTTP error: {status} - {message}")]
    Http { status: u16, message: String },

    #[error("Server does not support range requests")]
    RangeNotSupported,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("Resource changed on server (ETag/Last-Modified mismatch)")]
    ResourceChanged,

    #[error("Download cancelled")]
    Cancelled,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Rate limited by server")]
    RateLimited,

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("{0}")]
    Other(String),
}
