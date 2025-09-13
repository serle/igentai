//! Trait definitions with mockall annotations for testing
//! 
//! This module contains all the trait definitions extracted from the orchestrator
//! components with proper mockall mock generation annotations. These traits are
//! used for dependency injection and enable comprehensive testing.

use shared::{ProducerId, SystemMetrics, ProcessHealth, KeyValuePair};
use crate::error::OrchestratorResult;
use crate::services::optimizer::{OptimizationStats, OptimizationPlan};
use tokio::sync::mpsc;

/// Error when a required API key is missing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredKeyMissing {
    pub key_name: String,
    pub message: String,
}

/// Handle for a producer's inbound channel to the orchestrator/transport
#[derive(Debug)]
pub struct ProducerHandle {
    pub id: ProducerId,
    pub inbound: mpsc::Receiver<Vec<String>>,
}

/// Handle for webserver TCP communication (orchestrator <-> webserver)
#[derive(Debug)]
pub struct WebServerHandle {
    pub address: std::net::SocketAddr,
}

/// API key source abstraction for dependency injection
/// 
/// This trait provides API key management for different LLM providers,
/// enabling secure credential injection into producer processes.
#[mockall::automock]
#[async_trait::async_trait]
pub trait ApiKeySource: Send + Sync {
    /// Retrieve all available API keys with validation
    /// 
    /// # Returns
    /// Result containing vector of key-value pairs for available API keys,
    /// or RequiredKeyMissing error if required keys are not available
    async fn get_api_keys(&self) -> Result<Vec<KeyValuePair>, RequiredKeyMissing>;
}

/// Process management abstraction - supports both producer and webserver child processes
/// 
/// This trait handles spawning, monitoring, and managing producer and webserver processes
/// with their associated communication channels.
#[mockall::automock]
#[async_trait::async_trait]
pub trait ProcessManager: Send + Sync {
    /// Spawn multiple producer processes with communication channels
    /// 
    /// # Parameters
    /// - `count`: Number of producers to spawn
    /// - `topic`: Topic name for the producers
    /// - `api_keys`: Vector of available API key-value pairs
    /// 
    /// # Returns
    /// Vector of ProducerHandle containing the process ID and inbound channel
    async fn spawn_producers_with_channels(
        &self,
        count: u32,
        topic: &str,
        api_keys: Vec<KeyValuePair>,
    ) -> OrchestratorResult<Vec<ProducerHandle>>;

    /// Spawn webserver process with communication channel
    /// 
    /// # Parameters
    /// - `port`: Port number for the webserver
    /// 
    /// # Returns
    /// WebServerHandle containing the outbound channel for metrics
    async fn spawn_webserver_with_channel(&self, port: u16) -> OrchestratorResult<WebServerHandle>;

    /// Monitor all managed processes and return their health status
    /// 
    /// # Returns
    /// Vector of ProcessHealth reports for all active processes
    async fn monitor_processes(&self) -> OrchestratorResult<Vec<ProcessHealth>>;

    /// Restart a failed producer process
    /// 
    /// # Parameters
    /// - `producer_id`: ID of the producer to restart
    /// - `topic`: Topic name for the restarted producer
    /// - `openai_key`: Optional OpenAI API key
    /// - `anthropic_key`: Optional Anthropic API key
    /// 
    /// # Returns
    /// New ProducerHandle for the restarted process
    async fn restart_failed_producer(
        &self,
        producer_id: ProducerId,
        topic: &str,
        api_keys: Vec<KeyValuePair>,
    ) -> OrchestratorResult<ProducerHandle>;

    /// Restart a failed webserver process
    /// 
    /// # Parameters
    /// - `port`: Port number for the restarted webserver
    /// 
    /// # Returns
    /// New WebServerHandle for the restarted webserver
    async fn restart_failed_webserver(&self, port: u16) -> OrchestratorResult<WebServerHandle>;

    /// Stop all producer processes
    async fn stop_producers(&self) -> OrchestratorResult<()>;

    /// Stop the webserver process
    async fn stop_webserver(&self) -> OrchestratorResult<()>;
}

/// IPC communication abstraction for orchestrator coordination
/// 
/// This trait manages the communication channels between the orchestrator and
/// producer/webserver processes, handling message routing and coordination.
#[mockall::automock]
#[async_trait::async_trait]
pub trait IpcCommunicator: Send + Sync {
    /// Establish communication channels for producer processes
    /// 
    /// # Parameters
    /// - `producer_handles`: Vector of producer handles with communication channels
    async fn establish_producer_channels(
        &self,
        producer_handles: Vec<ProducerHandle>,
    ) -> OrchestratorResult<()>;

    /// Establish communication channel for webserver process
    /// 
    /// # Parameters
    /// - `webserver_handle`: Webserver handle with communication channel
    async fn establish_webserver_channel(
        &self,
        webserver_handle: WebServerHandle,
    ) -> OrchestratorResult<()>;

    /// Reestablish communication channel for a restarted producer
    /// 
    /// # Parameters
    /// - `new_handle`: New producer handle after restart
    async fn reestablish_producer_channel(
        &self,
        new_handle: ProducerHandle,
    ) -> OrchestratorResult<()>;

    /// Process incoming messages from all producers
    /// 
    /// # Returns
    /// Vector of tuples containing producer ID and their message batch
    async fn process_messages(&self) -> OrchestratorResult<Vec<(ProducerId, Vec<String>)>>;

    /// Send system metrics updates to webserver
    ///
    /// # Parameters
    /// - `metrics`: System metrics to send to webserver
    async fn send_updates(&self, metrics: SystemMetrics) -> OrchestratorResult<()>;
    
    /// Start listening for TaskRequest messages from webserver
    ///
    /// # Parameters
    /// - `listen_addr`: Address to bind TCP listener for incoming requests
    /// 
    /// # Returns
    /// Receiver channel for TaskRequest messages from webserver
    async fn start_task_request_listener(&self, listen_addr: std::net::SocketAddr) -> OrchestratorResult<tokio::sync::mpsc::Receiver<shared::TaskRequest>>;
    
    /// Send TaskUpdate message to webserver
    ///
    /// # Parameters
    /// - `update`: TaskUpdate message to send to webserver
    async fn send_task_update(&self, update: shared::TaskUpdate) -> OrchestratorResult<()>;

    /// Send ProducerCommand message to producer
    ///
    /// # Parameters
    /// - `producer_id`: Target producer ID
    /// - `command`: ProducerCommand message to send to producer
    async fn send_producer_command(&self, producer_id: ProducerId, command: shared::ProducerCommand) -> OrchestratorResult<()>;

    /// Listen for ProducerUpdate messages from producers
    ///
    /// # Parameters
    /// - `listen_addr`: Address to bind TCP listener for incoming updates
    /// 
    /// # Returns
    /// Receiver channel for ProducerUpdate messages from producers
    async fn start_producer_update_listener(&self, listen_addr: std::net::SocketAddr) -> OrchestratorResult<tokio::sync::mpsc::Receiver<shared::ProducerUpdate>>;

    /// Shutdown all communication channels
    async fn shutdown_communication(&self) -> OrchestratorResult<()>;
}

/// File system abstraction for dependency injection
/// 
/// This trait provides file system operations for managing topic folders,
/// writing attributes, and synchronizing data to disk.
#[mockall::automock]
#[async_trait::async_trait]
pub trait FileSystem: Send + Sync {
    /// Create a topic folder with initialization files
    /// 
    /// # Parameters
    /// - `topic`: Topic name for the folder
    /// - `producer_count`: Number of producers for metadata
    async fn create_topic_folder(&self, topic: &str, producer_count: u32) -> OrchestratorResult<()>;

    /// Write attributes to the pending queue for batch processing
    /// 
    /// # Parameters
    /// - `attributes`: Array of attribute strings to write
    async fn write_attributes(&self, attributes: &[String]) -> OrchestratorResult<()>;

    /// Synchronize all pending writes to disk and update metadata files
    async fn sync_files(&self) -> OrchestratorResult<()>;
}

/// Optimizer abstraction for intelligent prompt generation and routing strategy tuning
/// 
/// This trait analyzes producer performance statistics and generates optimized
/// prompts and routing strategies to maximize unique attributes per minute.
#[mockall::automock]
#[async_trait::async_trait]
pub trait Optimizer: Send + Sync {
    /// Generate optimized prompt and routing strategy for a given topic
    /// 
    /// # Parameters
    /// - `topic`: Raw topic from web UI
    /// - `current_stats`: Current system performance statistics
    /// 
    /// # Returns
    /// Optimization plan with prompt, routing strategy, and confidence metrics
    async fn optimize_for_topic(&self, topic: &str, current_stats: &OptimizationStats) -> OrchestratorResult<OptimizationPlan>;
    
    /// Analyze current performance and suggest optimizations
    /// 
    /// # Parameters
    /// - `current_stats`: Performance statistics from all producers
    /// 
    /// # Returns
    /// Optional optimization plan if improvements are recommended
    async fn analyze_and_suggest(&self, current_stats: &OptimizationStats) -> OrchestratorResult<Option<OptimizationPlan>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Test that mock traits can be instantiated
    #[tokio::test]
    async fn test_mock_trait_instantiation() {
        let _mock_process_manager = MockProcessManager::new();
        let _mock_ipc_communicator = MockIpcCommunicator::new();
        let _mock_file_system = MockFileSystem::new();
        let _mock_api_key_source = MockApiKeySource::new();
        let _mock_optimizer = MockOptimizer::new();
        
        // If this compiles and runs, our mock generation is working
        assert!(true);
    }
}