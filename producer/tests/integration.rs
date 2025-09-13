//! Integration tests for producer
//!
//! These tests verify that the producer components work together correctly
//! using the real service implementations.

mod fixtures;
mod helpers;

use producer::services::{
    RealIpcCommunicator, RealProviderRouter, RealResponseProcessor, RealPerformanceTracker
};
use producer::{Producer, IpcCommunicator, ProviderRouter, ResponseProcessor, PerformanceTracker};
use shared::{ProducerCommand, ProducerUpdate, ProviderId, ProducerId};
use helpers::*;
use fixtures::*;
use std::time::Duration;

#[tokio::test]
async fn test_producer_creation_and_shutdown() {
    let orchestrator_addr = create_test_addresses();
    
    // Create real service implementations
    let ipc_communicator = RealIpcCommunicator::new();
    let provider_router = RealProviderRouter::new();
    let response_processor = RealResponseProcessor::new();
    let performance_tracker = RealPerformanceTracker::new();
    
    // Create producer with real implementations
    let producer = Producer::new(
        ProducerId::new(),
        orchestrator_addr,
        ipc_communicator,
        provider_router,
        response_processor,
        performance_tracker,
    );
    
    // Test that producer was created successfully by attempting shutdown
    let shutdown_result = producer.shutdown().await;
    assert!(shutdown_result.is_ok(), "Producer should shutdown cleanly");
}

#[tokio::test]
async fn test_producer_api_key_management() {
    let orchestrator_addr = create_test_addresses();
    
    // Create producer
    let ipc_communicator = RealIpcCommunicator::new();
    let provider_router = RealProviderRouter::new();
    let response_processor = RealResponseProcessor::new();
    let performance_tracker = RealPerformanceTracker::new();
    
    let producer = Producer::new(
        ProducerId::new(),
        orchestrator_addr,
        ipc_communicator,
        provider_router,
        response_processor,
        performance_tracker,
    );
    
    // Test that producer handles API key setting
    let mut api_keys = std::collections::HashMap::new();
    api_keys.insert("openai".to_string(), "test-key".to_string());
    
    let result = producer.set_api_keys(api_keys).await;
    assert!(result.is_ok(), "Should accept valid API keys");
    
    // Test empty API keys are rejected
    let empty_keys = std::collections::HashMap::new();
    let empty_result = producer.set_api_keys(empty_keys).await;
    assert!(empty_result.is_err(), "Should reject empty API keys");
}

#[tokio::test]
async fn test_ipc_communicator_connection_handling() {
    let orchestrator_addr = create_test_addresses();
    
    // Create IPC communicator
    let ipc_communicator = RealIpcCommunicator::new();
    
    // Test connection attempt (will fail but should handle gracefully)
    let connect_result = ipc_communicator.connect(orchestrator_addr).await;
    
    // Connection should fail since no orchestrator is running, but should return proper error
    assert!(connect_result.is_err(), "Should fail to connect when no orchestrator running");
    
    // Test that we can't send updates when not connected
    let update = create_test_response_update();
    let send_result = ipc_communicator.send_update(update).await;
    assert!(send_result.is_err(), "Should fail to send when not connected");
    
    // Test that we can't listen for commands when not connected
    let listen_result = ipc_communicator.listen_for_commands().await;
    assert!(listen_result.is_err(), "Should fail to listen when not connected");
    
    // Test successful disconnect (should always work)
    let disconnect_result = ipc_communicator.disconnect().await;
    assert!(disconnect_result.is_ok(), "Disconnect should always succeed");
}

#[tokio::test]
async fn test_provider_router_strategy_management() {
    let provider_router = RealProviderRouter::new();
    
    // Test round robin strategy
    let round_robin_strategy = create_test_routing_update();
    if let ProducerCommand::UpdateRoutingStrategy { routing_strategy } = round_robin_strategy {
        let set_result = provider_router.set_routing_strategy(routing_strategy).await;
        assert!(set_result.is_ok(), "Should successfully set routing strategy");
        
        // Test provider selection
        let provider1 = provider_router.select_provider().await.unwrap();
        let provider2 = provider_router.select_provider().await.unwrap();
        
        // Should cycle through providers (both should be valid ProviderId)
        assert!(matches!(provider1, ProviderId::OpenAI | ProviderId::Anthropic));
        assert!(matches!(provider2, ProviderId::OpenAI | ProviderId::Anthropic));
    }
    
    // Test weighted strategy
    let weighted_strategy = create_test_weighted_strategy();
    let weighted_result = provider_router.set_routing_strategy(weighted_strategy).await;
    assert!(weighted_result.is_ok(), "Should successfully set weighted strategy");
    
    // Test multiple selections (all should succeed)
    for _ in 0..5 {
        let provider = provider_router.select_provider().await;
        assert!(provider.is_ok(), "Provider selection should succeed with weighted strategy");
        let provider_id = provider.unwrap();
        assert!(matches!(provider_id, ProviderId::OpenAI | ProviderId::Anthropic | ProviderId::Gemini));
    }
}

#[tokio::test]
async fn test_response_processor_attribute_extraction() {
    let response_processor = RealResponseProcessor::new();
    
    // Test processing without bloom filter
    let test_response = create_test_llm_response();
    let attributes = response_processor.process_response(&test_response).await;
    
    assert!(attributes.is_ok(), "Should successfully process response");
    let attr_list = attributes.unwrap();
    assert!(!attr_list.is_empty(), "Should extract at least some attributes");
    
    // Verify specific attributes were extracted
    let contains_innovative = attr_list.iter().any(|attr| attr.contains("innovative"));
    let contains_dynamic = attr_list.iter().any(|attr| attr.contains("dynamic"));
    assert!(contains_innovative || contains_dynamic, "Should extract expected attributes");
    
    // Test empty response handling
    let empty_attributes = response_processor.process_response("").await;
    assert!(empty_attributes.is_ok(), "Should handle empty responses");
    assert!(empty_attributes.unwrap().is_empty(), "Empty response should yield no attributes");
}

#[tokio::test]
async fn test_response_processor_bloom_filter() {
    let response_processor = RealResponseProcessor::new();
    
    // Test bloom filter update
    let filter_data = create_test_bloom_filter_data();
    let filter_result = response_processor.set_bloom_filter(filter_data).await;
    assert!(filter_result.is_ok(), "Should successfully set bloom filter");
    
    // Test processing with bloom filter (should still work)
    let test_response = create_test_llm_response();
    let filtered_attributes = response_processor.process_response(&test_response).await;
    
    assert!(filtered_attributes.is_ok(), "Should process with bloom filter");
    // Bloom filter may reduce results, but shouldn't cause errors
    let attr_list = filtered_attributes.unwrap();
    // Don't assert on count since bloom filter may filter out duplicates
    
    // Test direct attribute extraction
    let extracted = response_processor.extract_attributes(&test_response).await;
    assert!(extracted.is_ok(), "Direct extraction should work");
    assert!(!extracted.unwrap().is_empty(), "Should extract attributes directly");
}

#[tokio::test]
async fn test_performance_tracker_metrics() {
    let performance_tracker = RealPerformanceTracker::new();
    
    // Test recording success
    let response_time = Duration::from_millis(1500);
    let success_result = performance_tracker.record_success("openai", response_time, 100).await;
    assert!(success_result.is_ok(), "Should record success metrics");
    
    // Test recording failure
    let failure = shared::ApiFailure::RateLimitExceeded;
    let failure_result = performance_tracker.record_failure("anthropic", failure).await;
    assert!(failure_result.is_ok(), "Should record failure metrics");
    
    // Test getting stats
    let stats = performance_tracker.get_stats().await;
    assert!(stats.is_ok(), "Should retrieve stats");
    
    let stats_map = stats.unwrap();
    // Should have recorded stats for both providers
    assert!(stats_map.contains_key("openai"), "Should have OpenAI stats");
    assert!(stats_map.contains_key("anthropic"), "Should have Anthropic stats");
    
    // Verify specific metrics
    let openai_stats = &stats_map["openai"];
    assert_eq!(openai_stats.successful_requests, 1, "Should have 1 successful request");
    assert_eq!(openai_stats.failed_requests, 0, "Should have 0 failed requests");
    
    let anthropic_stats = &stats_map["anthropic"];
    assert_eq!(anthropic_stats.successful_requests, 0, "Should have 0 successful requests");
    assert_eq!(anthropic_stats.failed_requests, 1, "Should have 1 failed request");
}

#[tokio::test]
async fn test_service_integration_flow() {
    let orchestrator_addr = create_test_addresses();
    
    // Create all services
    let ipc_communicator = RealIpcCommunicator::new();
    let provider_router = RealProviderRouter::new();
    let response_processor = RealResponseProcessor::new();
    let performance_tracker = RealPerformanceTracker::new();
    
    // Test command processing sequence
    let start_command = create_test_start_command();
    if let ProducerCommand::Start { routing_strategy, generation_config, prompt, .. } = start_command {
        // Set up routing strategy
        let strategy_result = provider_router.set_routing_strategy(routing_strategy).await;
        assert!(strategy_result.is_ok(), "Should set routing strategy");
        
        // Set generation config
        let config_result = provider_router.set_generation_config(generation_config).await;
        assert!(config_result.is_ok(), "Should set generation config");
        
        // Set prompt
        let prompt_result = provider_router.set_prompt(prompt).await;
        assert!(prompt_result.is_ok(), "Should set prompt");
        
        // Select provider (should work now that strategy is set)
        let selected_provider = provider_router.select_provider().await;
        assert!(selected_provider.is_ok(), "Should select provider after configuration");
        let provider_id = selected_provider.unwrap();
        assert!(matches!(provider_id, ProviderId::OpenAI | ProviderId::Anthropic));
        
        // Process a mock response
        let test_response = create_test_llm_response();
        let attributes = response_processor.process_response(&test_response).await;
        assert!(attributes.is_ok(), "Should process response");
        
        let attr_list = attributes.unwrap();
        assert!(!attr_list.is_empty(), "Should extract attributes");
        
        // Record performance metrics
        let response_time = Duration::from_millis(1200);
        let perf_result = performance_tracker.record_success("openai", response_time, 150).await;
        assert!(perf_result.is_ok(), "Should record performance");
        
        // Verify end-to-end flow worked
        let stats = performance_tracker.get_stats().await.unwrap();
        assert!(stats.contains_key("openai"), "Should have performance stats");
    }
}

#[tokio::test]
async fn test_error_handling_integration() {
    let provider_router = RealProviderRouter::new();
    let response_processor = RealResponseProcessor::new();
    let performance_tracker = RealPerformanceTracker::new();
    
    // Test empty routing strategy
    let empty_strategy = shared::RoutingStrategy::RoundRobin { providers: vec![] };
    let strategy_result = provider_router.set_routing_strategy(empty_strategy).await;
    assert!(strategy_result.is_ok(), "Should accept empty strategy");
    
    // Should handle empty provider list gracefully
    let provider_result = provider_router.select_provider().await;
    assert!(provider_result.is_err(), "Should fail with empty provider list");
    
    // Test processing malformed response
    let malformed_response = "This is not a properly formatted LLM response with no attributes";
    let empty_attributes = response_processor.process_response(&malformed_response).await;
    assert!(empty_attributes.is_ok(), "Should handle malformed response gracefully");
    // May or may not extract attributes from malformed response, but shouldn't crash
    
    // Test getting stats when none recorded initially
    let empty_stats = performance_tracker.get_stats().await;
    assert!(empty_stats.is_ok(), "Should return empty stats initially");
    let stats_map = empty_stats.unwrap();
    assert!(stats_map.is_empty(), "Should have no stats initially");
    
    // Test recording metrics with invalid provider names (should still work)
    let invalid_failure = shared::ApiFailure::NetworkError("Connection failed".to_string());
    let record_result = performance_tracker.record_failure("invalid_provider", invalid_failure).await;
    assert!(record_result.is_ok(), "Should handle invalid provider names gracefully");
}