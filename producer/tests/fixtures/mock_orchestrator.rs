//! Mock orchestrator for testing producer communication

use shared::{ProducerCommand, ProcessId, ProducerUpdate};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};

/// Mock orchestrator for testing producer communication
pub struct MockOrchestrator {
    /// Address to bind the mock orchestrator server
    pub listen_addr: SocketAddr,

    /// Received producer updates
    pub received_updates: Arc<RwLock<Vec<ProducerUpdate>>>,

    /// Commands to send to producers
    pub commands_to_send: Arc<Mutex<Vec<ProducerCommand>>>,

    /// Connected producer streams
    producer_streams: Arc<RwLock<HashMap<ProcessId, TcpStream>>>,

    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

#[allow(dead_code)] // Some utility methods may not be used in current tests
impl MockOrchestrator {
    /// Create new mock orchestrator
    pub fn new(listen_addr: SocketAddr) -> Self {
        let (shutdown_tx, _) = mpsc::channel(1);

        Self {
            listen_addr,
            received_updates: Arc::new(RwLock::new(Vec::new())),
            commands_to_send: Arc::new(Mutex::new(Vec::new())),
            producer_streams: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Start the mock orchestrator server
    pub async fn start(&mut self) -> Result<mpsc::Sender<()>, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.listen_addr).await?;
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

        let received_updates = self.received_updates.clone();
        let commands_to_send = self.commands_to_send.clone();
        let producer_streams = self.producer_streams.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Accept new connections
                    Ok((stream, addr)) = listener.accept() => {
                        let received_updates = received_updates.clone();
                        let commands_to_send = commands_to_send.clone();
                        let producer_streams = producer_streams.clone();

                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_producer_connection(
                                stream,
                                addr,
                                received_updates,
                                commands_to_send,
                                producer_streams
                            ).await {
                                eprintln!("Error handling producer connection: {}", e);
                            }
                        });
                    },

                    // Handle shutdown
                    _ = shutdown_rx.recv() => {
                        println!("MockOrchestrator shutting down");
                        break;
                    }
                }
            }
        });

        self.shutdown_tx = Some(shutdown_tx.clone());
        Ok(shutdown_tx)
    }

    /// Handle individual producer connection
    async fn handle_producer_connection(
        mut stream: TcpStream,
        addr: SocketAddr,
        received_updates: Arc<RwLock<Vec<ProducerUpdate>>>,
        commands_to_send: Arc<Mutex<Vec<ProducerCommand>>>,
        producer_streams: Arc<RwLock<HashMap<ProcessId, TcpStream>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Producer connected from: {}", addr);

        let mut buffer = vec![0; 4096];
        let mut producer_id_opt: Option<ProcessId> = None;

        loop {
            tokio::select! {
                // Read messages from producer
                Ok(n) = stream.read(&mut buffer) => {
                    if n == 0 {
                        println!("Producer {} disconnected", addr);
                        break;
                    }

                    // Parse the message (simplified - in reality would use bincode)
                    if let Ok(update) = bincode::deserialize::<ProducerUpdate>(&buffer[..n]) {
                        println!("Received update from producer: {:?}", update);

                        // Extract producer ID from first update for tracking
                        if producer_id_opt.is_none() {
                            producer_id_opt = Some(match &update {
                                ProducerUpdate::AttributeBatch { producer_id, .. } => producer_id.clone(),
                                ProducerUpdate::StatusUpdate { producer_id, .. } => producer_id.clone(),
                                ProducerUpdate::SyncAck { producer_id, .. } => producer_id.clone(),
                                ProducerUpdate::Pong { producer_id, .. } => producer_id.clone(),
                                ProducerUpdate::Error { producer_id, .. } => producer_id.clone(),
                                _ => ProcessId::default(),
                            });
                        }

                        received_updates.write().await.push(update);
                    }
                },

                // Send commands to producer
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                    let mut commands = commands_to_send.lock().await;
                    if let Some(command) = commands.pop() {
                        let serialized = bincode::serialize(&command)?;
                        if let Err(e) = stream.write_all(&serialized).await {
                            eprintln!("Failed to send command to producer: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        // Clean up producer stream tracking when connection closes
        if let Some(producer_id) = producer_id_opt {
            producer_streams.write().await.remove(&producer_id);
            println!("Removed producer {} from tracking", producer_id);
        }

        Ok(())
    }

    /// Add command to send queue
    pub async fn queue_command(&self, command: ProducerCommand) {
        self.commands_to_send.lock().await.push(command);
    }

    /// Get all received updates
    pub async fn get_received_updates(&self) -> Vec<ProducerUpdate> {
        self.received_updates.read().await.clone()
    }

    /// Clear received updates
    pub async fn clear_received_updates(&self) {
        self.received_updates.write().await.clear();
    }

    /// Wait for specific number of updates
    pub async fn wait_for_updates(&self, count: usize, timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if self.received_updates.read().await.len() >= count {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        false
    }

    /// Get number of connected producers
    pub async fn connected_producer_count(&self) -> usize {
        self.producer_streams.read().await.len()
    }

    /// Get list of connected producer IDs
    pub async fn get_connected_producers(&self) -> Vec<ProcessId> {
        self.producer_streams.read().await.keys().cloned().collect()
    }

    /// Send command to specific producer (if connected)
    pub async fn send_command_to_producer(&self, producer_id: &ProcessId, command: ProducerCommand) -> bool {
        let mut streams = self.producer_streams.write().await;
        if let Some(stream) = streams.get_mut(producer_id) {
            if let Ok(serialized) = bincode::serialize(&command) {
                stream.write_all(&serialized).await.is_ok()
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Shutdown the mock orchestrator
    pub async fn shutdown(&self) {
        if let Some(ref shutdown_tx) = self.shutdown_tx {
            let _ = shutdown_tx.send(()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_mock_orchestrator_startup() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let mut mock_orch = MockOrchestrator::new(addr);

        let shutdown_tx = mock_orch.start().await.unwrap();

        // Give it a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Shutdown
        shutdown_tx.send(()).await.unwrap();
    }
}
