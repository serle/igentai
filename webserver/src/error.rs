//! WebServer-specific error types

use thiserror::Error;
use shared::SharedError;

#[derive(Error, Debug)]
pub enum WebServerError {
    #[error("HTTP server startup failed on port {port}")]
    ServerStartupFailed { port: u16 },
    
    #[error("WebSocket connection failed: {message}")]
    WebSocketFailed { message: String },
    
    #[error("Client connection error: {client_id}")]
    ClientConnectionError { client_id: String },
    
    #[error("Static file not found: {path}")]
    StaticFileNotFound { path: String },
    
    #[error("Static file serving error: {path}")]
    StaticFileError { path: String },
    
    #[error("Orchestrator communication failed: {message}")]
    OrchestratorCommError { message: String },
    
    #[error("Request handler error: {endpoint}")]
    HandlerError { endpoint: String },
    
    #[error("Message broadcasting failed to {client_count} clients")]
    BroadcastError { client_count: usize },
    
    #[error("Dashboard data retrieval failed")]
    DashboardError,
    
    #[error("Invalid request format: {details}")]
    InvalidRequest { details: String },
    
    #[error("Shared component error")]
    SharedError(#[from] SharedError),
    
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Server startup error: {0}")]
    ServerStartup(String),
    
    #[error("Response building error: {0}")]
    ResponseError(String),
    
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    #[error("Client not found: {0}")]
    ClientNotFound(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Not connected to orchestrator")]
    NotConnected,
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type WebServerResult<T> = Result<T, WebServerError>;