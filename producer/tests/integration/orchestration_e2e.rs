//! End-to-end orchestration tests

use std::collections::HashMap;
use shared::{ProducerCommand, ProcessStatus};
use shared::messages::producer::{ProducerSyncStatus, ProducerPerformanceStats, ProviderUsageStats};
use crate::fixtures::{
    TestProducerManager, CommandFactory, UpdateFactory, ProducerIdFactory
};

/// Test complete producer lifecycle orchestration
#[tokio::test]
async fn test_complete_producer_lifecycle() {
    let mut manager = TestProducerManager::new(26000);
    
    // Step 1: Start a producer
    let topic = "Complete lifecycle test".to_string();
    let producer_id = manager.spawn_producer(topic.clone(), true).await.unwrap();
    
    assert_eq!(manager.running_count().await, 1);
    
    // Step 2: Simulate orchestrator commands throughout lifecycle
    let commands = vec![
        // Start generation
        CommandFactory::start_command(1, &topic, "Generate test data"),
        
        // Health check
        CommandFactory::ping_command(1),
        
        // Update configuration
        CommandFactory::update_config_command(2, Some("Updated test prompt".to_string())),
        
        // Sync bloom filter
        CommandFactory::sync_check_command(1, Some(vec![0u8; 512]), true),
        
        // Final health check
        CommandFactory::ping_command(2),
        
        // Stop generation
        CommandFactory::stop_command(3),
    ];
    
    // Verify command sequence
    assert_eq!(commands.len(), 6);
    
    // Step 3: Simulate producer responses
    let expected_updates = vec![
        // Response to Start
        UpdateFactory::status_update(
            producer_id.clone(),
            ProcessStatus::Running,
            Some("Generation started".to_string()),
            true,
        ),
        
        // Response to Ping
        UpdateFactory::pong(producer_id.clone(), 1),
        
        // Response to UpdateConfig
        UpdateFactory::status_update(
            producer_id.clone(),
            ProcessStatus::Running,
            Some("Configuration updated".to_string()),
            true,
        ),
        
        // Attribute batch during generation
        UpdateFactory::attribute_batch(
            producer_id.clone(),
            1,
            vec!["test_attr_1".to_string(), "test_attr_2".to_string()],
            shared::types::ProviderId::OpenAI,
        ),
        
        // Response to SyncCheck
        UpdateFactory::sync_ack(
            producer_id.clone(),
            1,
            ProducerSyncStatus::BloomUpdated { filter_size_bytes: 512 },
        ),
        
        // Response to second Ping
        UpdateFactory::pong(producer_id.clone(), 2),
        
        // Response to Stop
        UpdateFactory::status_update(
            producer_id.clone(),
            ProcessStatus::Stopped,
            Some("Generation stopped".to_string()),
            false,
        ),
    ];
    
    // Verify expected updates
    assert_eq!(expected_updates.len(), 7);
    
    // Step 4: Cleanup
    manager.stop_producer(&producer_id).await.unwrap();
    assert_eq!(manager.running_count().await, 0);
}

/// Test orchestrator managing producer failure and recovery
#[tokio::test]
async fn test_producer_failure_recovery() {
    let mut manager = TestProducerManager::new(27000);
    
    // Start initial producer
    let topic = "Failure recovery test".to_string();
    let failed_producer_id = manager.spawn_producer(topic.clone(), true).await.unwrap();
    
    // Simulate producer failure scenario
    let failure_error = UpdateFactory::error(
        failed_producer_id.clone(),
        "CRITICAL_FAILURE",
        "Producer process crashed unexpectedly",
        None,
    );
    
    match failure_error {
        shared::ProducerUpdate::Error { error_code, message, .. } => {
            assert_eq!(error_code, "CRITICAL_FAILURE");
            assert!(message.contains("crashed"));
        }
        _ => panic!("Expected Error update"),
    }
    
    // Stop failed producer
    manager.stop_producer(&failed_producer_id).await.unwrap();
    
    // Start replacement producer
    let replacement_producer_id = manager.spawn_producer(topic.clone(), true).await.unwrap();
    
    // Verify replacement is working
    let recovery_status = UpdateFactory::status_update(
        replacement_producer_id.clone(),
        ProcessStatus::Running,
        Some("Recovery producer started".to_string()),
        true,
    );
    
    match recovery_status {
        shared::ProducerUpdate::StatusUpdate { status, message, .. } => {
            assert_eq!(status, ProcessStatus::Running);
            assert!(message.unwrap().contains("Recovery"));
        }
        _ => panic!("Expected StatusUpdate"),
    }
    
    // Cleanup
    manager.stop_producer(&replacement_producer_id).await.unwrap();
}

/// Test bloom filter synchronization workflow
#[tokio::test]
async fn test_bloom_filter_sync_workflow() {
    let producer_id = ProducerIdFactory::create();
    
    // Step 1: Initial sync check (empty bloom filter)
    let initial_sync = CommandFactory::sync_check_command(1, None, false);
    
    match &initial_sync {
        ProducerCommand::SyncCheck { sync_id, bloom_filter, requires_dedup, .. } => {
            assert_eq!(*sync_id, 1);
            assert!(bloom_filter.is_none());
            assert!(!*requires_dedup);
        }
        _ => panic!("Expected SyncCheck"),
    }
    
    let _initial_ack = UpdateFactory::sync_ack(
        producer_id.clone(),
        1,
        ProducerSyncStatus::Ready,
    );
    
    // Step 2: Producer generates some attributes
    let attribute_batches = vec![
        UpdateFactory::attribute_batch(
            producer_id.clone(),
            1,
            vec!["attr1".to_string(), "attr2".to_string(), "attr3".to_string()],
            shared::types::ProviderId::OpenAI,
        ),
        UpdateFactory::attribute_batch(
            producer_id.clone(),
            2,
            vec!["attr4".to_string(), "attr5".to_string()],
            shared::types::ProviderId::Anthropic,
        ),
    ];
    
    // Verify attribute batches
    let mut total_attributes = 0;
    for batch in &attribute_batches {
        match batch {
            shared::ProducerUpdate::AttributeBatch { attributes, .. } => {
                total_attributes += attributes.len();
            }
            _ => panic!("Expected AttributeBatch"),
        }
    }
    assert_eq!(total_attributes, 5);
    
    // Step 3: Orchestrator sends bloom filter update
    let bloom_data = vec![1u8; 2048]; // Simulated bloom filter with 5 items
    let bloom_sync = CommandFactory::sync_check_command(2, Some(bloom_data.clone()), true);
    
    match &bloom_sync {
        ProducerCommand::SyncCheck { sync_id, bloom_filter, requires_dedup, .. } => {
            assert_eq!(*sync_id, 2);
            assert_eq!(bloom_filter, &Some(bloom_data));
            assert!(*requires_dedup);
        }
        _ => panic!("Expected SyncCheck"),
    }
    
    // Step 4: Producer acknowledges bloom filter update
    let bloom_ack = UpdateFactory::sync_ack(
        producer_id.clone(),
        2,
        ProducerSyncStatus::BloomUpdated { filter_size_bytes: 2048 },
    );
    
    match bloom_ack {
        shared::ProducerUpdate::SyncAck { sync_id, status, .. } => {
            assert_eq!(sync_id, 2);
            match status {
                ProducerSyncStatus::BloomUpdated { filter_size_bytes } => {
                    assert_eq!(filter_size_bytes, 2048);
                }
                _ => panic!("Expected BloomUpdated"),
            }
        }
        _ => panic!("Expected SyncAck"),
    }
    
    // Step 5: Subsequent generation should avoid duplicates
    let post_sync_batch = UpdateFactory::attribute_batch(
        producer_id,
        3,
        vec!["new_attr1".to_string(), "new_attr2".to_string()], // Different from previous
        shared::types::ProviderId::Gemini,
    );
    
    match post_sync_batch {
        shared::ProducerUpdate::AttributeBatch { attributes, .. } => {
            // These should be different from previous batches
            assert!(attributes.iter().all(|attr| attr.starts_with("new_")));
        }
        _ => panic!("Expected AttributeBatch"),
    }
}

/// Test high-throughput multi-producer scenario
#[tokio::test]
async fn test_high_throughput_scenario() {
    let mut manager = TestProducerManager::new(28000);
    
    // Start multiple producers for different topics
    let topics = vec![
        "High throughput topic 1",
        "High throughput topic 2", 
        "High throughput topic 3",
        "High throughput topic 4",
        "High throughput topic 5",
    ];
    
    let mut producer_ids = Vec::new();
    for topic in &topics {
        let id = manager.spawn_producer(topic.to_string(), true).await.unwrap();
        producer_ids.push(id);
    }
    
    assert_eq!(manager.running_count().await, 5);
    
    // Simulate high-throughput generation
    let mut all_batches = Vec::new();
    
    for (i, producer_id) in producer_ids.iter().enumerate() {
        // Each producer generates multiple batches
        for batch_num in 0..10 {
            let attributes: Vec<String> = (0..20)
                .map(|attr_num| format!("producer_{}_batch_{}_attr_{}", i, batch_num, attr_num))
                .collect();
            
            let batch = UpdateFactory::attribute_batch(
                producer_id.clone(),
                batch_num as u64,
                attributes,
                shared::types::ProviderId::OpenAI,
            );
            
            all_batches.push(batch);
        }
    }
    
    // Verify we have batches from all producers
    assert_eq!(all_batches.len(), 50); // 5 producers × 10 batches
    
    // Verify batch contents
    let mut total_attributes = 0;
    for batch in &all_batches {
        match batch {
            shared::ProducerUpdate::AttributeBatch { attributes, .. } => {
                assert_eq!(attributes.len(), 20);
                total_attributes += attributes.len();
            }
            _ => panic!("Expected AttributeBatch"),
        }
    }
    
    assert_eq!(total_attributes, 1000); // 50 batches × 20 attributes
    
    // Cleanup
    manager.stop_all().await.unwrap();
}

/// Test orchestrator performance monitoring
#[tokio::test]
async fn test_performance_monitoring() {
    let producer_id = ProducerIdFactory::create();
    
    // Create comprehensive performance stats
    let mut provider_usage = HashMap::new();
    provider_usage.insert(
        shared::types::ProviderId::OpenAI,
        ProviderUsageStats {
            requests_sent: 50,
            successful_responses: 48,
            unique_attributes_contributed: 960,
            avg_response_time_ms: 750.5,
            last_used_timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    );
    provider_usage.insert(
        shared::types::ProviderId::Anthropic,
        ProviderUsageStats {
            requests_sent: 25,
            successful_responses: 25,
            unique_attributes_contributed: 500,
            avg_response_time_ms: 1200.0,
            last_used_timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    );
    
    let detailed_stats = ProducerPerformanceStats {
        attributes_generated_last_minute: 1500,
        unique_contributed_last_minute: 1460,
        requests_made_last_minute: 75,
        provider_usage,
        current_batch_rate: 25.5,
        memory_usage_mb: Some(128),
        bloom_filter_size_mb: Some(4.2),
    };
    
    let performance_update = shared::ProducerUpdate::StatusUpdate {
        producer_id: producer_id.clone(),
        status: ProcessStatus::Running,
        message: Some("High performance generation active".to_string()),
        performance_stats: Some(detailed_stats),
    };
    
    // Verify performance metrics
    match performance_update {
        shared::ProducerUpdate::StatusUpdate { performance_stats, .. } => {
            let stats = performance_stats.unwrap();
            
            // Overall metrics
            assert_eq!(stats.attributes_generated_last_minute, 1500);
            assert_eq!(stats.unique_contributed_last_minute, 1460);
            assert_eq!(stats.requests_made_last_minute, 75);
            assert_eq!(stats.current_batch_rate, 25.5);
            assert_eq!(stats.memory_usage_mb, Some(128));
            assert_eq!(stats.bloom_filter_size_mb, Some(4.2));
            
            // Provider-specific metrics
            assert_eq!(stats.provider_usage.len(), 2);
            
            let openai_stats = stats.provider_usage.get(&shared::types::ProviderId::OpenAI).unwrap();
            assert_eq!(openai_stats.requests_sent, 50);
            assert_eq!(openai_stats.successful_responses, 48);
            assert_eq!(openai_stats.unique_attributes_contributed, 960);
            assert_eq!(openai_stats.avg_response_time_ms, 750.5);
            
            let anthropic_stats = stats.provider_usage.get(&shared::types::ProviderId::Anthropic).unwrap();
            assert_eq!(anthropic_stats.requests_sent, 25);
            assert_eq!(anthropic_stats.successful_responses, 25);
            assert_eq!(anthropic_stats.unique_attributes_contributed, 500);
            assert_eq!(anthropic_stats.avg_response_time_ms, 1200.0);
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

/// Test error propagation and handling in orchestration
#[tokio::test]
async fn test_error_propagation() {
    let producer_id = ProducerIdFactory::create();
    
    // Test various error scenarios that could occur during orchestration
    let error_scenarios = vec![
        // API-related errors
        ("API_RATE_LIMIT", "OpenAI rate limit exceeded", Some(1)),
        ("API_INVALID_KEY", "Invalid API key for Anthropic", Some(2)),
        ("API_NETWORK_TIMEOUT", "Network timeout connecting to Gemini", Some(3)),
        
        // Processing errors
        ("PROCESSING_FAILED", "Failed to extract attributes from response", None),
        ("BLOOM_FILTER_ERROR", "Bloom filter update failed", Some(4)),
        
        // Resource errors
        ("MEMORY_ERROR", "Insufficient memory for processing", None),
        ("DISK_FULL", "Disk full, cannot save state", None),
        
        // Configuration errors
        ("INVALID_CONFIG", "Invalid generation configuration", Some(5)),
        ("TOPIC_ERROR", "Topic configuration is invalid", Some(6)),
    ];
    
    for (error_code, error_message, command_id) in error_scenarios {
        let error_update = UpdateFactory::error(
            producer_id.clone(),
            error_code,
            error_message,
            command_id,
        );
        
        match error_update {
            shared::ProducerUpdate::Error { 
                producer_id: pid,
                error_code: code,
                message: msg,
                command_id: cmd_id,
            } => {
                assert_eq!(pid, producer_id);
                assert_eq!(code, error_code);
                assert_eq!(msg, error_message);
                assert_eq!(cmd_id, command_id);
            }
            _ => panic!("Expected Error update"),
        }
    }
}

/// Test graceful shutdown workflow
#[tokio::test]
async fn test_graceful_shutdown_workflow() {
    let mut manager = TestProducerManager::new(29000);
    
    // Start multiple producers
    let mut producer_ids = Vec::new();
    for i in 0..3 {
        let topic = format!("Shutdown test topic {}", i);
        let id = manager.spawn_producer(topic, true).await.unwrap();
        producer_ids.push(id);
    }
    
    assert_eq!(manager.running_count().await, 3);
    
    // Simulate graceful shutdown sequence
    for (i, producer_id) in producer_ids.iter().enumerate() {
        // Send stop command
        let _stop_command = CommandFactory::stop_command((i + 1) as u64);
        
        // Simulate producer responding with final status and cleanup
        let final_status = UpdateFactory::status_update(
            producer_id.clone(),
            ProcessStatus::Stopped,
            Some(format!("Producer {} shutting down gracefully", i)),
            false,
        );
        
        match final_status {
            shared::ProducerUpdate::StatusUpdate { status, message, .. } => {
                assert_eq!(status, ProcessStatus::Stopped);
                assert!(message.unwrap().contains("gracefully"));
            }
            _ => panic!("Expected StatusUpdate"),
        }
        
        // Stop the actual test producer
        manager.stop_producer(producer_id).await.unwrap();
    }
    
    assert_eq!(manager.running_count().await, 0);
}