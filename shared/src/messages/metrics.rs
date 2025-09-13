//! System metrics and performance monitoring types
//!
//! Types for tracking system performance, LLM provider metrics, and attribute updates.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// System metrics shared across all components
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemMetrics {
    pub total_unique_entries: u64,
    pub entries_per_minute: f64,
    pub per_llm_performance: HashMap<String, LLMPerformance>,
    pub current_topic: Option<String>,
    pub active_producers: u32,
    pub uptime_seconds: u64,
    pub last_updated: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            total_unique_entries: 0,
            entries_per_minute: 0.0,
            per_llm_performance: HashMap::new(),
            current_topic: None,
            active_producers: 0,
            uptime_seconds: 0,
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }
    }
}

/// LLM provider performance metrics (includes health status)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LLMPerformance {
    pub unique_generated: u64,
    pub success_rate: f32,
    pub average_response_time_ms: u64,
    pub uniqueness_ratio: f32,
    pub efficiency_score: f32,
    pub consecutive_failures: u32,
    pub last_success_timestamp: Option<u64>,
    pub current_status: crate::ProviderStatus,
}

/// Attribute update notification
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AttributeUpdate {
    pub content: String,
    pub timestamp: u64,
    pub producer_id: String,
    pub llm_provider: String,
    pub is_unique: bool,
}

/// System health status
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SystemHealth {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Provider request metadata from LLM API calls
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderRequestMetadata {
    pub response_time_ms: u64,
    pub tokens_used: u32,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub provider_status: crate::ProviderStatus,
    pub success: bool,
}

/// Bloom filter statistics (Phase 2)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BloomFilterStats {
    pub total_candidates: u64,
    pub filtered_candidates: u64,
    pub filter_effectiveness: f64,
    pub last_filter_update: u64,
}