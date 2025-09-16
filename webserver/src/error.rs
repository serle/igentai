//! WebServer error types and handling

use thiserror::Error;
use std::io;

/// WebServer result type
pub type WebServerResult<T> = Result<T, WebServerError>;

/// Comprehensive error types for WebServer operations
#[derive(Error, Debug)]
pub enum WebServerError {
    /// Communication errors with orchestrator
    #[error("Communication error: {message}")]
    Communication { message: String },

    /// HTTP server errors
    #[error("HTTP server error: {message}")]
    Http { message: String },

    /// WebSocket errors
    #[error("WebSocket error: {message}")]
    WebSocket { message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// I/O errors
    #[error("I/O error: {source}")]
    IoError { source: io::Error },

    /// JSON serialization/deserialization errors
    #[error("JSON error: {source}")]
    JsonError { source: serde_json::Error },

    /// Bincode serialization/deserialization errors
    #[error("Bincode error: {source}")]
    BincodeError { source: bincode::Error },

    /// Internal server errors
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl WebServerError {
    /// Create communication error
    pub fn communication(message: impl Into<String>) -> Self {
        Self::Communication { message: message.into() }
    }

    /// Create HTTP server error
    pub fn http(message: impl Into<String>) -> Self {
        Self::Http { message: message.into() }
    }

    /// Create WebSocket error
    pub fn websocket(message: impl Into<String>) -> Self {
        Self::WebSocket { message: message.into() }
    }

    /// Create configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config { message: message.into() }
    }

    /// Create internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal { message: message.into() }
    }
}

impl From<io::Error> for WebServerError {
    fn from(error: io::Error) -> Self {
        Self::IoError { source: error }
    }
}

impl From<serde_json::Error> for WebServerError {
    fn from(error: serde_json::Error) -> Self {
        Self::JsonError { source: error }
    }
}

impl From<bincode::Error> for WebServerError {
    fn from(error: bincode::Error) -> Self {
        Self::BincodeError { source: error }
    }
}