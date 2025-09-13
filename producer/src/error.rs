//! Producer-specific error types

use thiserror::Error;
use std::time::Duration;
use shared::SharedError;

#[derive(Error, Debug)]
pub enum ProducerError {
    #[error("LLM API error for {provider}: {message}")]
    ApiError { provider: String, message: String },
    
    #[error("Provider {provider} is not configured")]
    ProviderNotConfigured { provider: String },
    
    #[error("No healthy providers available")]
    NoHealthyProviders,
    
    #[error("Unknown provider: {provider}")]
    UnknownProvider { provider: String },
    
    #[error("Provider selection failed: {reason}")]
    SelectionError { reason: String },
    
    #[error("Response parsing failed: {response_length} chars")]
    ParseError { response_length: usize },
    
    #[error("Empty response from {provider}")]
    EmptyResponse { provider: String },
    
    #[error("Communication timeout: {duration:?}")]
    CommunicationTimeout { duration: Duration },
    
    #[error("TCP connection failed to {address}")]
    ConnectionFailed { address: String },
    
    #[error("Message serialization failed")]
    SerializationError(String),
    
    #[error("Generation configuration invalid: {field}")]
    ConfigError { field: String },
    
    #[error("Provider health check failed: {provider}")]
    HealthCheckFailed { provider: String },
    
    #[error("No providers configured")]
    NoProvidersConfigured,
    
    #[error("Shared component error")]
    SharedError(#[from] SharedError),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

pub type ProducerResult<T> = Result<T, ProducerError>;