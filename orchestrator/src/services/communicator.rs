//! Real communication service implementation
//! 
//! Handles TCP-based communication between orchestrator and external processes
//! using the new message types and clean async interfaces.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared::{
    ProcessId, WebServerRequest, OrchestratorUpdate, 
    OrchestratorCommand, ProducerUpdate, process_debug
};
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::traits::Communicator;

/// Real communicator implementation using TCP + bincode protocol
pub struct RealCommunicator {
    /// Producer addresses for sending commands
    producer_addresses: Arc<Mutex<HashMap<ProcessId, SocketAddr>>>,
    
    /// Producer readiness state
    producer_ready: Arc<Mutex<HashMap<ProcessId, bool>>>,
    
    /// WebServer address for sending updates
    webserver_address: Arc<Mutex<Option<SocketAddr>>>,
    
    /// WebServer readiness state
    webserver_ready: Arc<Mutex<bool>>,
    
    /// Active TCP listeners for cleanup
    active_listeners: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl RealCommunicator {
    /// Create new communicator
    pub fn new() -> Self {
        Self {
            producer_addresses: Arc::new(Mutex::new(HashMap::new())),
            producer_ready: Arc::new(Mutex::new(HashMap::new())),
            webserver_address: Arc::new(Mutex::new(None)),
            webserver_ready: Arc::new(Mutex::new(false)),
            active_listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Mark webserver as ready to receive updates
    pub async fn mark_webserver_ready(&self, address: SocketAddr) -> OrchestratorResult<()> {
        {
            let mut webserver_addr = self.webserver_address.lock().await;
            *webserver_addr = Some(address);
        }
        
        {
            let mut ready = self.webserver_ready.lock().await;
            *ready = true;
        }
        
        process_debug!(shared::ProcessId::current(), "✅ WebServer marked as ready at {}", address);
        Ok(())
    }
    
    /// Mark producer as ready to receive commands
    pub async fn mark_producer_ready(&self, producer_id: ProcessId, address: SocketAddr) -> OrchestratorResult<()> {
        {
            let mut addresses = self.producer_addresses.lock().await;
            addresses.insert(producer_id.clone(), address);
        }
        
        {
            let mut ready = self.producer_ready.lock().await;
            ready.insert(producer_id.clone(), true);
        }
        
        process_debug!(shared::ProcessId::current(), "✅ Producer {} marked as ready at {}", producer_id, address);
        Ok(())
    }
    
    /// Send message via TCP + bincode
    async fn send<T: serde::Serialize>(&self, address: SocketAddr, message: &T) -> OrchestratorResult<()> {
        match TcpStream::connect(address).await {
            Ok(mut stream) => {
                let data = bincode::serialize(message)
                    .map_err(|e| OrchestratorError::communication(format!("Serialize failed: {}", e)))?;
                
                // Write length prefix + data
                let len = data.len() as u32;
                stream.write_all(&len.to_be_bytes()).await
                    .map_err(|e| OrchestratorError::communication(format!("Write length failed: {}", e)))?;
                stream.write_all(&data).await
                    .map_err(|e| OrchestratorError::communication(format!("Write data failed: {}", e)))?;
                
                Ok(())
            },
            Err(e) => Err(OrchestratorError::communication(format!("Connect failed to {}: {}", address, e)))
        }
    }
    
    /// Start TCP listener for incoming messages
    async fn listen<T, F>(
        &self,
        bind_addr: SocketAddr,
        tx: mpsc::Sender<T>,
        parser: F,
    ) -> OrchestratorResult<()>
    where
        T: Send + 'static,
        F: Fn(Vec<u8>) -> OrchestratorResult<T> + Send + Clone + 'static,
    {
        let listener = TcpListener::bind(bind_addr).await
            .map_err(|e| OrchestratorError::communication(format!("Failed to bind to {}: {}", bind_addr, e)))?;
        
        let handle = tokio::spawn(async move {
            while let Ok((mut stream, _addr)) = listener.accept().await {
                let tx_clone = tx.clone();
                let parser_clone = parser.clone();
                
                tokio::spawn(async move {
                    // Read length prefix
                    let mut len_bytes = [0u8; 4];
                    if stream.read_exact(&mut len_bytes).await.is_err() {
                        return;
                    }
                    let len = u32::from_be_bytes(len_bytes) as usize;
                    
                    // Size validation
                    if len > 1024 * 1024 {
                        return;
                    }
                    
                    // Read message data
                    let mut data = vec![0u8; len];
                    if stream.read_exact(&mut data).await.is_err() {
                        return;
                    }
                    
                    // Parse and send
                    if let Ok(message) = parser_clone(data) {
                        let _ = tx_clone.send(message).await;
                    }
                });
            }
        });
        
        // Store handle for cleanup
        {
            let mut listeners = self.active_listeners.lock().await;
            listeners.push(handle);
        }
        
        Ok(())
    }
}

#[async_trait]
impl Communicator for RealCommunicator {
    async fn start_webserver_listener(&self, bind_addr: SocketAddr) -> OrchestratorResult<mpsc::Receiver<WebServerRequest>> {
        let (tx, rx) = mpsc::channel(100);
        
        self.listen(
            bind_addr,
            tx,
            |data| {
                bincode::deserialize::<WebServerRequest>(&data)
                    .map_err(|e| OrchestratorError::communication(format!("Failed to parse WebServerRequest: {}", e)))
            }
        ).await?;
        
        process_debug!(shared::ProcessId::current(), "🌐 WebServer listener started on {}", bind_addr);
        Ok(rx)
    }
    
    async fn send_webserver_update(&self, update: OrchestratorUpdate) -> OrchestratorResult<()> {
        let (address, ready) = {
            let webserver_addr = self.webserver_address.lock().await;
            let ready = self.webserver_ready.lock().await;
            (webserver_addr.clone(), *ready)
        };
        
        if ready {
            if let Some(addr) = address {
                self.send(addr, &update).await?;
            }
        } else {
            // Silently ignore if webserver not ready yet
        }
        
        Ok(())
    }
    
    async fn start_producer_listener(&self, bind_addr: SocketAddr) -> OrchestratorResult<mpsc::Receiver<ProducerUpdate>> {
        let (tx, rx) = mpsc::channel(1000); // Larger buffer for producer messages
        
        self.listen(
            bind_addr,
            tx,
            |data| {
                bincode::deserialize::<ProducerUpdate>(&data)
                    .map_err(|e| OrchestratorError::communication(format!("Failed to parse ProducerUpdate: {}", e)))
            }
        ).await?;
        
        process_debug!(shared::ProcessId::current(), "🏭 Producer listener started on {}", bind_addr);
        Ok(rx)
    }
    
    async fn send_producer_command(&self, producer_id: ProcessId, command: OrchestratorCommand) -> OrchestratorResult<()> {
        let (address, ready) = {
            let addresses = self.producer_addresses.lock().await;
            let ready_map = self.producer_ready.lock().await;
            let address = addresses.get(&producer_id).cloned();
            let ready = ready_map.get(&producer_id).copied().unwrap_or(false);
            (address, ready)
        };
        
        if ready {
            if let Some(addr) = address {
                self.send(addr, &command).await?;
                process_debug!(shared::ProcessId::current(), "📤 Sent command to producer {}: {:?}", producer_id, command);
            } else {
                return Err(OrchestratorError::communication(format!("Producer {} not registered", producer_id)));
            }
        } else {
            // Silently ignore if producer not ready yet
            process_debug!(shared::ProcessId::current(), "⏳ Producer {} not ready yet, ignoring command", producer_id);
        }
        
        Ok(())
    }
    
    async fn register_producer(&self, producer_id: ProcessId, address: SocketAddr) -> OrchestratorResult<()> {
        {
            let mut addresses = self.producer_addresses.lock().await;
            addresses.insert(producer_id.clone(), address);
        }
        
        process_debug!(shared::ProcessId::current(), "📝 Registered producer {} at {}", producer_id, address);
        Ok(())
    }
    
    async fn mark_producer_ready(&self, producer_id: ProcessId, address: SocketAddr) -> OrchestratorResult<()> {
        // Call the impl method (not trait method to avoid recursion)
        RealCommunicator::mark_producer_ready(self, producer_id, address).await
    }
    
    async fn register_webserver(&self, address: SocketAddr) -> OrchestratorResult<()> {
        {
            let mut webserver_addr = self.webserver_address.lock().await;
            *webserver_addr = Some(address);
        }
        
        process_debug!(shared::ProcessId::current(), "🌐 Registered webserver at {}", address);
        Ok(())
    }
    
    async fn mark_webserver_ready(&self, address: SocketAddr) -> OrchestratorResult<()> {
        // Call the impl method (not trait method to avoid recursion)
        RealCommunicator::mark_webserver_ready(self, address).await
    }
    
    async fn shutdown(&self) -> OrchestratorResult<()> {
        // Cancel all active listeners
        let listeners = {
            let mut active = self.active_listeners.lock().await;
            std::mem::take(&mut *active)
        };
        
        for handle in listeners {
            handle.abort();
        }
        
        // Clear all state
        {
            let mut addresses = self.producer_addresses.lock().await;
            addresses.clear();
        }
        {
            let mut webserver = self.webserver_address.lock().await;
            *webserver = None;
        }
        
        process_debug!(shared::ProcessId::current(), "🔌 Communication channels shut down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::ProcessId;
    
    #[tokio::test]
    async fn test_communicator_creation() {
        let comm = RealCommunicator::new();
        
        // Should start with empty state
        assert!(comm.producer_addresses.lock().await.is_empty());
        assert!(comm.webserver_address.lock().await.is_none());
    }
    
    #[tokio::test]
    async fn test_producer_registration() {
        let comm = RealCommunicator::new();
        let producer_id = ProcessId::Producer(1);
        let addr = "127.0.0.1:8001".parse().unwrap();
        
        // Register producer
        let result = comm.register_producer(producer_id.clone(), addr).await;
        assert!(result.is_ok());
        
        // Should be registered
        let addresses = comm.producer_addresses.lock().await;
        assert_eq!(addresses.get(&producer_id), Some(&addr));
    }
    
    #[tokio::test]
    async fn test_shutdown() {
        let comm = RealCommunicator::new();
        let producer_id = ProcessId::Producer(2);
        let addr = "127.0.0.1:8002".parse().unwrap();
        
        // Register something
        comm.register_producer(producer_id, addr).await.unwrap();
        
        // Shutdown should clear everything
        let result = comm.shutdown().await;
        assert!(result.is_ok());
        assert!(comm.producer_addresses.lock().await.is_empty());
    }
}