//! Producer service trait definitions for dependency injection

use crate::error::ProducerResult;
use crate::types::{ApiRequest, ApiResponse};
use async_trait::async_trait;
use shared::{ProducerCommand, ProducerUpdate};
use tokio::sync::mpsc;

/// Trait for communicating with orchestrator via IPC
#[async_trait]
pub trait Communicator: Send + Sync {
    /// Initialize connection to orchestrator
    async fn initialize(&mut self) -> ProducerResult<()>;

    /// Get command receiver from orchestrator
    async fn get_commands(&mut self) -> ProducerResult<mpsc::Receiver<ProducerCommand>>;

    /// Send update to orchestrator
    async fn send_update(&self, update: ProducerUpdate) -> ProducerResult<()>;

    /// Check connection health
    async fn health_check(&self) -> ProducerResult<bool>;

    /// Get the listen port (if any)
    fn get_listen_port(&self) -> Option<u16>;

    /// Disconnect from orchestrator
    async fn disconnect(&self) -> ProducerResult<()>;
}

/// Trait for making API calls to external providers
#[async_trait]
pub trait ApiClient: Send + Sync {
    /// Send request to provider API
    async fn send_request(&self, request: ApiRequest) -> ProducerResult<ApiResponse>;

    /// Check if provider is available
    async fn health_check(&self, provider: shared::ProviderId) -> ProducerResult<bool>;

    /// Get estimated cost for request
    fn estimate_cost(&self, provider: shared::ProviderId, tokens: &shared::TokenUsage) -> f64;
}
