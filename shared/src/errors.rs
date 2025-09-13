//! Shared error types for the LLM orchestration system

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SharedError {
    #[error("Serialization failed: {message}")]
    SerializationError { message: String },
    
    #[error("Deserialization failed: {message}")]
    DeserializationError { message: String },
    
    #[error("Invalid UUID: {input}")]
    InvalidUuid { input: String },
    
    #[error("Invalid configuration: {field} = {value}")]
    InvalidConfig { field: String, value: String },
    
    #[error("Message protocol error: {message}")]
    ProtocolError { message: String },
}

pub type SharedResult<T> = Result<T, SharedError>;