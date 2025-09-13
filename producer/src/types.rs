//! Producer-specific data types

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use serde::{Serialize, Deserialize};
use shared::{ProducerId, ProviderStatus, ApiFailure};

/// Producer state management (simplified - most config moved to services)
#[derive(Debug)]
pub struct ProducerState {
    // Identity and configuration
    pub id: ProducerId,
    pub orchestrator_addr: std::net::SocketAddr,
    
    // Current task context
    pub topic: String,
    
    // Control flags
    pub is_running: Arc<AtomicBool>,
    pub should_stop: Arc<AtomicBool>,
}

/// Provider health tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub consecutive_failures: u32,
    pub last_success: Option<u64>, // timestamp
    pub last_failure: Option<u64>, // timestamp
    pub average_response_time_ms: u64,
    pub current_status: ProviderStatus,
}

/// Provider performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_response_time_ms: u64,
    pub unique_contributions: u64,
    pub last_used: Option<u64>, // timestamp
}

/// A single provider request record
#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub timestamp: Instant,
    pub provider_used: String,
    pub model_used: String,
    pub prompt_sent: String,
    pub response_time: Duration,
    pub success: bool,
    pub failure_reason: Option<ApiFailure>,
}

/// Provider response data
#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub content: String,
    pub tokens_used: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model_used: String,
    pub response_time: Duration,
}

impl ProducerState {
    /// Create new producer state
    pub fn new(id: ProducerId, orchestrator_addr: std::net::SocketAddr) -> Self {
        Self {
            id,
            orchestrator_addr,
            topic: String::new(),
            is_running: Arc::new(AtomicBool::new(false)),
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            last_success: None,
            last_failure: None,
            average_response_time_ms: 0,
            current_status: ProviderStatus::Unknown,
        }
    }
}

impl Default for ProviderStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_response_time_ms: 0,
            unique_contributions: 0,
            last_used: None,
        }
    }
}