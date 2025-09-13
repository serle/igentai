//! Producer ↔ Orchestrator communication messages
//!
//! Messages for communication between orchestrator and producer processes.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::GenerationConfig;
use super::metrics::{GenerationMetadata, BloomFilterStats};

/// Messages sent between orchestrator and producer processes
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProducerMessage {
    // Orchestrator → Producer
    StartTopic {
        topic: String,
        prompt: String,
        provider_weights: HashMap<String, f32>,
        generation_config: GenerationConfig,
    },
    UpdateConfiguration {
        prompt: Option<String>,
        provider_weights: Option<HashMap<String, f32>>,
        bloom_filter: Option<Vec<u8>>, // For Phase 2
    },
    Stop,
    
    // Producer → Orchestrator  
    AttributeBatch {
        producer_id: String,
        attributes: Vec<String>,
        provider_used: String,
        generation_metadata: GenerationMetadata,
        bloom_stats: Option<BloomFilterStats>, // For Phase 2
    },
    ProducerStatus {
        producer_id: String,
        status: crate::ProducerStatus,
        provider_performance: HashMap<String, LLMPerformance>,
    },
}

use super::metrics::LLMPerformance;