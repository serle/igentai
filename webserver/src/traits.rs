//! Service trait definitions for dependency injection
//! 
//! All I/O operations are abstracted through these traits for testability

use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;

use shared::{WebServerRequest, OrchestratorUpdate};
use crate::error::WebServerResult;
use crate::types::ClientMessage;

/// Orchestrator communication service trait
#[mockall::automock]
#[async_trait]
pub trait OrchestratorClient: Send + Sync {
    /// Initialize IPC connection with orchestrator
    async fn initialize(&mut self) -> WebServerResult<()>;
    
    /// Send request to orchestrator
    async fn send_request(&self, request: WebServerRequest) -> WebServerResult<()>;
    
    /// Get receiver for orchestrator updates
    async fn get_updates(&mut self) -> WebServerResult<mpsc::Receiver<OrchestratorUpdate>>;
    
    /// Check connection health with orchestrator
    async fn health_check(&self) -> WebServerResult<bool>;
    
    /// Disconnect from orchestrator
    async fn disconnect(&self) -> WebServerResult<()>;
}

/// WebSocket client management service trait
#[mockall::automock]
#[async_trait]
pub trait WebSocketManager: Send + Sync {
    /// Add new WebSocket client
    async fn add_client(&self, client_id: Uuid, sender: mpsc::Sender<ClientMessage>) -> WebServerResult<()>;
    
    /// Remove WebSocket client
    async fn remove_client(&self, client_id: Uuid) -> WebServerResult<()>;
    
    /// Broadcast message to all connected clients
    async fn broadcast(&self, message: ClientMessage) -> WebServerResult<()>;
    
    /// Send message to specific client
    async fn send_to_client(&self, client_id: Uuid, message: ClientMessage) -> WebServerResult<()>;
    
    /// Get count of active clients
    async fn client_count(&self) -> usize;
    
    /// Get list of active client IDs
    async fn active_clients(&self) -> Vec<Uuid>;
}

/// Static file serving service trait
#[mockall::automock]
#[async_trait]
pub trait StaticFileServer: Send + Sync {
    /// Serve static file
    async fn serve_file(&self, path: &str) -> WebServerResult<StaticFileResponse>;
    
    /// Check if file exists
    async fn file_exists(&self, path: &str) -> bool;
    
    /// Get file content type
    async fn content_type(&self, path: &str) -> WebServerResult<String>;
    
    /// List available files (for development)
    async fn list_files(&self) -> WebServerResult<Vec<String>>;
}

/// Static file response
#[derive(Debug, Clone)]
pub struct StaticFileResponse {
    pub content: Vec<u8>,
    pub content_type: String,
    pub cache_control: Option<String>,
}

impl StaticFileResponse {
    /// Create new static file response
    pub fn new(content: Vec<u8>, content_type: String) -> Self {
        Self {
            content,
            content_type,
            cache_control: None,
        }
    }
    
    /// Set cache control header
    pub fn with_cache_control(mut self, cache_control: String) -> Self {
        self.cache_control = Some(cache_control);
        self
    }
}