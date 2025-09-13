//! Service trait definitions for webserver
//!
//! This module defines only the service traits used for dependency injection.
//! All other types are in the types module.

use std::net::SocketAddr;
use std::sync::Arc;
use async_trait::async_trait;
use axum::extract::ws::WebSocket;
use axum::response::Response;
use shared::SystemMetrics;
use crate::error::WebServerResult;
use crate::types::{
    ClientId, BrowserMessage, AttributeUpdate, SystemHealth, DashboardData, ClientConnection
};

/// File manager trait
#[async_trait]
pub trait FileManager: Send + Sync {
    /// Serve a static file by path
    async fn serve_file(&self, path: &str) -> WebServerResult<Response>;
    
    /// Check if a file exists
    async fn file_exists(&self, path: &str) -> bool;
    
    /// Get file content type
    fn get_content_type(&self, path: &str) -> String;
    
    /// Get file size
    async fn get_file_size(&self, path: &str) -> WebServerResult<u64>;
}

/// Client broadcaster trait  
#[async_trait]
pub trait ClientBroadcaster: Send + Sync {
    /// Handle a new WebSocket connection
    async fn handle_connection(&self, socket: WebSocket, client_id: ClientId) -> WebServerResult<()>;
    
    /// Broadcast message to all connected clients
    async fn broadcast_to_all(&self, message: BrowserMessage) -> WebServerResult<()>;
    
    /// Send message to specific client
    async fn send_to_client(&self, client_id: &ClientId, message: BrowserMessage) -> WebServerResult<()>;
    
    /// Get connected client count
    async fn get_client_count(&self) -> u32;
    
    /// Remove disconnected client
    async fn remove_client(&self, client_id: &ClientId) -> WebServerResult<()>;
}

/// IPC communicator trait for orchestrator communication
#[async_trait]
pub trait IpcCommunicator: Send + Sync {
    /// Connect to orchestrator
    async fn connect(&self, orchestrator_addr: SocketAddr) -> WebServerResult<()>;
    
    /// Send browser message to orchestrator (converts to TaskRequest internally)
    async fn send_message(&self, message: BrowserMessage) -> WebServerResult<()>;
    
    /// Check if connected to orchestrator
    async fn is_connected(&self) -> bool;
    
    /// Start connection management loop (handles TaskUpdate messages from orchestrator)
    async fn start_connection_loop(&self) -> WebServerResult<()>;
    
    /// Disconnect from orchestrator
    async fn disconnect(&self) -> WebServerResult<()>;
}

/// Client registry trait
#[async_trait]
pub trait ClientRegistry: Send + Sync {
    /// Add a new client connection
    async fn add_client(&self, connection: ClientConnection) -> WebServerResult<()>;
    
    /// Remove a client connection
    async fn remove_client(&self, client_id: &ClientId) -> WebServerResult<()>;
    
    /// Get client connection
    async fn get_client(&self, client_id: &ClientId) -> WebServerResult<Arc<ClientConnection>>;
    
    /// Get all connected client IDs
    async fn get_all_clients(&self) -> Vec<ClientId>;
    
    /// Get connection count
    async fn get_connection_count(&self) -> u32;
    
    /// Cleanup stale connections
    async fn cleanup_stale_connections(&self) -> WebServerResult<u32>;
}

/// Metrics aggregator trait
#[async_trait]
pub trait MetricsAggregator: Send + Sync {
    /// Update system metrics from orchestrator
    async fn update_metrics(&self, metrics: SystemMetrics) -> WebServerResult<()>;
    
    /// Get current metrics
    async fn get_current_metrics(&self) -> WebServerResult<SystemMetrics>;
    
    /// Add new attribute update
    async fn add_attribute_update(&self, update: AttributeUpdate) -> WebServerResult<()>;
    
    /// Get recent attribute updates
    async fn get_recent_attributes(&self, limit: usize) -> WebServerResult<Vec<AttributeUpdate>>;
    
    /// Get system health information
    async fn get_system_health(&self) -> WebServerResult<SystemHealth>;
    
    /// Get dashboard data bundle
    async fn get_dashboard_data(&self) -> WebServerResult<DashboardData>;
    
    /// Clear all metrics (for reset)
    async fn clear_metrics(&self) -> WebServerResult<()>;
}