//! Client broadcaster service implementation
//!
//! This service handles broadcasting messages to connected web clients.

use std::sync::Arc;
use axum::extract::ws::{WebSocket, Message};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use crate::traits::ClientBroadcaster;
use crate::types::{BrowserMessage, ClientId, ClientConnection};
use crate::error::{WebServerError, WebServerResult};
use crate::state::WebServerState;

/// Real client broadcaster service implementation
#[derive(Clone)]
pub struct RealClientBroadcaster {
    state: Arc<WebServerState>,
}

impl RealClientBroadcaster {
    /// Create a new WebSocket service
    pub fn new(state: Arc<WebServerState>) -> Self {
        Self { state }
    }
    
    /// Handle incoming message from client and forward appropriately
    async fn handle_client_message(&self, client_id: ClientId, message: BrowserMessage) -> WebServerResult<()> {
        println!("📨 Received message from client {}: {:?}", client_id.0, message);
        
        match message {
            BrowserMessage::StartTopic { .. } | 
            BrowserMessage::StopGeneration | 
            BrowserMessage::RequestDashboard => {
                // These messages should be forwarded to orchestrator via IPC
                // For now, just log that we received them
                // In full implementation, this would use IpcCommunicator service
                println!("🔀 Message type should be forwarded to orchestrator: {:?}", message);
            }
            BrowserMessage::MetricsUpdate { .. } | 
            BrowserMessage::AttributeBatch { .. } | 
            BrowserMessage::SystemAlert { .. } | 
            BrowserMessage::DashboardData { .. } => {
                // These are typically server->client messages, unusual to receive from client
                println!("⚠️ Unexpected message type from client: {:?}", message);
            }
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClientBroadcaster for RealClientBroadcaster {
    async fn handle_connection(&self, socket: WebSocket, client_id: ClientId) -> WebServerResult<()> {
        println!("📱 New WebSocket client connected: {}", client_id.0);
        
        // Create client connection with websocket channel
        let (tx, _rx) = mpsc::channel(100);
        let connection = ClientConnection {
            id: client_id.clone(),
            websocket_tx: tx,
            connected_at: std::time::Instant::now(),
            last_ping: std::time::Instant::now(),
            user_agent: None, // TODO: Extract from WebSocket headers if needed
        };
        
        // Add client to state (this will be used by ClientRegistry if integrated)
        {
            let mut connections = self.state.client_connections.write().await;
            connections.insert(client_id.clone(), Arc::new(connection));
        }
        
        // Increment connection count
        self.state.increment_connection_count();
        
        // Send initial dashboard data to new client
        let initial_message = BrowserMessage::RequestDashboard;
        if let Err(e) = self.handle_client_message(client_id.clone(), initial_message).await {
            eprintln!("Failed to send initial dashboard data: {}", e);
        }
        
        // Split the socket for concurrent reading/writing
        let (mut sender, mut receiver) = socket.split();
        
        // Set up broadcast receiver for outgoing messages
        let mut broadcast_rx = self.state.client_broadcast.subscribe();
        let client_id_for_broadcast = client_id.clone();
        
        // Handle outgoing messages (server -> client)
        let broadcast_task = tokio::spawn(async move {
            while let Ok(message) = broadcast_rx.recv().await {
                let json_msg = match serde_json::to_string(&message) {
                    Ok(json) => json,
                    Err(e) => {
                        eprintln!("Failed to serialize broadcast message: {}", e);
                        continue;
                    }
                };
                
                if let Err(e) = sender.send(Message::Text(json_msg)).await {
                    eprintln!("Failed to send message to client {}: {}", client_id_for_broadcast.0, e);
                    break;
                }
            }
        });
        
        // Handle incoming messages (client -> server)
        let broadcaster_for_incoming = self.clone();
        let client_id_for_incoming = client_id.clone();
        
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Parse and handle incoming browser messages
                        match serde_json::from_str::<BrowserMessage>(&text) {
                            Ok(browser_msg) => {
                                if let Err(e) = broadcaster_for_incoming.handle_client_message(client_id_for_incoming.clone(), browser_msg).await {
                                    eprintln!("Failed to handle message from client {}: {}", client_id_for_incoming.0, e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse message from client {}: {}", client_id_for_incoming.0, e);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        println!("🔌 Client {} disconnected", client_id_for_incoming.0);
                        break;
                    }
                    Ok(Message::Ping(_data)) => {
                        // Respond to ping with pong
                        // Note: Axum handles this automatically, but we could log it
                        println!("🏓 Ping from client {}", client_id_for_incoming.0);
                    }
                    Ok(_) => {
                        // Handle other message types (binary, pong) if needed
                    }
                    Err(e) => {
                        eprintln!("WebSocket error for client {}: {}", client_id_for_incoming.0, e);
                        break;
                    }
                }
            }
        });
        
        // Wait for either task to complete (connection closed or error)
        tokio::select! {
            _ = broadcast_task => {
                println!("Broadcast task completed for client {}", client_id.0);
            }
            _ = incoming_task => {
                println!("Incoming task completed for client {}", client_id.0);
            }
        }
        
        // Clean up: remove client from connections and decrement count
        {
            let mut connections = self.state.client_connections.write().await;
            connections.remove(&client_id);
        }
        
        self.state.decrement_connection_count();
        println!("🚪 Client {} disconnected and cleaned up", client_id.0);
        
        Ok(())
    }
    
    async fn broadcast_to_all(&self, message: BrowserMessage) -> WebServerResult<()> {
        match self.state.client_broadcast.send(message) {
            Ok(_receiver_count) => {
                println!("📡 Broadcasted message to all clients");
                Ok(())
            }
            Err(e) => {
                eprintln!("Failed to broadcast message: {}", e);
                Err(WebServerError::BroadcastError { client_count: 0 })
            }
        }
    }
    
    async fn send_to_client(&self, client_id: &ClientId, message: BrowserMessage) -> WebServerResult<()> {
        // Check if client exists and get their websocket channel
        let connections = self.state.client_connections.read().await;
        if let Some(connection) = connections.get(client_id) {
            let json_msg = serde_json::to_string(&message)
                .map_err(|e| WebServerError::JsonError(e))?;
            
            if let Err(_) = connection.websocket_tx.send(Message::Text(json_msg)).await {
                return Err(WebServerError::ClientConnectionError { 
                    client_id: client_id.0.clone() 
                });
            }
            Ok(())
        } else {
            Err(WebServerError::ClientConnectionError { 
                client_id: client_id.0.clone() 
            })
        }
    }
    
    async fn get_client_count(&self) -> u32 {
        self.state.get_connection_count()
    }
    
    async fn remove_client(&self, client_id: &ClientId) -> WebServerResult<()> {
        // Remove from connections map (the connection tasks will handle cleanup)
        let mut connections = self.state.client_connections.write().await;
        if connections.remove(client_id).is_some() {
            self.state.decrement_connection_count();
            println!("🔌 Forcibly disconnected client {}", client_id.0);
            Ok(())
        } else {
            Err(WebServerError::ClientConnectionError { 
                client_id: client_id.0.clone() 
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    
    #[tokio::test]
    async fn test_websocket_service_creation() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let service = RealClientBroadcaster::new(state);
        
        // Test broadcast message
        let message = BrowserMessage::MetricsUpdate {
            metrics: shared::SystemMetrics::default(),
        };
        
        // Should fail with no receivers (expected behavior)
        let result = service.broadcast_to_all(message).await;
        // When no receivers, broadcast will return error
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_get_connected_clients() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let service = RealClientBroadcaster::new(state);
        
        let client_count = service.get_client_count().await;
        assert_eq!(client_count, 0);
    }
}