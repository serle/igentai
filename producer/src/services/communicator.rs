//! IPC communication service with orchestrator

use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpStream, TcpListener};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, error, warn, debug};
use shared::{ProducerCommand, ProducerUpdate};
use crate::traits::Communicator;
use crate::error::{ProducerError, ProducerResult};

/// Connection state for the communicator
#[derive(Clone)]
pub struct ConnectionState {
    pub stream: Arc<Mutex<Option<TcpStream>>>,
    pub connected: Arc<RwLock<bool>>,
    pub orchestrator_addr: SocketAddr,
}

/// Real IPC communicator implementation with proper bidirectional communication
pub struct RealCommunicator {
    connection: ConnectionState,
    command_tx: Option<mpsc::Sender<ProducerCommand>>,
    command_rx: Option<mpsc::Receiver<ProducerCommand>>,
    listen_port: Option<u16>,
    standalone_mode: bool,
    producer_id: shared::ProcessId,
}

impl RealCommunicator {
    /// Create new communicator
    pub fn new(orchestrator_addr: SocketAddr, producer_id: shared::ProcessId) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        
        Self {
            connection: ConnectionState {
                stream: Arc::new(Mutex::new(None)),
                connected: Arc::new(RwLock::new(false)),
                orchestrator_addr,
            },
            command_tx: Some(command_tx),
            command_rx: Some(command_rx),
            listen_port: None,
            standalone_mode: false,
            producer_id,
        }
    }
    
    /// Create new communicator in standalone mode (no orchestrator connection)
    pub fn new_standalone(producer_id: shared::ProcessId) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        // Use dummy address for standalone mode
        let dummy_addr = "127.0.0.1:0".parse().unwrap();
        
        Self {
            connection: ConnectionState {
                stream: Arc::new(Mutex::new(None)),
                connected: Arc::new(RwLock::new(true)), // Always "connected" in standalone
                orchestrator_addr: dummy_addr,
            },
            command_tx: Some(command_tx),
            command_rx: Some(command_rx),
            listen_port: None,
            standalone_mode: true,
            producer_id,
        }
    }
    
    /// Create new communicator with listen port
    pub fn with_listen_port(orchestrator_addr: SocketAddr, listen_port: u16, producer_id: shared::ProcessId) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        
        Self {
            connection: ConnectionState {
                stream: Arc::new(Mutex::new(None)),
                connected: Arc::new(RwLock::new(false)),
                orchestrator_addr,
            },
            command_tx: Some(command_tx),
            command_rx: Some(command_rx),
            listen_port: Some(listen_port),
            standalone_mode: false,
            producer_id,
        }
    }
    
    /// Get the listen port (if any)
    pub fn get_listen_port(&self) -> Option<u16> {
        self.listen_port
    }

    /// Read message with length prefix
    async fn read<T>(stream: &mut TcpStream) -> ProducerResult<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Read length prefix
        let mut length_buf = [0u8; 4];
        stream.read_exact(&mut length_buf).await
            .map_err(|e| ProducerError::ipc(format!("Read length failed: {}", e)))?;
        
        let length = u32::from_be_bytes(length_buf) as usize;
        
        // Size validation
        if length > 10 * 1024 * 1024 {
            return Err(ProducerError::ipc(format!("Message too large: {} bytes", length)));
        }
        
        // Read message data
        let mut data = vec![0u8; length];
        stream.read_exact(&mut data).await
            .map_err(|e| ProducerError::ipc(format!("Read data failed: {}", e)))?;
        
        // Deserialize
        bincode::deserialize(&data)
            .map_err(|e| ProducerError::ipc(format!("Deserialize failed: {}", e)))
    }

    /// Write message with length prefix
    async fn write<T>(stream: &mut TcpStream, message: &T) -> ProducerResult<()>
    where
        T: serde::Serialize,
    {
        // Serialize
        let data = bincode::serialize(message)
            .map_err(|e| ProducerError::ipc(format!("Serialize failed: {}", e)))?;
        
        // Size validation
        if data.len() > 10 * 1024 * 1024 {
            return Err(ProducerError::ipc(format!("Message too large: {} bytes", data.len())));
        }
        
        // Write length prefix
        let length = data.len() as u32;
        stream.write_all(&length.to_be_bytes()).await
            .map_err(|e| ProducerError::ipc(format!("Write length failed: {}", e)))?;
        
        // Write data
        stream.write_all(&data).await
            .map_err(|e| ProducerError::ipc(format!("Write data failed: {}", e)))?;
        
        // Flush
        stream.flush().await
            .map_err(|e| ProducerError::ipc(format!("Flush failed: {}", e)))?;
        
        Ok(())
    }

    /// Start command listener task
    async fn start_listener(&self, tx: mpsc::Sender<ProducerCommand>) -> ProducerResult<()> {
        let connection = self.connection.clone();
        
        tokio::spawn(async move {
            loop {
                if !*connection.connected.read().await {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
                
                let mut stream_guard = connection.stream.lock().await;
                if let Some(ref mut stream) = *stream_guard {
                    match Self::read::<ProducerCommand>(stream).await {
                        Ok(command) => {
                            debug!("📨 Received command: {:?}", command);
                            if tx.send(command).await.is_err() {
                                warn!("Command receiver dropped, stopping listener");
                                break;
                            }
                        }
                        Err(e) => {
                            error!("❌ Failed to read command: {}", e);
                            *connection.connected.write().await = false;
                            *connection.stream.lock().await = None;
                            break;
                        }
                    }
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
            
            info!("🛑 Command listener stopped");
        });
        
        Ok(())
    }

    /// Send update with retry logic and exponential backoff
    async fn send_update_with_retries(&self, update: ProducerUpdate, max_retries: u32) -> ProducerResult<()> {
        let mut last_error = None;
        
        for attempt in 0..=max_retries {
            match TcpStream::connect(self.connection.orchestrator_addr).await {
                Ok(mut stream) => {
                    // Send the update
                    match Self::write(&mut stream, &update).await {
                        Ok(()) => {
                            if attempt > 0 {
                                info!("✅ Sent update after {} retries: {:?}", attempt, update);
                            } else {
                                debug!("📤 Sent update: {:?}", update);
                            }
                            return Ok(());
                        }
                        Err(e) => {
                            last_error = Some(e);
                            if attempt < max_retries {
                                let delay = std::time::Duration::from_millis(50 * (attempt + 1) as u64);
                                warn!("⏳ Failed to send update (attempt {}), retrying in {}ms: {}", 
                                      attempt + 1, delay.as_millis(), last_error.as_ref().unwrap());
                                tokio::time::sleep(delay).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(ProducerError::ipc(format!("Failed to connect: {}", e)));
                    if attempt < max_retries {
                        let delay = std::time::Duration::from_millis(50 * (attempt + 1) as u64);
                        warn!("⏳ Failed to connect for update (attempt {}), retrying in {}ms: {}", 
                              attempt + 1, delay.as_millis(), e);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        error!("❌ Failed to send update after {} retries, orchestrator may be unreachable", max_retries);
        Err(last_error.unwrap_or_else(|| ProducerError::ipc("Max retries exceeded")))
    }

}

#[async_trait]
impl Communicator for RealCommunicator {
    async fn initialize(&mut self) -> ProducerResult<()> {
        if self.standalone_mode {
            // Standalone mode: No IPC setup
            info!("🔧 Producer communicator initialized in standalone mode");
            Ok(())
        } else if let Some(port) = self.listen_port {
            // IPC mode: Start listening for commands from orchestrator
            info!("🔊 Starting command listener on port {}", port);
            
            let bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(bind_addr).await
                .map_err(|e| ProducerError::ipc(format!("Failed to bind to {}: {}", bind_addr, e)))?;
            
            info!("✅ Producer listening on {} for commands", bind_addr);
            
            // Mark as connected (ready to receive commands)
            *self.connection.connected.write().await = true;
            
            // Start listener task
            let tx = self.command_tx.clone()
                .ok_or_else(|| ProducerError::ipc("Command sender not available".to_string()))?;
            
            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut stream, addr)) => {
                            debug!("📥 Accepted connection from {}", addr);
                            let tx = tx.clone();
                            
                            // Handle each connection in a separate task with continuous reading
                            tokio::spawn(async move {
                                loop {
                                    match Self::read::<ProducerCommand>(&mut stream).await {
                                        Ok(command) => {
                                            debug!("📨 Received command: {:?}", command);
                                            if tx.send(command).await.is_err() {
                                                warn!("Command receiver dropped, closing connection");
                                                break;
                                            }
                                        }
                                        Err(e) => {
                                            debug!("🔌 Connection closed or error reading command: {}", e);
                                            break;
                                        }
                                    }
                                }
                                debug!("🛑 Connection handler finished");
                            });
                        }
                        Err(e) => {
                            error!("❌ Failed to accept connection: {}", e);
                        }
                    }
                }
            });
            
            // Send ready signal to orchestrator after IPC listener is initialized
            let ready_msg = shared::ProducerUpdate::Ready {
                producer_id: self.producer_id.clone(),
                listen_port: port,
            };
            
            info!("📤 Sending ready signal to orchestrator");
            if let Err(e) = self.send_update(ready_msg).await {
                warn!("⚠️ Failed to send ready signal: {}", e);
            } else {
                info!("✅ Ready signal sent to orchestrator");
            }
            
            Ok(())
        } else {
            // Old behavior: connect to orchestrator
            info!("🔗 Connecting to orchestrator at {}", self.connection.orchestrator_addr);
            
            match TcpStream::connect(self.connection.orchestrator_addr).await {
                Ok(stream) => {
                    info!("✅ Connected to orchestrator successfully");
                    
                    *self.connection.stream.lock().await = Some(stream);
                    *self.connection.connected.write().await = true;
                    
                    Ok(())
                }
                Err(e) => {
                    error!("❌ Failed to connect to orchestrator: {}", e);
                    Err(ProducerError::ipc(format!("Connection failed: {}", e)))
                }
            }
        }
    }

    async fn get_commands(&mut self) -> ProducerResult<mpsc::Receiver<ProducerCommand>> {
        // Take the receiver (can only be called once)
        let rx = self.command_rx.take()
            .ok_or_else(|| ProducerError::ipc("Commands already retrieved".to_string()))?;
        
        // Only start command listener if we're not in listen mode (old behavior)
        if self.listen_port.is_none() {
            let tx = self.command_tx.take()
                .ok_or_else(|| ProducerError::ipc("Command sender not available".to_string()))?;
            
            self.start_listener(tx).await?;
        }
        
        Ok(rx)
    }

    async fn send_update(&self, update: ProducerUpdate) -> ProducerResult<()> {
        if self.standalone_mode {
            // Standalone mode: Just log and ignore
            debug!("📤 Ignoring update in standalone mode: {:?}", update);
            Ok(())
        } else {
            // IPC mode: Send update to orchestrator with retry logic
            self.send_update_with_retries(update, 3).await
        }
    }

    async fn health_check(&self) -> ProducerResult<bool> {
        Ok(*self.connection.connected.read().await)
    }

    fn get_listen_port(&self) -> Option<u16> {
        self.listen_port
    }

    async fn disconnect(&self) -> ProducerResult<()> {
        info!("🔌 Disconnecting from orchestrator");
        
        *self.connection.connected.write().await = false;
        
        let mut stream_guard = self.connection.stream.lock().await;
        if let Some(stream) = stream_guard.take() {
            drop(stream); // Close the connection
        }
        
        info!("✅ Disconnected from orchestrator");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_communicator_creation() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 6001);
        let communicator = RealCommunicator::new(addr);
        
        // Verify initial state
        assert_eq!(communicator.connection.orchestrator_addr, addr);
        // Note: Can't test internal state easily due to async nature
    }

    #[tokio::test]
    async fn test_health_check_without_connection() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 6001);
        let communicator = RealCommunicator::new(addr);
        
        // Should report unhealthy when not connected
        assert!(!communicator.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_send_update_without_connection() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 6001);
        let communicator = RealCommunicator::new(addr);
        
        let update = shared::ProducerUpdate::Pong {
            producer_id: shared::types::ProducerId::from_uuid(uuid::Uuid::new_v4()),
            ping_id: 1,
        };
        
        // Should fail when not connected
        assert!(communicator.send_update(update).await.is_err());
    }

    #[tokio::test] 
    async fn test_message_serialization_limits() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 6001);
        let _communicator = RealCommunicator::new(addr);
        
        // Test message size validation would go here
        // This is tested indirectly through the write_message function
    }
}