//! IPC communicator implementation for orchestrator communication

use std::net::SocketAddr;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};

use shared::{ProducerCommand, ProducerUpdate};
use crate::error::{ProducerError, ProducerResult};
use crate::traits::IpcCommunicator;

/// Real IPC communicator using TCP + bincode
pub struct RealIpcCommunicator {
    state: Arc<RwLock<IpcState>>,
}

#[derive(Debug)]
struct IpcState {
    orchestrator_addr: Option<SocketAddr>,
    command_tx: Option<mpsc::Sender<ProducerCommand>>,
    connected: bool,
}

impl RealIpcCommunicator {
    /// Create new IPC communicator
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(IpcState {
                orchestrator_addr: None,
                command_tx: None,
                connected: false,
            })),
        }
    }
}

#[async_trait]
impl IpcCommunicator for RealIpcCommunicator {
    async fn connect(&self, orchestrator_addr: SocketAddr) -> ProducerResult<()> {
        // Test connection
        let _stream = TcpStream::connect(orchestrator_addr)
            .await
            .map_err(|e| ProducerError::IpcError {
                message: format!("Failed to connect to orchestrator: {}", e),
            })?;

        // Update state
        let mut state = self.state.write().await;
        state.orchestrator_addr = Some(orchestrator_addr);
        state.connected = true;

        println!("Connected to orchestrator at {}", orchestrator_addr);
        Ok(())
    }

    async fn listen_for_commands(&self) -> ProducerResult<mpsc::Receiver<ProducerCommand>> {
        let state = self.state.read().await;
        if !state.connected {
            return Err(ProducerError::IpcError {
                message: "Not connected to orchestrator".to_string(),
            });
        }

        let (_tx, rx) = mpsc::channel(100);
        // In real implementation, would start TCP listener here
        // For now, just return the receiver
        Ok(rx)
    }

    async fn send_update(&self, update: ProducerUpdate) -> ProducerResult<()> {
        let state = self.state.read().await;
        if !state.connected {
            return Err(ProducerError::IpcError {
                message: "Not connected to orchestrator".to_string(),
            });
        }

        if let Some(addr) = state.orchestrator_addr {
            let mut stream = TcpStream::connect(addr)
                .await
                .map_err(|e| ProducerError::IpcError {
                    message: format!("Failed to connect to orchestrator: {}", e),
                })?;

            let serialized = bincode::serialize(&update)
                .map_err(|e| ProducerError::SerializationError {
                    message: format!("Failed to serialize update: {}", e),
                })?;

            // Send message length first (4 bytes)
            let len = serialized.len() as u32;
            stream.write_all(&len.to_le_bytes()).await?;
            stream.write_all(&serialized).await?;

            println!("Sent update to orchestrator: {:?}", update);
        }
        Ok(())
    }

    async fn disconnect(&self) -> ProducerResult<()> {
        let mut state = self.state.write().await;
        state.connected = false;
        state.orchestrator_addr = None;
        state.command_tx = None;
        
        println!("Disconnected from orchestrator");
        Ok(())
    }
}