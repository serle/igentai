//! Orchestrator-specific error types

use thiserror::Error;
use std::time::Duration;
use shared::SharedError;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Failed to spawn producer process: {producer_id}")]
    ProducerSpawnFailed { producer_id: String },
    
    #[error("Producer communication timeout: {producer_id}")]
    ProducerTimeout { producer_id: String, timeout: Duration },
    
    #[error("WebServer process management failed: {message}")]
    WebServerError { message: String },
    
    #[error("File system operation failed: {operation} on {path}")]
    FileSystemError { operation: String, path: String },
    
    #[error("Topic folder creation failed: {topic}")]
    TopicFolderError { topic: String },
    
    #[error("Statistics calculation error: {message}")]
    StatisticsError { message: String },
    
    #[error("Optimization cycle failed: {reason}")]
    OptimizationError { reason: String },
    
    #[error("Uniqueness check failed: batch size {batch_size}")]
    UniquenessError { batch_size: usize },
    
    #[error("Configuration error: {field}")]
    ConfigurationError { field: String },
    
    #[error("Shared component error")]
    SharedError(#[from] SharedError),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Binary serialization error: {message}")]
    SerializationError { message: String },
    
    #[error("Network communication error: {message}")]
    NetworkError { message: String },
}

pub type OrchestratorResult<T> = Result<T, OrchestratorError>;