//! Producer ↔ Orchestrator communication messages
//!
//! Separate message types for bidirectional communication between orchestrator and producer processes.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::{GenerationConfig, ProviderId, RoutingStrategy};
use super::metrics::{ProviderRequestMetadata, BloomFilterStats, LLMPerformance};

/// Commands sent from orchestrator to producer
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProducerCommand {
    /// Start generation for a topic
    Start {
        topic: String,
        prompt: String,
        routing_strategy: RoutingStrategy,
        generation_config: GenerationConfig,
    },
    /// Update the generation prompt
    UpdatePrompt {
        prompt: String,
    },
    /// Update routing strategy (includes provider ordering/weights)
    UpdateRoutingStrategy {
        routing_strategy: RoutingStrategy,
    },
    /// Update generation configuration
    UpdateGenerationConfig {
        generation_config: GenerationConfig,
    },
    /// Update bloom filter data (Phase 2)
    UpdateBloomFilter {
        bloom_filter: Vec<u8>,
    },
    /// Stop generation and shutdown
    Stop,
}

/// Updates sent from producer to orchestrator
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProducerUpdate {
    /// New attribute data generated
    DataUpdate {
        producer_id: String,
        attributes: Vec<String>,
        provider_used: ProviderId,
        provider_metadata: ProviderRequestMetadata,
        bloom_stats: Option<BloomFilterStats>, // For Phase 2
    },
    /// Performance and health statistics
    StatisticsUpdate {
        producer_id: String,
        status: crate::ProducerStatus,
        provider_performance: HashMap<ProviderId, LLMPerformance>,
    },
}

/// Legacy enum for backward compatibility - deprecated
#[deprecated(note = "Use ProducerCommand and ProducerUpdate instead")]
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
        bloom_filter: Option<Vec<u8>>,
    },
    Stop,
    
    // Producer → Orchestrator  
    AttributeBatch {
        producer_id: String,
        attributes: Vec<String>,
        provider_used: String,
        provider_metadata: ProviderRequestMetadata,
        bloom_stats: Option<BloomFilterStats>,
    },
    ProducerStatus {
        producer_id: String,
        status: crate::ProducerStatus,
        provider_performance: HashMap<String, LLMPerformance>,
    },
}