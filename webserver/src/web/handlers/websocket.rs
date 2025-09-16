//! WebSocket connection handler
//! 
//! Handles WebSocket connections from browser clients

use std::sync::Arc;
use axum::{
    extract::{
        ws::{WebSocket, Message},
        WebSocketUpgrade,
        State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;
use tracing::{info, warn, error, debug};

use crate::traits::WebSocketManager;
use crate::types::{ClientMessage, ClientRequest};
use crate::error::WebServerResult;

/// WebSocket connection handler
pub async fn websocket_handler<W>(
    ws: WebSocketUpgrade,
    State(websocket_manager): State<Arc<W>>,
) -> Response
where
    W: WebSocketManager + 'static,
{
    ws.on_upgrade(|socket| handle_websocket(socket, websocket_manager))
}

/// Handle individual WebSocket connection
async fn handle_websocket<W>(socket: WebSocket, websocket_manager: Arc<W>)
where
    W: WebSocketManager,
{
    let client_id = Uuid::new_v4();
    info!("🔗 New WebSocket connection: {}", client_id);
    
    // Split the socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();
    
    // Create channel for outgoing messages
    let (tx, mut rx) = mpsc::channel::<ClientMessage>(100);
    
    // Register client with WebSocket manager
    if let Err(e) = websocket_manager.add_client(client_id, tx).await {
        error!("Failed to register WebSocket client {}: {}", client_id, e);
        return;
    }
    
    // Spawn task to handle outgoing messages
    let _websocket_manager_clone = websocket_manager.clone();
    let outgoing_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json_msg = match serde_json::to_string(&msg) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to serialize client message: {}", e);
                    continue;
                }
            };
            
            if let Err(e) = sender.send(Message::Text(json_msg)).await {
                warn!("Failed to send message to client {}: {}", client_id, e);
                break;
            }
        }
        
        debug!("Outgoing message task ended for client {}", client_id);
    });
    
    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                warn!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
        };
        
        match msg {
            Message::Text(text) => {
                debug!("📨 Received from client {}: {}", client_id, text);
                
                // Parse client request
                match serde_json::from_str::<ClientRequest>(&text) {
                    Ok(request) => {
                        if let Err(e) = handle_client_request(client_id, request, &websocket_manager).await {
                            error!("Failed to handle client request from {}: {}", client_id, e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to parse client request from {}: {}", client_id, e);
                        
                        // Send error response
                        let error_msg = ClientMessage::Alert {
                            level: crate::types::AlertLevel::Error,
                            title: "Invalid Request".to_string(),
                            message: format!("Failed to parse request: {}", e),
                            timestamp: chrono::Utc::now().timestamp() as u64,
                            dismissible: true,
                        };
                        
                        if let Err(e) = websocket_manager.send_to_client(client_id, error_msg).await {
                            error!("Failed to send error message to client {}: {}", client_id, e);
                        }
                    }
                }
            },
            Message::Binary(_) => {
                warn!("Received binary message from client {} - not supported", client_id);
            },
            Message::Ping(_payload) => {
                debug!("Received ping from client {}", client_id);
                // Note: sender was moved into outgoing_task, so pong handling
                // would need to be restructured. For now, just log the ping.
            },
            Message::Pong(_) => {
                debug!("Received pong from client {}", client_id);
            },
            Message::Close(_) => {
                info!("Client {} requested close", client_id);
                break;
            },
        }
    }
    
    // Cleanup
    outgoing_task.abort();
    
    if let Err(e) = websocket_manager.remove_client(client_id).await {
        error!("Failed to remove client {}: {}", client_id, e);
    }
    
    info!("👋 WebSocket connection closed: {}", client_id);
}

/// Handle individual client request
async fn handle_client_request<W>(
    client_id: Uuid,
    request: ClientRequest,
    websocket_manager: &Arc<W>,
) -> WebServerResult<()>
where
    W: WebSocketManager,
{
    match request {
        ClientRequest::GetDashboard => {
            debug!("Client {} requested dashboard", client_id);
            // Dashboard data would be provided by the main WebServer
            // For now, just acknowledge the request
        },
        
        
        ClientRequest::Ping => {
            debug!("Client {} sent ping", client_id);
            // Respond with current server time
            let response = ClientMessage::StatusUpdate {
                orchestrator_connected: true, // Would get actual status
                active_producers: 0, // Would get actual count
                current_topic: None, // Would get actual topic
                system_health: crate::types::SystemHealth::Healthy,
            };
            
            websocket_manager.send_to_client(client_id, response).await?;
        },
    }
    
    Ok(())
}
