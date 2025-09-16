//! Test producer utilities for integration testing

use std::process::{Child, Command, Stdio};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};
use shared::ProducerId;

/// Test producer instance for integration testing
pub struct TestProducer {
    /// Producer process handle
    pub child: Option<Child>,
    
    /// Producer ID
    pub producer_id: ProducerId,
    
    /// Listen address for this producer
    pub listen_addr: SocketAddr,
    
    /// Command address for this producer
    pub command_addr: SocketAddr,
    
    /// Topic this producer is working on
    pub topic: String,
    
    /// Whether this is in test mode
    pub test_mode: bool,
}

impl TestProducer {
    /// Create a new test producer configuration
    pub fn new(
        producer_id: ProducerId,
        listen_addr: SocketAddr,
        command_addr: SocketAddr,
        topic: String,
        test_mode: bool,
    ) -> Self {
        Self {
            child: None,
            producer_id,
            listen_addr,
            command_addr,
            topic,
            test_mode,
        }
    }
    
    /// Start the producer process
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--bin")
            .arg("producer")
            .arg("--")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        
        if self.test_mode {
            // Use topic and test provider for CLI mode
            cmd.arg("--topic").arg(&self.topic);
            cmd.arg("--provider").arg("test");
        } else {
            // For production mode, use orchestrator address
            cmd.arg("--orchestrator-addr").arg(format!("{}:{}", self.listen_addr.ip(), self.listen_addr.port()));
        }
        
        // Set environment variables for API keys (if needed for testing)
        cmd.env("OPENAI_API_KEY", "test-key-for-integration-tests");
        
        let child = cmd.spawn()?;
        self.child = Some(child);
        
        // Give the producer a moment to start up
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        Ok(())
    }
    
    /// Stop the producer process
    pub async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut child) = self.child.take() {
            // Try graceful shutdown first
            let _ = child.kill();
            
            // Wait for process to exit with timeout
            if let Ok(result) = timeout(Duration::from_secs(5), async {
                child.wait()
            }).await {
                result?;
            } else {
                // Force kill if graceful shutdown doesn't work
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        
        Ok(())
    }
    
    /// Check if the producer process is running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Process is still running
                Err(_) => false,      // Error checking status
            }
        } else {
            false
        }
    }
    
}

impl Drop for TestProducer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Manager for multiple test producers
pub struct TestProducerManager {
    /// Active test producers
    producers: Arc<RwLock<Vec<TestProducer>>>,
    
    /// Next port to assign
    next_port: u16,
}

impl TestProducerManager {
    /// Create new test producer manager
    pub fn new(base_port: u16) -> Self {
        Self {
            producers: Arc::new(RwLock::new(Vec::new())),
            next_port: base_port,
        }
    }
    
    /// Create and start a new test producer
    pub async fn spawn_producer(
        &mut self,
        topic: String,
        test_mode: bool,
    ) -> Result<ProducerId, Box<dyn std::error::Error + Send + Sync>> {
        let producer_id = shared::types::ProducerId::from_uuid(uuid::Uuid::new_v4());
        
        let listen_addr = SocketAddr::from(([127, 0, 0, 1], self.next_port));
        self.next_port += 1;
        let command_addr = SocketAddr::from(([127, 0, 0, 1], self.next_port));
        self.next_port += 1;
        
        let mut producer = TestProducer::new(
            producer_id.clone(),
            listen_addr,
            command_addr,
            topic,
            test_mode,
        );
        
        producer.start().await?;
        
        self.producers.write().await.push(producer);
        
        Ok(producer_id)
    }
    
    /// Stop a specific producer
    pub async fn stop_producer(&self, producer_id: &ProducerId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut producers = self.producers.write().await;
        if let Some(pos) = producers.iter().position(|p| &p.producer_id == producer_id) {
            producers[pos].stop().await?;
            producers.remove(pos);
        }
        Ok(())
    }
    
    /// Stop all producers
    pub async fn stop_all(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut producers = self.producers.write().await;
        for producer in producers.iter_mut() {
            producer.stop().await?;
        }
        producers.clear();
        Ok(())
    }
    
    /// Get count of running producers
    pub async fn running_count(&self) -> usize {
        let mut producers = self.producers.write().await;
        let mut running_count = 0;
        for producer in producers.iter_mut() {
            if producer.is_running() {
                running_count += 1;
            }
        }
        running_count
    }
    
    /// Get all producer IDs
    pub async fn get_producer_ids(&self) -> Vec<ProducerId> {
        let producers = self.producers.read().await;
        producers.iter().map(|p| p.producer_id.clone()).collect()
    }
    
    /// Get addresses for a specific producer
    pub async fn get_producer_addresses(&self, producer_id: &ProducerId) -> Option<(SocketAddr, SocketAddr)> {
        let producers = self.producers.read().await;
        producers.iter()
            .find(|p| &p.producer_id == producer_id)
            .map(|p| (p.listen_addr, p.command_addr))
    }
}

impl Drop for TestProducerManager {
    fn drop(&mut self) {
        // Note: This is not async, so we can't wait for graceful shutdown
        // The individual TestProducer Drop impls will handle cleanup
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_producer_manager() {
        let mut manager = TestProducerManager::new(19000);
        
        // Start a test producer
        let producer_id = manager.spawn_producer("test topic".to_string(), true).await.unwrap();
        
        // Check that it's running
        assert_eq!(manager.running_count().await, 1);
        
        // Get addresses
        let addresses = manager.get_producer_addresses(&producer_id).await;
        assert!(addresses.is_some());
        
        // Stop the producer
        manager.stop_producer(&producer_id).await.unwrap();
        assert_eq!(manager.running_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_multiple_producers() {
        let mut manager = TestProducerManager::new(19100);
        
        // Start multiple producers
        manager.spawn_producer("topic1".to_string(), true).await.unwrap();
        manager.spawn_producer("topic2".to_string(), true).await.unwrap();
        manager.spawn_producer("topic3".to_string(), true).await.unwrap();
        
        assert_eq!(manager.running_count().await, 3);
        
        // Stop all
        manager.stop_all().await.unwrap();
        assert_eq!(manager.running_count().await, 0);
    }
}