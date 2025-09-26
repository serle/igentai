//! WebSocket client management service
//!
//! Manages WebSocket connections and broadcasting to browser clients

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tokio::time::{sleep, interval, Duration};
use uuid::Uuid;

use crate::error::{WebServerError, WebServerResult};
use crate::traits::WebSocketManager;
use crate::types::ClientMessage;

/// WebSocket client connection info
#[derive(Debug)]
struct ClientConnection {
    #[allow(dead_code)]
    id: Uuid,
    sender: mpsc::Sender<ClientMessage>,
    #[allow(dead_code)]
    connected_at: DateTime<Utc>,
}

/// Real WebSocket manager implementation
#[derive(Clone)]
pub struct RealWebSocketManager {
    /// Active client connections
    clients: Arc<RwLock<HashMap<Uuid, ClientConnection>>>,
}

impl RealWebSocketManager {
    /// Create new WebSocket manager
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clean up disconnected clients   
    async fn cleanup_disconnected_clients(&self) {
        let mut clients = self.clients.write().await;
        let mut to_remove = Vec::new();

        for (client_id, connection) in clients.iter() {
            if connection.sender.is_closed() {
                to_remove.push(*client_id);
            }
        }

        for client_id in to_remove {
            clients.remove(&client_id);
            shared::process_info!(shared::ProcessId::current(), "🗑️ Cleaned up disconnected client {}", client_id);
        }
    }
}

#[async_trait]
impl WebSocketManager for RealWebSocketManager {
    async fn add_client(&self, client_id: Uuid, sender: mpsc::Sender<ClientMessage>) -> WebServerResult<()> {
        let connection = ClientConnection {
            id: client_id,
            sender,
            connected_at: Utc::now(),
        };

        {
            let mut clients = self.clients.write().await;
            clients.insert(client_id, connection);
        }

        shared::process_info!(shared::ProcessId::current(), "👋 Added WebSocket client {}", client_id);

        // Send connection acknowledgment in the background to avoid test interference
        let clients_arc = self.clients.clone();
        tokio::spawn(async move {
            // Small delay to ensure the client is fully registered
            sleep(Duration::from_millis(10)).await;

            let clients = clients_arc.read().await;
            if let Some(connection) = clients.get(&client_id) {
                let ack_message = ClientMessage::ConnectionAck {
                    session_id: client_id,
                    server_time: Utc::now().timestamp() as u64,
                };

                if let Err(e) = connection.sender.try_send(ack_message) {
                    shared::process_warn!(shared::ProcessId::current(), "Failed to send connection ack to {}: {:?}", client_id, e);
                }
            }
        });

        Ok(())
    }

    async fn remove_client(&self, client_id: Uuid) -> WebServerResult<()> {
        let mut clients = self.clients.write().await;
        if clients.remove(&client_id).is_some() {
            shared::process_info!(shared::ProcessId::current(), "👋 Removed WebSocket client {}", client_id);
        }
        Ok(())
    }

    async fn broadcast(&self, message: ClientMessage) -> WebServerResult<()> {
        // Get all clients and their senders at once to avoid holding the lock too long
        let client_senders = {
            let clients = self.clients.read().await;

            if clients.is_empty() {
                shared::process_warn!(shared::ProcessId::current(), "📭 No WebSocket clients connected - message not broadcasted");
                return Ok(());
            }

            shared::process_info!(shared::ProcessId::current(), "📡 Preparing to broadcast message to {} clients", clients.len());

            clients
                .iter()
                .map(|(client_id, connection)| (*client_id, connection.sender.clone()))
                .collect::<Vec<_>>()
        };

        let mut failed_clients = Vec::new();
        let mut success_count = 0;
        let total_clients = client_senders.len();

        for (client_id, sender) in client_senders {
            match sender.try_send(message.clone()) {
                Ok(_) => {
                    success_count += 1;
                }
                Err(TrySendError::Full(_)) => {
                    shared::process_warn!(shared::ProcessId::current(), "Client {} channel full, dropping message", client_id);
                }
                Err(TrySendError::Closed(_)) => {
                    failed_clients.push(client_id);
                }
            }
        }

        // Clean up failed clients
        if !failed_clients.is_empty() {
            let mut clients = self.clients.write().await;
            for client_id in failed_clients {
                if clients.remove(&client_id).is_some() {
                    shared::process_info!(shared::ProcessId::current(), "🗑️ Removed disconnected client {} during broadcast", client_id);
                }
            }
        }

        if success_count > 0 {
            shared::process_info!(shared::ProcessId::current(), "✅ Successfully broadcasted message to {}/{} clients", success_count, total_clients);
        } else {
            shared::process_warn!(shared::ProcessId::current(), "❌ Failed to broadcast message to any clients");
        }

        Ok(())
    }

    async fn send_to_client(&self, client_id: Uuid, message: ClientMessage) -> WebServerResult<()> {
        // Get sender clone to avoid holding the lock during the send operation
        let sender = {
            let clients = self.clients.read().await;
            clients.get(&client_id).map(|connection| connection.sender.clone())
        };

        if let Some(sender) = sender {
            match sender.try_send(message) {
                Ok(_) => {
                    return Ok(());
                }
                Err(TrySendError::Full(_)) => {
                    return Err(WebServerError::websocket("Client channel full".to_string()));
                }
                Err(TrySendError::Closed(_)) => {
                    // Remove the disconnected client
                    let mut clients = self.clients.write().await;
                    if clients.remove(&client_id).is_some() {
                        shared::process_info!(shared::ProcessId::current(), "🗑️ Removed disconnected client {} during individual send", client_id);
                    }
                    return Err(WebServerError::websocket("Client disconnected".to_string()));
                }
            }
        }

        Err(WebServerError::websocket(format!("Client {} not found", client_id)))
    }

    async fn client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }

    async fn active_clients(&self) -> Vec<Uuid> {
        let clients = self.clients.read().await;
        clients.keys().copied().collect()
    }
}

impl Default for RealWebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

// Background task to periodically clean up disconnected clients
impl RealWebSocketManager {
    /// Start background cleanup task
    pub fn start_cleanup_task(&self) -> JoinHandle<()> {
        let clients = self.clients.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let mut client_map = clients.write().await;
                let mut to_remove = Vec::new();

                for (client_id, connection) in client_map.iter() {
                    if connection.sender.is_closed() {
                        to_remove.push(*client_id);
                    }
                }

                if !to_remove.is_empty() {
                    for client_id in to_remove {
                        client_map.remove(&client_id);
                    }
                    shared::process_info!(shared::ProcessId::current(), "🧹 Cleaned up {} disconnected clients", client_map.len());
                }
            }
        })
    }
}
