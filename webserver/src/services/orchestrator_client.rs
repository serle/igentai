//! Orchestrator IPC client - matches producer communication pattern
//!
//! Implements bidirectional communication like producers:
//! - Listens on assigned port for orchestrator updates
//! - Connects to orchestrator to send requests

use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tracing::debug;

use crate::error::{WebServerError, WebServerResult};
use crate::traits::OrchestratorClient;
use shared::{OrchestratorUpdate, ProcessId, WebServerRequest, process_debug, process_info};

/// Connection state for the communicator  
#[derive(Clone)]
pub struct ConnectionState {
    pub connected: Arc<RwLock<bool>>,
    pub orchestrator_addr: SocketAddr,
}

/// Real orchestrator client implementation using producer pattern
pub struct RealOrchestratorClient {
    connection: ConnectionState,
    update_tx: Option<mpsc::Sender<OrchestratorUpdate>>,
    update_rx: Option<mpsc::Receiver<OrchestratorUpdate>>,
    listen_port: Option<u16>, // None = standalone mode
}

impl RealOrchestratorClient {
    /// Create new orchestrator client with IPC
    pub fn new(bind_addr: SocketAddr, orchestrator_addr: SocketAddr) -> Self {
        let (update_tx, update_rx) = mpsc::channel(100);

        Self {
            connection: ConnectionState {
                connected: Arc::new(RwLock::new(false)),
                orchestrator_addr,
            },
            update_tx: Some(update_tx),
            update_rx: Some(update_rx),
            listen_port: Some(bind_addr.port()),
        }
    }

    /// Create new orchestrator client in standalone mode (no IPC)
    pub fn new_standalone() -> Self {
        let (update_tx, update_rx) = mpsc::channel(100);
        // Use dummy address for standalone mode
        let dummy_addr = "127.0.0.1:0".parse().unwrap();

        Self {
            connection: ConnectionState {
                connected: Arc::new(RwLock::new(true)), // Always "connected" in standalone
                orchestrator_addr: dummy_addr,
            },
            update_tx: Some(update_tx),
            update_rx: Some(update_rx),
            listen_port: None, // No IPC in standalone mode
        }
    }

    /// Read message with length prefix (copied from producer)
    async fn read<T>(stream: &mut TcpStream) -> WebServerResult<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Read length prefix
        let mut length_buf = [0u8; 4];
        stream
            .read_exact(&mut length_buf)
            .await
            .map_err(|e| WebServerError::communication(format!("Read length failed: {e}")))?;

        let length = u32::from_be_bytes(length_buf) as usize;

        // Size validation
        if length > 10 * 1024 * 1024 {
            return Err(WebServerError::communication(format!(
                "Message too large: {length} bytes"
            )));
        }

        // Read message data
        let mut data = vec![0u8; length];
        stream
            .read_exact(&mut data)
            .await
            .map_err(|e| WebServerError::communication(format!("Read data failed: {e}")))?;

        // Deserialize
        bincode::deserialize(&data).map_err(|e| WebServerError::communication(format!("Deserialize failed: {e}")))
    }

    /// Write message with length prefix (copied from producer)
    async fn write<T>(stream: &mut TcpStream, message: &T) -> WebServerResult<()>
    where
        T: serde::Serialize,
    {
        // Serialize
        let data = bincode::serialize(message)
            .map_err(|e| WebServerError::communication(format!("Serialize failed: {e}")))?;

        // Size validation
        if data.len() > 10 * 1024 * 1024 {
            return Err(WebServerError::communication(format!(
                "Message too large: {} bytes",
                data.len()
            )));
        }

        // Write length prefix
        let length = data.len() as u32;
        stream
            .write_all(&length.to_be_bytes())
            .await
            .map_err(|e| WebServerError::communication(format!("Write length failed: {e}")))?;

        // Write data
        stream
            .write_all(&data)
            .await
            .map_err(|e| WebServerError::communication(format!("Write data failed: {e}")))?;

        // Flush
        stream
            .flush()
            .await
            .map_err(|e| WebServerError::communication(format!("Flush failed: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl OrchestratorClient for RealOrchestratorClient {
    async fn initialize(&mut self) -> WebServerResult<()> {
        if let Some(port) = self.listen_port {
            // IPC mode: Start listening for updates from orchestrator (like producer does)
            process_debug!(ProcessId::current(), "🔊 Starting update listener on port {}", port);

            let bind_addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(bind_addr)
                .await
                .map_err(|e| WebServerError::communication(format!("Failed to bind to {bind_addr}: {e}")))?;

            process_debug!(
                ProcessId::current(),
                "✅ WebServer listening on {} for orchestrator updates",
                bind_addr
            );

            // Don't mark as connected yet - wait for orchestrator to actually connect

            // Start listener task
            let tx = self
                .update_tx
                .clone()
                .ok_or_else(|| WebServerError::communication("Update sender not available".to_string()))?;
            let connected = self.connection.connected.clone();

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut stream, addr)) => {
                            debug!("📥 Accepted connection from {}", addr);
                            
                            // Mark as connected when orchestrator connects
                            *connected.write().await = true;
                            
                            let tx = tx.clone();
                            let connected_inner = connected.clone();

                            // Handle each connection in a separate task
                            tokio::spawn(async move {
                                match Self::read::<OrchestratorUpdate>(&mut stream).await {
                                    Ok(update) => {
                                        debug!("📨 Received update: {:?}", update);
                                        if tx.send(update).await.is_err() {
                                            shared::process_warn!(shared::ProcessId::current(), "Update receiver dropped");
                                        }
                                    }
                                    Err(e) => {
                                        shared::process_error!(shared::ProcessId::current(), "❌ Failed to read update: {}", e);
                                        // Mark as disconnected on communication error
                                        *connected_inner.write().await = false;
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            shared::process_error!(shared::ProcessId::current(), "❌ Failed to accept connection: {}", e);
                        }
                    }
                }
            });

            // Send ready signal to orchestrator
            let ready_msg = WebServerRequest::Ready {
                listen_port: port,
                http_port: 8080, // TODO: Make this configurable
            };

            process_debug!(ProcessId::current(), "📤 Sending ready signal to orchestrator");
            if let Err(e) = self.send_request(ready_msg).await {
                shared::process_warn!(shared::ProcessId::current(), "⚠️ Failed to send ready signal: {}", e);
            } else {
                process_info!(ProcessId::current(), "✅ WebServer ready and connected to orchestrator");
            }
        } else {
            // Standalone mode: No IPC setup
            process_info!(ProcessId::current(), "🔧 WebServer ready in standalone mode");
        }

        Ok(())
    }

    async fn send_request(&self, request: WebServerRequest) -> WebServerResult<()> {
        if self.listen_port.is_some() {
            // IPC mode: Send request to orchestrator
            match TcpStream::connect(self.connection.orchestrator_addr).await {
                Ok(mut stream) => {
                    // Send the request
                    match Self::write(&mut stream, &request).await {
                        Ok(()) => {
                            debug!("📤 Sent request: {:?}", request);
                            Ok(())
                        }
                        Err(e) => {
                            shared::process_error!(shared::ProcessId::current(), "❌ Failed to send request: {}", e);
                            Err(e)
                        }
                    }
                }
                Err(e) => {
                    shared::process_error!(shared::ProcessId::current(), "❌ Failed to connect for request: {}", e);
                    Err(WebServerError::communication(format!("Failed to connect: {e}")))
                }
            }
        } else {
            // Standalone mode: Just log and ignore
            process_debug!(shared::ProcessId::current(), "📤 Ignoring request in standalone mode: {:?}", request);
            Ok(())
        }
    }

    async fn get_updates(&mut self) -> WebServerResult<mpsc::Receiver<OrchestratorUpdate>> {
        // Take the receiver (can only be called once)
        self.update_rx
            .take()
            .ok_or_else(|| WebServerError::communication("Updates already retrieved".to_string()))
    }

    async fn health_check(&self) -> WebServerResult<bool> {
        Ok(*self.connection.connected.read().await)
    }

    async fn disconnect(&self) -> WebServerResult<()> {
        shared::process_info!(shared::ProcessId::current(), "🔌 Disconnecting from orchestrator");

        *self.connection.connected.write().await = false;

        shared::process_info!(shared::ProcessId::current(), "✅ Disconnected from orchestrator");
        Ok(())
    }
}

// Note: No Drop implementation needed as the tokio::spawn handles cleanup automatically
