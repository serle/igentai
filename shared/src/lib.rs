//! Shared types and messages for the orchestrator system
//!
//! This crate provides the core types and message definitions used for
//! communication between the orchestrator, producers, and webserver.

pub mod logging;
pub mod messages;
pub mod types;

// Re-export commonly used types
pub use types::{
    ApiFailure, GenerationConfig, GenerationConstraints, OptimizationMode, ProcessId, ProcessStatus, ProducerMetrics,
    ProviderId, ProviderMetadata, ProviderMetrics, ProviderRequestMetadata, ProviderStatus, RequestConfig,
    RoutingStrategy, SharedError, SystemMetrics, TokenUsage,
};

// Re-export message types
pub use messages::{
    producer::{OrchestratorCommand, ProducerCommand, ProducerResponse, ProducerUpdate},
    webserver::{OrchestratorUpdate, TaskRequest, TaskUpdate, WebServerRequest},
};
