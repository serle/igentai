//! Shared types and messages for the orchestrator system
//!
//! This crate provides the core types and message definitions used for
//! communication between the orchestrator, producers, and webserver.

pub mod messages;
pub mod types;
pub mod logging;

// Re-export commonly used types
pub use types::{
    ProviderId, ProcessId, ProviderMetadata, TokenUsage, SystemMetrics, ProcessStatus,
    OptimizationMode, GenerationConstraints, RoutingStrategy, GenerationConfig,
    ProducerMetrics, ProviderMetrics, ProviderStatus, RequestConfig, ApiFailure,
    ProviderRequestMetadata, SharedError,
};

// Re-export message types
pub use messages::{
    webserver::{WebServerRequest, OrchestratorUpdate, TaskRequest, TaskUpdate},
    producer::{OrchestratorCommand, ProducerUpdate, ProducerCommand, ProducerResponse},
};
