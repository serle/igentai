//! Producer trait definitions for dependency injection

use std::collections::HashMap;
use std::net::SocketAddr;
use async_trait::async_trait;

use shared::{ProducerCommand, ProducerUpdate, ApiFailure, ProviderId, RoutingStrategy};
use crate::error::ProducerResult;
use crate::types::{ProviderResponse, ProviderStats};

/// Provider router trait with API key management and provider requests
#[mockall::automock]
#[async_trait]
pub trait ProviderRouter: Send + Sync {
    /// Set API keys for providers
    async fn set_api_keys(&self, api_keys: HashMap<ProviderId, String>) -> ProducerResult<()>;
    
    /// Set routing strategy (includes provider ordering/weights)
    async fn set_routing_strategy(&self, strategy: RoutingStrategy) -> ProducerResult<()>;
    
    /// Set prompt for requests
    async fn set_prompt(&self, prompt: String) -> ProducerResult<()>;
    
    /// Set generation configuration parameters
    async fn set_generation_config(&self, config: shared::GenerationConfig) -> ProducerResult<()>;
    
    /// Get request config for a provider
    async fn get_request_config(&self, provider: &ProviderId) -> shared::GenerationConfig;
    
    /// Select the next provider based on routing strategy
    async fn select_provider(&self) -> ProducerResult<ProviderId>;
    
    /// Get current provider health status
    async fn get_provider_stats(&self) -> ProducerResult<HashMap<ProviderId, ProviderStats>>;
    
    /// Make request to selected provider using stored prompt and topic
    async fn make_provider_request(&self, provider: &ProviderId, model: &str, topic: &str) -> Result<ProviderResponse, ApiFailure>;
}

/// IPC communication trait for orchestrator communication
#[mockall::automock]
#[async_trait]
pub trait IpcCommunicator: Send + Sync {
    /// Connect to orchestrator
    async fn connect(&self, orchestrator_addr: SocketAddr) -> ProducerResult<()>;
    
    /// Listen for commands from orchestrator
    async fn listen_for_commands(&self) -> ProducerResult<tokio::sync::mpsc::Receiver<ProducerCommand>>;
    
    /// Send update to orchestrator
    async fn send_update(&self, update: ProducerUpdate) -> ProducerResult<()>;
    
    /// Disconnect from orchestrator
    async fn disconnect(&self) -> ProducerResult<()>;
}

/// Response processing trait with integrated bloom filter management
#[mockall::automock]
#[async_trait]
pub trait ResponseProcessor: Send + Sync {
    /// Set bloom filter data from orchestrator
    async fn set_bloom_filter(&self, filter_data: Vec<u8>) -> ProducerResult<()>;
    
    /// Process raw response into clean, deduplicated attributes
    async fn process_response(&self, response: &str) -> ProducerResult<Vec<String>>;
    
    /// Extract attributes from raw response text
    async fn extract_attributes(&self, response: &str) -> ProducerResult<Vec<String>>;
    
    /// Filter attributes for duplicates using internal bloom filter
    async fn filter_duplicates(&self, attributes: &[String]) -> ProducerResult<Vec<String>>;
}

/// Performance tracking trait
#[mockall::automock]
#[async_trait]
pub trait PerformanceTracker: Send + Sync {
    /// Record successful request
    async fn record_success(&self, provider: &str, response_time: std::time::Duration, tokens: u32) -> ProducerResult<()>;
    
    /// Record failed request
    async fn record_failure(&self, provider: &str, failure: ApiFailure) -> ProducerResult<()>;
    
    /// Get provider statistics
    async fn get_stats(&self) -> ProducerResult<HashMap<String, ProviderStats>>;
}

