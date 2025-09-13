//! Producer error types

use thiserror::Error;
use shared::ApiFailure;

/// Result type for producer operations
pub type ProducerResult<T> = Result<T, ProducerError>;

/// Producer error types
#[derive(Error, Debug)]
pub enum ProducerError {
    #[error("IPC communication error: {message}")]
    IpcError { message: String },

    #[error("Provider request failed: {provider} - {reason:?}")]
    ProviderError { provider: String, reason: ApiFailure },

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Bloom filter error: {message}")]
    BloomFilterError { message: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}