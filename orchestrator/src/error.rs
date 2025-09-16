//! Error types for the orchestrator

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
    
    #[error("Communication error: {message}")]
    CommunicationError { message: String },
    
    #[error("Process management error: {message}")]
    ProcessError { message: String },
    
    #[error("Performance tracking error: {message}")]
    PerformanceError { message: String },
    
    #[error("Uniqueness tracking error: {message}")]
    UniquenessError { message: String },
    
    #[error("Optimization error: {message}")]
    OptimizationError { message: String },
    
    #[error("File system error: {source}")]
    FileSystemError {
        #[from]
        source: std::io::Error,
    },
    
    #[error("Serialization error: {source}")]
    SerializationError {
        #[from]
        source: bincode::Error,
    },
    
    #[error("JSON error: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
    
    #[error("UUID parse error: {source}")]
    UuidError {
        #[from]
        source: uuid::Error,
    },
}

impl OrchestratorError {
    pub fn config(message: impl Into<String>) -> Self {
        Self::ConfigurationError { message: message.into() }
    }
    
    pub fn communication(message: impl Into<String>) -> Self {
        Self::CommunicationError { message: message.into() }
    }
    
    pub fn process(message: impl Into<String>) -> Self {
        Self::ProcessError { message: message.into() }
    }
    
    pub fn performance(message: impl Into<String>) -> Self {
        Self::PerformanceError { message: message.into() }
    }
    
    pub fn uniqueness(message: impl Into<String>) -> Self {
        Self::UniquenessError { message: message.into() }
    }
    
    pub fn optimization(message: impl Into<String>) -> Self {
        Self::OptimizationError { message: message.into() }
    }
}

pub type OrchestratorResult<T> = Result<T, OrchestratorError>;