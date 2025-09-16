//! WebServer ↔ Orchestrator communication messages

use crate::types::{GenerationConstraints, OptimizationMode, SystemMetrics};
use serde::{Deserialize, Serialize};

/// Legacy alias for compatibility
pub type TaskRequest = WebServerRequest;
/// Legacy alias for compatibility  
pub type TaskUpdate = OrchestratorUpdate;

/// Messages sent from WebServer to Orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebServerRequest {
    /// Start generation for a topic
    StartGeneration {
        request_id: u64,
        topic: String,
        producer_count: u32,
        optimization_mode: OptimizationMode,
        constraints: GenerationConstraints,
        iterations: Option<u32>,
    },

    /// Stop current generation
    StopGeneration { request_id: u64 },

    /// Request current system status
    GetStatus { request_id: u64 },

    /// Update system configuration
    UpdateConfig {
        request_id: u64,
        optimization_mode: Option<OptimizationMode>,
        constraints: Option<GenerationConstraints>,
    },

    /// WebServer ready signal - sent when IPC listener is initialized
    Ready { listen_port: u16, http_port: u16 },
}

/// Messages sent from Orchestrator to WebServer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorUpdate {
    /// Acknowledgment of request
    RequestAck {
        request_id: u64,
        success: bool,
        message: Option<String>,
    },

    /// New attributes notification
    NewAttributes {
        attributes: Vec<String>,
        provider_metadata: Option<crate::types::ProviderMetadata>,
    },

    /// Error notification
    ErrorNotification(String),

    /// Real-time statistics update
    StatisticsUpdate {
        timestamp: u64,
        active_producers: u32,
        current_topic: Option<String>,
        total_unique_attributes: usize,
        metrics: SystemMetrics,
    },

    /// Generation completed notification
    GenerationComplete {
        timestamp: u64,
        topic: String,
        total_iterations: u32,
        final_unique_count: usize,
        completion_reason: CompletionReason,
    },
}

/// Reason for generation completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionReason {
    /// Reached iteration limit
    IterationLimitReached,
    /// Manual stop requested
    ManualStop,
    /// All producers failed
    AllProducersFailed,
    /// System error
    SystemError { error: String },
}

/// Overall system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemStatus {
    Initializing,
    Ready,
    Generating { topic: String, active_producers: u32 },
    Stopping,
    Stopped,
    Error { code: String },
}
