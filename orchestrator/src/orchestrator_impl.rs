//! Orchestrator implementation with enhanced IPC coordination and process monitoring

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use shared::{ProducerId, SystemMetrics, ProcessStatus};

use crate::error::{OrchestratorResult, OrchestratorError};
use crate::state::OrchestratorState;
use crate::traits::{ApiKeySource, ProcessManager, IpcCommunicator, FileSystem, Optimizer};

/// Enhanced orchestrator with IPC coordination and monitoring
pub struct Orchestrator<A, F, P, M, O>
where
    A: ApiKeySource,
    F: FileSystem,
    P: ProcessManager,
    M: IpcCommunicator,
    O: Optimizer,
{
    // Configuration
    producer_count: u32,
    webserver_port: u16,
    
    // Core state (testable independently)
    state: OrchestratorState,
    
    // Injected dependencies (mockable for testing)
    api_keys: A,
    file_system: F,
    process_manager: P,
    ipc_communicator: M,
    optimizer: O,
    
    // Process monitoring
    monitoring_active: Arc<AtomicBool>,
    monitoring_handle: Option<tokio::task::JoinHandle<()>>,
    current_topic: Option<String>,
    
    // Control state
    file_sync_active: Arc<AtomicBool>,
    is_running: Arc<AtomicBool>,
    start_time: Instant,
}

impl<A, F, P, M, O> Orchestrator<A, F, P, M, O>
where
    A: ApiKeySource + Send + Sync + 'static,
    F: FileSystem + Send + Sync + 'static,
    P: ProcessManager + Send + Sync + 'static,
    M: IpcCommunicator + Send + Sync + 'static,
    O: Optimizer + Send + Sync + 'static,
{
    /// Create orchestrator with all injected dependencies
    pub fn new(producer_count: u32, webserver_port: u16, api_keys: A, file_system: F, process_manager: P, ipc_communicator: M, optimizer: O) -> Self {
        Self {
            producer_count,
            webserver_port,
            state: OrchestratorState::new(),
            api_keys,
            file_system,
            process_manager,
            ipc_communicator,
            optimizer,
            monitoring_active: Arc::new(AtomicBool::new(false)),
            monitoring_handle: None,
            current_topic: None,
            file_sync_active: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(true)),
            start_time: Instant::now(),
        }
    }

    /// Main orchestrator control loop
    pub async fn run(&mut self) -> OrchestratorResult<()> {
        // Validate API keys on startup
        let _api_keys = self.api_keys.get_api_keys().await
            .map_err(|missing| OrchestratorError::ConfigurationError { 
                field: missing.message 
            })?;
        
        println!("Orchestrator running with API key validation complete");
        
        while self.is_running.load(Ordering::Relaxed) {
            self.next().await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Ok(())
    }

    /// Single orchestrator cycle iteration
    pub async fn next(&mut self) -> OrchestratorResult<()> {
        // Process incoming messages from producers
        let producer_batches = self.ipc_communicator.process_messages().await?;
        
        // Handle uniqueness checking for each batch
        for (producer_id, candidates) in producer_batches {
            self.process_result(producer_id, candidates).await?;
        }
        
        // Send metrics updates to web server
        let metrics = self.get_metrics();
        self.ipc_communicator.send_updates(metrics).await?;
        
        Ok(())
    }

    /// Process attribute batch from producer for uniqueness checking
    pub async fn process_result(&mut self, producer_id: ProducerId, candidates: Vec<String>) -> OrchestratorResult<Vec<String>> {
        let unique_items = self.state.process_result(producer_id, candidates)?;
        
        if !unique_items.is_empty() {
            // Write unique attributes through file system
            self.file_system.write_attributes(&unique_items).await?;
        }
        
        Ok(unique_items)
    }

    /// Initialize producer configurations for topic
    pub async fn init_producer_config(&mut self, topic: &str) -> OrchestratorResult<()> {
        // Initialize state management
        self.state.init_producer_config(topic, self.producer_count)?;
        
        // Create topic folder through file system
        self.file_system.create_topic_folder(topic, self.producer_count).await?;
        
        Ok(())
    }

    /// Start TCP listener for webserver TaskRequest messages
    pub async fn start_webserver_listener(&self) -> OrchestratorResult<tokio::sync::mpsc::Receiver<shared::TaskRequest>> {
        // Start listening for TaskRequest messages from webserver
        let listen_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8080)); // TODO: Make configurable
        self.ipc_communicator.start_task_request_listener(listen_addr).await
    }
    
    /// Handle incoming TaskRequest from webserver
    pub async fn handle_task_request(&mut self, request: shared::TaskRequest) -> OrchestratorResult<()> {
        println!("📨 Handling TaskRequest: {:?}", request);
        
        match request {
            shared::TaskRequest::TopicRequest { topic, producer_count, prompt: _ } => {
                println!("🚀 Starting topic generation: '{}' with {} producers", topic, producer_count);
                
                // If already running, stop current session
                if self.current_topic.is_some() {
                    self.stop_producers().await?;
                }
                
                // Update producer count if different
                self.producer_count = producer_count;
                
                // Start new topic generation
                self.start_producers(topic).await?;
                
                // Send status update back to webserver
                let current_metrics = self.calculate_system_metrics().await;
                self.ipc_communicator.send_updates(current_metrics).await?;
            }
            shared::TaskRequest::StopGeneration => {
                println!("⏹️ Stopping topic generation");
                self.stop_producers().await?;
                
                // Send status update back to webserver
                let current_metrics = self.calculate_system_metrics().await;
                self.ipc_communicator.send_updates(current_metrics).await?;
            }
            shared::TaskRequest::RequestStatus => {
                println!("📊 Sending status update to webserver");
                
                // Send current status back to webserver
                let current_metrics = self.calculate_system_metrics().await;
                self.ipc_communicator.send_updates(current_metrics).await?;
            }
        }
        
        Ok(())
    }
    
    /// Calculate current system metrics for webserver updates
    async fn calculate_system_metrics(&self) -> shared::SystemMetrics {
        // TODO: Implement proper metrics calculation based on current state
        // For now, return basic metrics
        shared::SystemMetrics {
            total_unique_entries: self.state.total_count(),
            entries_per_minute: 0.0, // TODO: Calculate from recent activity
            per_llm_performance: std::collections::HashMap::new(), // TODO: Add LLM performance data
            current_topic: self.current_topic.clone(),
            active_producers: if self.current_topic.is_some() { self.producer_count } else { 0 },
            uptime_seconds: 0, // TODO: Calculate uptime
            last_updated: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        }
    }
    
    /// Enhanced start_producers with IPC coordination 
    pub async fn start_producers(&mut self, topic: String) -> OrchestratorResult<()> {
        // Initialize configuration first
        self.init_producer_config(&topic).await?;
        
        // Get API keys for producer spawning
        let api_keys = self.api_keys.get_api_keys().await
            .map_err(|missing| OrchestratorError::ConfigurationError { 
                field: missing.message 
            })?;
        
        // Spawn producers AND get coordination info
        let producer_handles = self.process_manager
            .spawn_producers_with_channels(self.producer_count, &topic, api_keys)
            .await?;
        
        // Establish communication channels using coordination info
        self.ipc_communicator
            .establish_producer_channels(producer_handles)
            .await?;
        
        // Spawn and connect web server
        let webserver_handle = self.process_manager
            .spawn_webserver_with_channel(self.webserver_port)
            .await?;
        
        self.ipc_communicator
            .establish_webserver_channel(webserver_handle)
            .await?;
        
        // Process monitoring will be handled through regular method calls
        
        self.current_topic = Some(topic);
        println!("All processes started with IPC coordination complete");
        Ok(())
    }

    /// Stop all processes
    pub async fn stop_producers(&mut self) -> OrchestratorResult<()> {
        // Stop process monitoring
        self.stop_process_monitoring().await?;
        
        // Stop all processes
        self.process_manager.stop_producers().await?;
        self.process_manager.stop_webserver().await?;
        
        // Shutdown communication channels
        self.ipc_communicator.shutdown_communication().await?;
        
        self.current_topic = None;
        println!("All processes stopped and communication channels closed");
        Ok(())
    }

    /// Synchronize files to disk
    pub async fn sync_files(&mut self) -> OrchestratorResult<()> {
        self.file_system.sync_files().await
    }

    /// Stop file synchronization
    pub async fn stop_file_sync(&mut self) -> OrchestratorResult<()> {
        self.file_sync_active.store(false, Ordering::Relaxed);
        self.file_system.sync_files().await?;
        Ok(())
    }

    /// Check process health and restart failed producers if needed
    pub async fn check_and_restart_failed_processes(&mut self) -> OrchestratorResult<()> {
        if let Some(topic) = &self.current_topic {
            let health_reports = self.process_manager.monitor_processes().await?;
            
            for health in health_reports {
                if health.status == ProcessStatus::Failed {
                    println!("Detected failed producer: {}, attempting restart", health.process_id);
                    
                    // Attempt restart with API key injection
                    let producer_id = match ProducerId::from_string(&health.process_id) {
                        Ok(id) => id,
                        Err(e) => {
                            println!("Invalid producer ID {}: {}", health.process_id, e);
                            continue;
                        }
                    };
                    
                    let keys = self.api_keys.get_api_keys().await
                        .map_err(|e| OrchestratorError::ConfigurationError { 
                            field: e.message 
                        })?;
                    
                    match self.process_manager
                        .restart_failed_producer(producer_id, topic, keys)
                        .await
                    {
                        Ok(new_handle) => {
                            // Reestablish communication channel
                            if let Err(e) = self.ipc_communicator
                                .reestablish_producer_channel(new_handle)
                                .await
                            {
                                println!("Failed to reestablish channel for {}: {}", health.process_id, e);
                            } else {
                                println!("Successfully restarted and reconnected producer {}", health.process_id);
                            }
                        }
                        Err(e) => {
                            println!("Failed to restart producer {}: {}", health.process_id, e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Stop process monitoring
    async fn stop_process_monitoring(&mut self) -> OrchestratorResult<()> {
        self.monitoring_active.store(false, Ordering::Relaxed);
        
        if let Some(handle) = self.monitoring_handle.take() {
            let _ = handle.await;
        }
        
        Ok(())
    }

    /// Get current system metrics
    pub fn get_metrics(&self) -> SystemMetrics {
        self.state.get_metrics(self.start_time)
    }
    
    // Accessors for testing and configuration
    pub fn total_count(&self) -> u64 {
        self.state.total_count()
    }
    
    pub fn current_topic(&self) -> Option<&str> {
        self.state.current_topic()
    }
    
    /// Get configured producer count
    pub fn producer_count(&self) -> u32 {
        self.producer_count
    }
    
    /// Get configured webserver port
    pub fn port(&self) -> u16 {
        self.webserver_port
    }
    
    /// Reset state for a new topic (async for potential cleanup operations)
    pub async fn reset_for_topic(&mut self, topic: String) {
        self.state.reset_for_topic(topic.clone());
        self.current_topic = Some(topic);
    }
}