//! Client registry service implementation
//!
//! This service maintains a registry of active client connections.

use std::sync::Arc;
use crate::traits::ClientRegistry;
use crate::types::{ClientId, ClientConnection};
use crate::error::{WebServerError, WebServerResult};
use crate::state::WebServerState;

/// Real client registry implementation
#[derive(Clone)]
pub struct RealClientRegistry {
    state: Arc<WebServerState>,
}

impl RealClientRegistry {
    /// Create a new connection manager
    pub fn new(state: Arc<WebServerState>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl ClientRegistry for RealClientRegistry {
    async fn add_client(&self, connection: ClientConnection) -> WebServerResult<()> {
        let client_id = connection.id.clone();
        println!("📝 Adding client {} to registry", client_id.0);
        
        let mut connections = self.state.client_connections.write().await;
        connections.insert(client_id, Arc::new(connection));
        
        // Update connection count
        self.state.increment_connection_count();
        
        println!("✅ Client registry now has {} active connections", connections.len());
        Ok(())
    }
    
    async fn remove_client(&self, client_id: &ClientId) -> WebServerResult<()> {
        println!("📝 Removing client {} from registry", client_id.0);
        
        let mut connections = self.state.client_connections.write().await;
        let removed = connections.remove(client_id);
        
        if removed.is_some() {
            // Update connection count  
            self.state.decrement_connection_count();
            println!("✅ Client removed. Registry now has {} active connections", connections.len());
        } else {
            println!("⚠️ Client {} not found in registry", client_id.0);
        }
        
        Ok(())
    }
    
    async fn get_connection_count(&self) -> u32 {
        let connections = self.state.client_connections.read().await;
        connections.len() as u32
    }
    
    async fn get_all_clients(&self) -> Vec<ClientId> {
        let connections = self.state.client_connections.read().await;
        connections.keys().cloned().collect()
    }
    
    async fn get_client(&self, client_id: &ClientId) -> WebServerResult<Arc<ClientConnection>> {
        let connections = self.state.client_connections.read().await;
        connections.get(client_id).cloned().ok_or(
            WebServerError::ClientConnectionError { client_id: client_id.0.clone() }
        )
    }
    
    async fn cleanup_stale_connections(&self) -> WebServerResult<u32> {
        println!("🧹 Cleaning up stale connections");
        
        let mut connections = self.state.client_connections.write().await;
        let mut stale_clients = Vec::new();
        let now = std::time::Instant::now();
        let stale_timeout = std::time::Duration::from_secs(300); // 5 minutes
        
        // Find stale connections
        for (client_id, connection) in connections.iter() {
            if now.duration_since(connection.last_ping) > stale_timeout {
                stale_clients.push(client_id.clone());
            }
        }
        
        // Remove stale connections
        let stale_count = stale_clients.len();
        for client_id in stale_clients {
            connections.remove(&client_id);
            println!("🗑️ Removed stale client: {}", client_id.0);
        }
        
        if stale_count > 0 {
            println!("✅ Cleaned up {} stale connections", stale_count);
        }
        
        Ok(stale_count as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    
    #[tokio::test]
    async fn test_connection_manager() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let manager = RealClientRegistry::new(state);
        
        // Test initial state
        assert_eq!(manager.get_connection_count().await, 0);
        
        let _client_id = ClientId(uuid::Uuid::new_v4().to_string());
        
        // Get all clients (should be empty initially)
        let clients = manager.get_all_clients().await;
        assert_eq!(clients.len(), 0);
    }
}