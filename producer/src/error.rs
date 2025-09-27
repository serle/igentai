//! Producer error types and result handling

use thiserror::Error;
use std::collections::HashMap;
use shared::ProviderId;

/// Result type for producer operations
pub type ProducerResult<T> = Result<T, ProducerError>;

/// Producer error types
#[derive(Error, Debug)]
pub enum ProducerError {
    #[error("IPC communication error: {message}")]
    IpcError { message: String },

    #[error("API provider error for {provider}: {reason}")]
    ApiError { provider: String, reason: String },

    #[error("Rate limit error for {provider}: {message}")]
    RateLimit {
        provider: ProviderId,
        status: u16,
        headers: HashMap<String, String>,
        body: String,
        message: String,
    },

    #[error("Processing error: {message}")]
    ProcessingError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
}

impl ProducerError {
    /// Create IPC error
    pub fn ipc<S: Into<String>>(message: S) -> Self {
        Self::IpcError {
            message: message.into(),
        }
    }

    /// Create API error
    pub fn api<S1: Into<String>, S2: Into<String>>(provider: S1, reason: S2) -> Self {
        Self::ApiError {
            provider: provider.into(),
            reason: reason.into(),
        }
    }

    /// Create processing error
    pub fn processing<S: Into<String>>(message: S) -> Self {
        Self::ProcessingError {
            message: message.into(),
        }
    }

    /// Create configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::ConfigError {
            message: message.into(),
        }
    }

    /// Create serialization error
    pub fn serialization<S: Into<String>>(message: S) -> Self {
        Self::SerializationError {
            message: message.into(),
        }
    }
}
