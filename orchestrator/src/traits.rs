//! Service traits for dependency injection
//!
//! This module defines all I/O service traits used by the orchestrator.
//! Each trait is mockable for comprehensive testing.

use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;

use crate::error::OrchestratorResult;
use shared::{OrchestratorCommand, OrchestratorUpdate, ProcessId, ProducerUpdate, ProviderId, WebServerRequest};

/// API key management service
#[mockall::automock]
#[async_trait]
pub trait ApiKeySource: Send + Sync {
    /// Get all available API keys
    async fn get_api_keys(&self) -> OrchestratorResult<HashMap<ProviderId, String>>;

    /// Check if a specific provider has an API key
    async fn has_api_key(&self, provider_id: ProviderId) -> bool;

    /// Get API key for a specific provider
    async fn get_provider_key(&self, provider_id: ProviderId) -> OrchestratorResult<Option<String>>;
}

/// Inter-process communication service
#[mockall::automock]
#[async_trait]
pub trait Communicator: Send + Sync {
    /// Start listening for WebServer requests
    async fn start_webserver_listener(
        &self,
        bind_addr: SocketAddr,
    ) -> OrchestratorResult<mpsc::Receiver<WebServerRequest>>;

    /// Send update to WebServer
    async fn send_webserver_update(&self, update: OrchestratorUpdate) -> OrchestratorResult<()>;

    /// Start listening for Producer updates
    async fn start_producer_listener(
        &self,
        bind_addr: SocketAddr,
    ) -> OrchestratorResult<mpsc::Receiver<ProducerUpdate>>;

    /// Send command to a specific Producer
    async fn send_producer_command(
        &self,
        producer_id: ProcessId,
        command: OrchestratorCommand,
    ) -> OrchestratorResult<()>;

    /// Register producer address for command sending
    async fn register_producer(&self, producer_id: ProcessId, address: SocketAddr) -> OrchestratorResult<()>;

    /// Mark producer as ready to receive commands
    async fn mark_producer_ready(&self, producer_id: ProcessId, address: SocketAddr) -> OrchestratorResult<()>;

    /// Register webserver address for update sending
    async fn register_webserver(&self, address: SocketAddr) -> OrchestratorResult<()>;

    /// Mark webserver as ready to receive updates
    async fn mark_webserver_ready(&self, address: SocketAddr) -> OrchestratorResult<()>;

    /// Close all communication channels
    async fn shutdown(&self) -> OrchestratorResult<()>;
}

/// File system operations service
#[mockall::automock]
#[async_trait]
pub trait FileSystem: Send + Sync {
    /// Create a topic directory structure
    async fn create_topic_directory(&self, topic: &str) -> OrchestratorResult<()>;

    /// Write unique attributes to storage
    async fn write_unique_attributes(&self, topic: &str, attributes: &[String]) -> OrchestratorResult<()>;

    /// Write unique attributes with provider metadata to storage
    async fn write_unique_attributes_with_metadata(
        &self,
        topic: &str,
        attributes: &[String],
        provider_metadata: &shared::types::ProviderMetadata,
    ) -> OrchestratorResult<()>;

    /// Read existing attributes for a topic
    async fn read_topic_attributes(&self, topic: &str) -> OrchestratorResult<Vec<String>>;

    /// Clean up old topic data
    async fn cleanup_topic(&self, topic: &str) -> OrchestratorResult<()>;

    /// Ensure all writes are synced to disk
    async fn sync_to_disk(&self) -> OrchestratorResult<()>;

    /// Append new unique attributes to output.txt file (one per line)
    async fn append_to_output(&self, topic: &str, new_attributes: &[String]) -> OrchestratorResult<()>;

    /// Write arbitrary file content (for JSON exports)
    async fn write_file(&self, filename: &str, content: &[u8]) -> OrchestratorResult<()>;
}

/// Process management service  
#[mockall::automock]
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Spawn producer processes
    async fn spawn_producers(
        &self,
        count: u32,
        topic: &str,
        api_keys: HashMap<ProviderId, String>,
        orchestrator_addr: SocketAddr,
        routing_strategy: Option<shared::RoutingStrategy>,
    ) -> OrchestratorResult<Vec<ProducerInfo>>;

    /// Spawn webserver process
    async fn spawn_webserver(&self, port: u16, orchestrator_addr: SocketAddr) -> OrchestratorResult<WebServerInfo>;

    /// Check health of all managed processes
    async fn check_process_health(&self) -> OrchestratorResult<Vec<ProcessHealthInfo>>;

    /// Stop a specific producer
    async fn stop_producer(&self, producer_id: ProcessId) -> OrchestratorResult<()>;

    /// Stop webserver
    async fn stop_webserver(&self) -> OrchestratorResult<()>;

    /// Stop all managed processes
    async fn stop_all(&self) -> OrchestratorResult<()>;
}

/// Information about a spawned producer
#[derive(Debug, Clone)]
pub struct ProducerInfo {
    pub id: ProcessId,
    pub process_id: u32,
    pub listen_address: SocketAddr,
    pub command_address: SocketAddr,
}

/// Information about spawned webserver
#[derive(Debug, Clone)]
pub struct WebServerInfo {
    pub process_id: u32,
    pub listen_address: SocketAddr,
    pub api_address: SocketAddr,
}

/// Health information for a process
#[derive(Debug, Clone)]
pub struct ProcessHealthInfo {
    pub process_id: u32,
    pub producer_id: Option<ProcessId>,
    pub status: ProcessStatus,
    pub last_heartbeat: Option<std::time::Instant>,
    pub memory_usage_mb: Option<u64>,
}

// Re-export ProcessStatus from shared to avoid duplication
pub use shared::ProcessStatus;
