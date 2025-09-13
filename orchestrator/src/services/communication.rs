//! Message transport service implementation
//!
//! This module contains the production message transport implementation that manages
//! communication channels between the orchestrator and producer/webserver processes.

use std::collections::HashMap;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use shared::{ProducerId, SystemMetrics, TaskRequest, TaskUpdate};
use crate::error::OrchestratorResult;
use crate::traits::{MessageTransport, ProducerHandle, WebServerHandle};

/// Real message transport using tokio mpsc channels for producers and TCP for webserver
/// 
/// Provides actual inter-process communication coordination using async channels for producers
/// and TCP + bincode protocol for webserver communication using TaskRequest/TaskUpdate messages.
pub struct RealMessageTransport {
    // Producer inbound message receivers keyed by ProducerId
    producers: Arc<Mutex<HashMap<ProducerId, mpsc::Receiver<Vec<String>>>>>,
    // Webserver TCP address for sending TaskUpdate messages
    webserver_address: Arc<Mutex<Option<SocketAddr>>>,
    // Channel for queuing TaskRequest messages from webserver
    webserver_requests: Arc<Mutex<Option<mpsc::Receiver<TaskRequest>>>>,
}

impl RealMessageTransport {
    /// Create a new message transport service instance
    pub fn new() -> Self {
        Self {
            producers: Arc::new(Mutex::new(HashMap::new())),
            webserver_address: Arc::new(Mutex::new(None)),
            webserver_requests: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Send TaskUpdate message to webserver via TCP + bincode (internal implementation)
    async fn send_task_update_internal(&self, update: TaskUpdate) -> OrchestratorResult<()> {
        let webserver_addr = {
            let addr_guard = self.webserver_address.lock().await;
            addr_guard.clone()
        };
        
        if let Some(addr) = webserver_addr {
            match tokio::net::TcpStream::connect(addr).await {
                Ok(mut stream) => {
                    let serialized = bincode::serialize(&update)
                        .map_err(|e| crate::error::OrchestratorError::SerializationError {
                            message: format!("Failed to serialize TaskUpdate: {}", e)
                        })?;
                    
                    // Send message length first (4 bytes)
                    let len = serialized.len() as u32;
                    if let Err(e) = stream.write_all(&len.to_le_bytes()).await {
                        println!("Failed to send message length to webserver: {}", e);
                        return Ok(()); // Don't fail orchestrator if webserver is down
                    }
                    
                    // Send the serialized message
                    if let Err(e) = stream.write_all(&serialized).await {
                        println!("Failed to send TaskUpdate to webserver: {}", e);
                        return Ok(()); // Don't fail orchestrator if webserver is down
                    }
                    
                    println!("📤 Sent TaskUpdate to webserver: {:?}", update);
                }
                Err(e) => {
                    println!("Failed to connect to webserver at {}: {}", addr, e);
                    // Don't fail orchestrator if webserver is temporarily unavailable
                }
            }
        } else {
            println!("No webserver address configured for TaskUpdate");
        }
        
        Ok(())
    }
    
    /// Start TCP listener for TaskRequest messages from webserver (internal implementation)
    pub async fn start_webserver_listener(&self, listen_addr: SocketAddr) -> OrchestratorResult<mpsc::Receiver<TaskRequest>> {
        let listener = TcpListener::bind(listen_addr).await
            .map_err(|e| crate::error::OrchestratorError::NetworkError {
                message: format!("Failed to bind TCP listener for webserver requests: {}", e)
            })?;
        
        let (tx, rx) = mpsc::channel::<TaskRequest>(100);
        
        println!("🔊 Listening for webserver TaskRequest messages on {}", listen_addr);
        
        // Spawn task to handle incoming connections
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, addr)) => {
                        println!("🔗 Accepted webserver connection from: {}", addr);
                        
                        let tx_clone = tx.clone();
                        tokio::spawn(async move {
                            // Read message length (4 bytes)
                            let mut len_buf = [0u8; 4];
                            if let Err(e) = stream.read_exact(&mut len_buf).await {
                                eprintln!("Failed to read message length from webserver: {}", e);
                                return;
                            }
                            
                            let msg_len = u32::from_le_bytes(len_buf) as usize;
                            if msg_len > 1024 * 1024 { // 1MB limit
                                eprintln!("TaskRequest message too large: {} bytes", msg_len);
                                return;
                            }
                            
                            // Read the message
                            let mut msg_buf = vec![0u8; msg_len];
                            if let Err(e) = stream.read_exact(&mut msg_buf).await {
                                eprintln!("Failed to read TaskRequest message: {}", e);
                                return;
                            }
                            
                            // Deserialize TaskRequest
                            match bincode::deserialize::<TaskRequest>(&msg_buf) {
                                Ok(request) => {
                                    println!("📨 Received TaskRequest from webserver: {:?}", request);
                                    if let Err(e) = tx_clone.send(request).await {
                                        eprintln!("Failed to forward TaskRequest to orchestrator: {}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to deserialize TaskRequest: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to accept webserver connection: {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        
        Ok(rx)
    }
}

#[async_trait::async_trait]
impl MessageTransport for RealMessageTransport {
    async fn establish_producer_channels(
        &self,
        producer_handles: Vec<ProducerHandle>,
    ) -> OrchestratorResult<()> {
        let mut map = self.producers.lock().await;
        let count = producer_handles.len();
        for handle in producer_handles {
            map.insert(handle.id.clone(), handle.inbound);
        }
        println!("Registered {} producer channel(s)", count);
        Ok(())
    }

    async fn establish_webserver_channel(
        &self,
        webserver_handle: WebServerHandle,
    ) -> OrchestratorResult<()> {
        // Store webserver address for sending TaskUpdate messages
        let mut addr_guard = self.webserver_address.lock().await;
        *addr_guard = Some(webserver_handle.address);
        println!("Webserver communication established with address: {:?}", webserver_handle.address);
        Ok(())
    }

    async fn reestablish_producer_channel(
        &self,
        new_handle: ProducerHandle,
    ) -> OrchestratorResult<()> {
        let mut map = self.producers.lock().await;
        map.insert(new_handle.id.clone(), new_handle.inbound);
        println!(
            "Reestablished communication channel for producer {}",
            new_handle.id
        );
        Ok(())
    }

    async fn process_messages(&self) -> OrchestratorResult<Vec<(ProducerId, Vec<String>)>> {
        use tokio::sync::mpsc::error::TryRecvError;

        let mut map = self.producers.lock().await;
        let mut all_batches: Vec<(ProducerId, Vec<String>)> = Vec::new();

        // Drain each receiver non-blockingly to collect available batches
        for (pid, rx) in map.iter_mut() {
            loop {
                match rx.try_recv() {
                    Ok(batch) => {
                        all_batches.push((pid.clone(), batch));
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        println!("Producer channel closed for {}", pid);
                        break;
                    }
                }
            }
        }

        Ok(all_batches)
    }

    async fn send_updates(&self, metrics: SystemMetrics) -> OrchestratorResult<()> {
        // Send SystemMetrics as TaskUpdate::SystemMetrics
        let task_update = TaskUpdate::SystemMetrics(metrics);
        self.send_task_update_internal(task_update).await
    }
    
    async fn start_task_request_listener(&self, listen_addr: SocketAddr) -> OrchestratorResult<mpsc::Receiver<TaskRequest>> {
        self.start_webserver_listener(listen_addr).await
    }
    
    async fn send_task_update(&self, update: TaskUpdate) -> OrchestratorResult<()> {
        // Send TaskUpdate via internal method
        self.send_task_update_internal(update).await
    }

    async fn shutdown_communication(&self) -> OrchestratorResult<()> {
        // Dropping all senders/receivers will close channels and stop tasks
        self.producers.lock().await.clear();
        *self.webserver_address.lock().await = None;
        *self.webserver_requests.lock().await = None;
        println!("Communication channels shut down");
        Ok(())
    }
}

