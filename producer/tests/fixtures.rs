//! Test fixtures for producer integration tests

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use shared::{ProducerCommand, ProducerUpdate, GenerationConfig, RoutingStrategy, ProviderId, messages::metrics::BloomFilterStats};
use producer::types::ProviderStats;

/// Create a test producer command for starting generation
pub fn create_test_start_command() -> ProducerCommand {
    ProducerCommand::Start {
        topic: "integration-test-topic".to_string(),
        prompt: "Generate creative attributes for testing".to_string(),
        routing_strategy: RoutingStrategy::RoundRobin {
            providers: vec![ProviderId::OpenAI, ProviderId::Anthropic]
        },
        generation_config: GenerationConfig {
            model: "gpt-4".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 1000,
            temperature: 0.7,
        },
    }
}

/// Create a test producer command for stopping generation
pub fn create_test_stop_command() -> ProducerCommand {
    ProducerCommand::Stop
}

/// Create a test producer command for updating routing strategy
pub fn create_test_routing_update() -> ProducerCommand {
    ProducerCommand::UpdateRoutingStrategy {
        routing_strategy: RoutingStrategy::PriorityOrder {
            providers: vec![ProviderId::Anthropic, ProviderId::OpenAI, ProviderId::Gemini]
        }
    }
}

/// Create a test producer command for updating generation config
pub fn create_test_config_update() -> ProducerCommand {
    ProducerCommand::UpdateGenerationConfig {
        generation_config: GenerationConfig {
            model: "claude-3".to_string(),
            batch_size: 2,
            context_window: 8192,
            max_tokens: 2000,
            temperature: 0.5,
        }
    }
}

/// Create a test producer command for updating bloom filter
pub fn create_test_bloom_update() -> ProducerCommand {
    ProducerCommand::UpdateBloomFilter {
        bloom_filter: vec![1, 2, 3, 4, 5, 6, 7, 8] // Test filter data
    }
}

/// Create a test producer update with response data
pub fn create_test_response_update() -> ProducerUpdate {
    ProducerUpdate::DataUpdate {
        producer_id: "integration-test-producer".to_string(),
        attributes: vec![
            "creative".to_string(),
            "innovative".to_string(),
            "unique".to_string()
        ],
        provider_used: ProviderId::OpenAI,
        provider_metadata: shared::ProviderRequestMetadata {
            response_time_ms: 1200,
            tokens_used: 150,
            prompt_tokens: 50,
            completion_tokens: 100,
            provider_status: shared::ProviderStatus::Healthy,
            success: true,
        },
        bloom_stats: Some(BloomFilterStats {
            total_candidates: 10,
            filtered_candidates: 7,
            filter_effectiveness: 0.7,
            last_filter_update: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }),
    }
}

/// Create a test producer update with statistics
pub fn create_test_statistics_update() -> ProducerUpdate {
    let mut provider_performance = HashMap::new();
    provider_performance.insert(ProviderId::OpenAI, shared::LLMPerformance {
        unique_generated: 95,
        success_rate: 0.95,
        average_response_time_ms: 1200,
        uniqueness_ratio: 0.85,
        efficiency_score: 0.9,
        consecutive_failures: 0,
        last_success_timestamp: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
        current_status: shared::ProviderStatus::Healthy,
    });

    ProducerUpdate::StatisticsUpdate {
        producer_id: "integration-test-producer".to_string(),
        status: shared::ProducerStatus::Running,
        provider_performance,
    }
}

/// Create test bloom filter data
pub fn create_test_bloom_filter_data() -> Vec<u8> {
    // Create realistic test bloom filter data
    let mut data = Vec::new();
    for i in 0..64 {
        data.push((i % 256) as u8);
    }
    data
}

/// Create test LLM response for processing
pub fn create_test_llm_response() -> String {
    r#"Here are some creative attributes: "innovative", "dynamic", "sustainable", modern, efficient, "user-friendly""#.to_string()
}

/// Create weighted routing strategy for testing
pub fn create_test_weighted_strategy() -> RoutingStrategy {
    let mut weights = HashMap::new();
    weights.insert(ProviderId::OpenAI, 0.5);
    weights.insert(ProviderId::Anthropic, 0.3);
    weights.insert(ProviderId::Gemini, 0.2);
    
    RoutingStrategy::Weighted { weights }
}