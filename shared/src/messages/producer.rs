//! Orchestrator ↔ Producer communication messages

use serde::{Serialize, Deserialize};
use crate::types::{
    ProcessId, ProviderMetadata, ProcessStatus, RoutingStrategy, GenerationConfig
};

/// Commands sent from Orchestrator to Producer
pub type ProducerCommand = OrchestratorCommand;

/// Commands sent from Orchestrator to Producer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorCommand {
    /// Start generation with configuration
    Start {
        command_id: u64,
        topic: String,
        prompt: String,
        routing_strategy: RoutingStrategy,
        generation_config: GenerationConfig,
    },
    
    /// Update configuration during operation
    UpdateConfig {
        command_id: u64,
        routing_strategy: Option<RoutingStrategy>,
        generation_config: Option<GenerationConfig>,
        prompt: Option<String>,
    },
    
    /// Sync check with optional bloom filter (health check + dedup sync)
    SyncCheck {
        sync_id: u64,
        timestamp: u64,
        bloom_filter: Option<Vec<u8>>,
        bloom_version: Option<u64>,
        requires_dedup: bool,
        seen_values: Option<Vec<String>>,
    },
    
    /// Stop generation
    Stop {
        command_id: u64,
    },
    
    /// Ping for health check
    Ping {
        ping_id: u64,
    },
    
}

/// Response sent from Producer to Orchestrator
pub type ProducerResponse = ProducerUpdate;

/// Updates sent from Producer to Orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProducerUpdate {
    /// New batch of generated attributes
    AttributeBatch {
        producer_id: ProcessId,
        batch_id: u64,
        attributes: Vec<String>,
        provider_metadata: ProviderMetadata,
    },
    
    /// Sync acknowledgment (responds to SyncCheck)
    SyncAck {
        producer_id: ProcessId,
        sync_id: u64,
        bloom_version: Option<u64>,
        status: ProducerSyncStatus,
    },
    
    /// Producer status update
    StatusUpdate {
        producer_id: ProcessId,
        status: ProcessStatus,
        message: Option<String>,
        performance_stats: Option<ProducerPerformanceStats>,
    },
    
    /// Response to ping
    Pong {
        producer_id: ProcessId,
        ping_id: u64,
    },
    
    /// Error report
    Error {
        producer_id: ProcessId,
        error_code: String,
        message: String,
        command_id: Option<u64>,
    },
    
    /// Producer ready signal - sent when IPC listener is initialized
    Ready {
        producer_id: ProcessId,
        listen_port: u16,
    },
}

/// Status of producer regarding sync operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProducerSyncStatus {
    /// Sync successful
    Ready,
    
    /// Bloom filter applied successfully
    BloomUpdated { filter_size_bytes: usize },
    
    /// Sync failed
    Failed { reason: String },
    
    /// Producer is busy, try again later
    Busy,
}

/// Performance statistics reported by producer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerPerformanceStats {
    /// Recent activity metrics
    pub attributes_generated_last_minute: u64,
    pub unique_contributed_last_minute: u64,
    pub requests_made_last_minute: u64,
    
    /// Provider usage breakdown
    pub provider_usage: std::collections::HashMap<crate::types::ProviderId, ProviderUsageStats>,
    
    /// Current batch processing rate
    pub current_batch_rate: f64,
    
    /// Memory and resource usage
    pub memory_usage_mb: Option<u64>,
    pub bloom_filter_size_mb: Option<f64>,
}

/// Usage statistics for a specific provider by this producer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsageStats {
    pub requests_sent: u64,
    pub successful_responses: u64,
    pub unique_attributes_contributed: u64,
    pub avg_response_time_ms: f64,
    pub last_used_timestamp: u64,
}