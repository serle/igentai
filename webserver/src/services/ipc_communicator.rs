//! IPC communicator service implementation
//!
//! This service handles inter-process communication with the orchestrator
//! using TCP + bincode for efficient internal messaging.

use std::sync::Arc;
use std::time::Duration;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use shared::{TaskRequest, TaskUpdate};
use crate::traits::IpcCommunicator;
use crate::types::BrowserMessage;
use crate::error::{WebServerError, WebServerResult};
use crate::state::WebServerState;

/// Real IPC communicator implementation
#[derive(Clone)]
pub struct RealIpcCommunicator {
    state: Arc<WebServerState>,
}

impl RealIpcCommunicator {
    /// Create a new orchestrator client
    pub fn new(state: Arc<WebServerState>) -> Self {
        Self { state }
    }
    
    /// Convert browser message to task request
    fn browser_to_task_request(&self, message: BrowserMessage) -> TaskRequest {
        match message {
            BrowserMessage::StartTopic { topic, producer_count, prompt } => {
                TaskRequest::TopicRequest { topic, producer_count, prompt }
            }
            BrowserMessage::StopGeneration => TaskRequest::StopGeneration,
            BrowserMessage::RequestDashboard => TaskRequest::RequestStatus,
            _ => {
                // For other message types, default to status request
                TaskRequest::RequestStatus
            }
        }
    }
    
    /// Send a task request to orchestrator via TCP + bincode
    async fn send_task_request(&self, request: TaskRequest) -> WebServerResult<()> {
        let mut stream = TcpStream::connect(self.state.orchestrator_address).await
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to connect to orchestrator: {}", e)
            })?;
            
        let serialized = bincode::serialize(&request)
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to serialize request: {}", e)
            })?;
            
        // Send message length first (4 bytes)
        let len = serialized.len() as u32;
        stream.write_all(&len.to_le_bytes()).await
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to send message length: {}", e)
            })?;
            
        // Send the serialized message
        stream.write_all(&serialized).await
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to send message: {}", e)
            })?;
            
        println!("📤 Sent task request to orchestrator: {:?}", request);
        Ok(())
    }
    
    /// Handle incoming task update from orchestrator
    async fn handle_task_update(&self, update: TaskUpdate) -> WebServerResult<()> {
        println!("📨 Received task update from orchestrator: {:?}", update);
        
        match update {
            TaskUpdate::SystemMetrics(_metrics) => {
                // Update metrics in state - this would be handled by MetricsAggregator
                println!("📊 Updated system metrics");
                // TODO: Forward to MetricsAggregator service
            }
            TaskUpdate::NewAttributes(attributes) => {
                // Handle new attributes
                println!("📝 Received {} new attributes", attributes.len());
                // TODO: Forward to MetricsAggregator and ClientBroadcaster
            }
            TaskUpdate::SystemStatus { active_producers, current_topic, total_unique } => {
                // Update system status
                println!("🔄 System status: {} producers, topic: {:?}, total: {}", 
                    active_producers, current_topic, total_unique);
                // TODO: Update state and broadcast to clients
            }
            TaskUpdate::ErrorNotification(error) => {
                // Handle error from orchestrator
                eprintln!("❌ Orchestrator error: {}", error);
                // TODO: Broadcast error to clients
            }
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl IpcCommunicator for RealIpcCommunicator {
    async fn connect(&self, orchestrator_addr: SocketAddr) -> WebServerResult<()> {
        println!("🔌 Attempting to connect to orchestrator at {}", orchestrator_addr);
        
        // Test connection by sending a status request
        let test_request = TaskRequest::RequestStatus;
        match self.send_task_request(test_request).await {
            Ok(_) => {
                println!("✅ Connected to orchestrator");
                self.state.set_orchestrator_connected(true);
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to orchestrator: {}", e);
                self.state.set_orchestrator_connected(false);
                Err(e)
            }
        }
    }
    
    async fn disconnect(&self) -> WebServerResult<()> {
        self.state.set_orchestrator_connected(false);
        println!("🔌 Disconnected from orchestrator");
        Ok(())
    }
    
    async fn send_message(&self, message: BrowserMessage) -> WebServerResult<()> {
        // Convert browser message to task request
        let task_request = self.browser_to_task_request(message);
        
        // Send to orchestrator via TCP + bincode
        self.send_task_request(task_request).await
    }
    
    async fn is_connected(&self) -> bool {
        self.state.is_orchestrator_connected()
    }
    
    async fn start_connection_loop(&self) -> WebServerResult<()> {
        println!("🔄 Starting orchestrator connection loop");
        
        // Start a listener for incoming TaskUpdate messages from orchestrator
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to start update listener: {}", e)
            })?;
            
        let local_addr = listener.local_addr()
            .map_err(|e| WebServerError::OrchestratorCommError {
                message: format!("Failed to get listener address: {}", e)
            })?;
            
        println!("🔊 Listening for orchestrator updates on {}", local_addr);
        
        let state = self.state.clone();
        let ipc = self.clone();
        
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, addr)) => {
                        println!("🔗 Accepted connection from orchestrator: {}", addr);
                        
                        // Read message length (4 bytes)
                        let mut len_buf = [0u8; 4];
                        if let Err(e) = stream.read_exact(&mut len_buf).await {
                            eprintln!("Failed to read message length: {}", e);
                            continue;
                        }
                        
                        let msg_len = u32::from_le_bytes(len_buf) as usize;
                        if msg_len > 1024 * 1024 { // 1MB limit
                            eprintln!("Message too large: {} bytes", msg_len);
                            continue;
                        }
                        
                        // Read the message
                        let mut msg_buf = vec![0u8; msg_len];
                        if let Err(e) = stream.read_exact(&mut msg_buf).await {
                            eprintln!("Failed to read message: {}", e);
                            continue;
                        }
                        
                        // Deserialize TaskUpdate
                        match bincode::deserialize::<TaskUpdate>(&msg_buf) {
                            Ok(update) => {
                                if let Err(e) = ipc.handle_task_update(update).await {
                                    eprintln!("Failed to handle task update: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to deserialize task update: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
                
                // Check if we should continue running
                if !state.is_running() {
                    break;
                }
            }
        });
        
        // Main connection monitoring loop
        loop {
            if !self.is_connected().await {
                println!("🔄 Attempting to reconnect to orchestrator...");
                if let Err(e) = self.connect(self.state.orchestrator_address).await {
                    eprintln!("❌ Reconnection failed: {}", e);
                    tokio::time::sleep(self.state.orchestrator_reconnect_interval).await;
                    continue;
                }
            }
            
            // Check if we should continue running
            if !self.state.is_running() {
                break;
            }
            
            // Wait before next connection check
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        
        println!("🔌 Orchestrator connection loop ended");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use crate::types::BrowserMessage;
    use shared::TaskRequest;
    
    #[tokio::test]
    async fn test_orchestrator_client_creation() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let client = RealIpcCommunicator::new(state);
        
        // Initially not connected
        assert!(!client.is_connected().await);
    }
    
    #[tokio::test]
    async fn test_task_request_conversion() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let client = RealIpcCommunicator::new(state);
        
        // Test browser message conversion
        let browser_msg = BrowserMessage::StartTopic {
            topic: "test topic".to_string(),
            producer_count: 5,
            prompt: Some("test prompt".to_string()),
        };
        
        let task_request = client.browser_to_task_request(browser_msg);
        match task_request {
            TaskRequest::TopicRequest { topic, producer_count, prompt } => {
                assert_eq!(topic, "test topic");
                assert_eq!(producer_count, 5);
                assert_eq!(prompt, Some("test prompt".to_string()));
            }
            _ => panic!("Wrong task request type"),
        }
    }
}