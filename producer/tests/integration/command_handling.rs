//! Producer command handling integration tests

use crate::fixtures::{CommandFactory, ProcessIdFactory, UpdateFactory};
use shared::messages::producer::ProducerSyncStatus;
use shared::{ProcessStatus, ProducerCommand};

/// Test Start command handling
#[tokio::test]
async fn test_start_command_handling() {
    let producer_id = ProcessIdFactory::create();

    // Create Start command
    let start_command = CommandFactory::start_command(1, "Japanese cuisine dishes", "Generate unique Japanese dishes");

    match &start_command {
        ProducerCommand::Start {
            command_id,
            topic,
            prompt,
            routing_strategy,
            generation_config,
        } => {
            assert_eq!(*command_id, 1);
            assert_eq!(topic, "Japanese cuisine dishes");
            assert_eq!(prompt, "Generate unique Japanese dishes");

            // Verify routing strategy
            match routing_strategy {
                shared::types::RoutingStrategy::RoundRobin { providers } => {
                    assert!(providers.contains(&shared::types::ProviderId::OpenAI));
                    assert!(providers.contains(&shared::types::ProviderId::Anthropic));
                }
                _ => panic!("Expected RoundRobin routing strategy"),
            }

            // Verify generation config
            assert_eq!(generation_config.model, "gpt-4o-mini");
            assert_eq!(generation_config.batch_size, 1);
            assert_eq!(generation_config.context_window, 4096);
            assert_eq!(generation_config.max_tokens, 150);
            assert_eq!(generation_config.temperature, 0.7);
        }
        _ => panic!("Expected Start command"),
    }

    // Simulate producer response to start command
    let status_response = UpdateFactory::status_update(
        producer_id,
        ProcessStatus::Running,
        Some("Started generation for Japanese cuisine dishes".to_string()),
        true,
    );

    match status_response {
        shared::ProducerUpdate::StatusUpdate {
            status,
            message,
            performance_stats,
            ..
        } => {
            assert_eq!(status, ProcessStatus::Running);
            assert!(message.unwrap().contains("Japanese cuisine"));
            assert!(performance_stats.is_some());
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

/// Test Stop command handling
#[tokio::test]
async fn test_stop_command_handling() {
    let producer_id = ProcessIdFactory::create();

    // Create Stop command
    let stop_command = CommandFactory::stop_command(2);

    match stop_command {
        ProducerCommand::Stop { command_id } => {
            assert_eq!(command_id, 2);
        }
        _ => panic!("Expected Stop command"),
    }

    // Simulate producer response to stop command
    let status_response = UpdateFactory::status_update(
        producer_id,
        ProcessStatus::Stopped,
        Some("Generation stopped".to_string()),
        false,
    );

    match status_response {
        shared::ProducerUpdate::StatusUpdate {
            status,
            message,
            performance_stats,
            ..
        } => {
            assert_eq!(status, ProcessStatus::Stopped);
            assert_eq!(message, Some("Generation stopped".to_string()));
            assert!(performance_stats.is_none());
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

/// Test UpdateConfig command handling
#[tokio::test]
async fn test_update_config_command_handling() {
    let producer_id = ProcessIdFactory::create();

    // Test prompt update
    let config_update = CommandFactory::update_config_command(3, Some("Generate Italian pasta dishes".to_string()));

    match &config_update {
        ProducerCommand::UpdateConfig {
            command_id,
            prompt,
            routing_strategy,
            generation_config,
        } => {
            assert_eq!(*command_id, 3);
            assert_eq!(prompt, &Some("Generate Italian pasta dishes".to_string()));
            assert!(routing_strategy.is_none());
            assert!(generation_config.is_none());
        }
        _ => panic!("Expected UpdateConfig command"),
    }

    // Simulate producer acknowledgment
    let ack_response = UpdateFactory::status_update(
        producer_id,
        ProcessStatus::Running,
        Some("Configuration updated: Italian pasta dishes".to_string()),
        true,
    );

    match ack_response {
        shared::ProducerUpdate::StatusUpdate { message, .. } => {
            assert!(message.unwrap().contains("Italian pasta"));
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

/// Test SyncCheck command handling
#[tokio::test]
async fn test_sync_check_command_handling() {
    let producer_id = ProcessIdFactory::create();

    // Create bloom filter data
    let bloom_data = vec![0u8, 1u8, 0u8, 1u8]; // Mock bloom filter

    let sync_command = CommandFactory::sync_check_command(4, Some(bloom_data.clone()), true);

    match &sync_command {
        ProducerCommand::SyncCheck {
            sync_id,
            bloom_filter,
            bloom_version,
            requires_dedup,
            timestamp,
            ..
        } => {
            assert_eq!(*sync_id, 4);
            assert_eq!(bloom_filter, &Some(bloom_data));
            assert_eq!(*bloom_version, Some(1));
            assert!(*requires_dedup);
            assert!(*timestamp > 0);
        }
        _ => panic!("Expected SyncCheck command"),
    }

    // Test different sync response scenarios

    // 1. Successful bloom filter update
    let success_response = UpdateFactory::sync_ack(
        producer_id.clone(),
        4,
        ProducerSyncStatus::BloomUpdated {
            filter_size_bytes: 1024,
        },
    );

    match success_response {
        shared::ProducerUpdate::SyncAck {
            sync_id,
            status,
            bloom_version,
            ..
        } => {
            assert_eq!(sync_id, 4);
            assert_eq!(bloom_version, Some(1));
            match status {
                ProducerSyncStatus::BloomUpdated { filter_size_bytes } => {
                    assert_eq!(filter_size_bytes, 1024);
                }
                _ => panic!("Expected BloomUpdated status"),
            }
        }
        _ => panic!("Expected SyncAck"),
    }

    // 2. Producer busy response
    let busy_response = UpdateFactory::sync_ack(producer_id.clone(), 4, ProducerSyncStatus::Busy);

    match busy_response {
        shared::ProducerUpdate::SyncAck { status, .. } => {
            assert!(matches!(status, ProducerSyncStatus::Busy));
        }
        _ => panic!("Expected SyncAck"),
    }

    // 3. Sync failed response
    let failed_response = UpdateFactory::sync_ack(
        producer_id,
        4,
        ProducerSyncStatus::Failed {
            reason: "Invalid bloom filter format".to_string(),
        },
    );

    match failed_response {
        shared::ProducerUpdate::SyncAck { status, .. } => match status {
            ProducerSyncStatus::Failed { reason } => {
                assert!(reason.contains("Invalid bloom filter"));
            }
            _ => panic!("Expected Failed status"),
        },
        _ => panic!("Expected SyncAck"),
    }
}

/// Test Ping/Pong command handling
#[tokio::test]
async fn test_ping_pong_handling() {
    let producer_id = ProcessIdFactory::create();

    // Create Ping command
    let ping_command = CommandFactory::ping_command(999);

    match ping_command {
        ProducerCommand::Ping { ping_id } => {
            assert_eq!(ping_id, 999);
        }
        _ => panic!("Expected Ping command"),
    }

    // Create Pong response
    let pong_response = UpdateFactory::pong(producer_id, 999);

    match pong_response {
        shared::ProducerUpdate::Pong { ping_id, .. } => {
            assert_eq!(ping_id, 999);
        }
        _ => panic!("Expected Pong response"),
    }
}

/// Test command sequencing scenarios
#[tokio::test]
async fn test_command_sequencing() {
    let producer_id = ProcessIdFactory::create();

    // Scenario 1: Start -> UpdateConfig -> Stop
    let commands = vec![
        CommandFactory::start_command(1, "topic1", "prompt1"),
        CommandFactory::update_config_command(2, Some("updated prompt".to_string())),
        CommandFactory::stop_command(3),
    ];

    let expected_responses = vec![
        ProcessStatus::Running, // After Start
        ProcessStatus::Running, // After UpdateConfig
        ProcessStatus::Stopped, // After Stop
    ];

    for (i, (command, expected_status)) in commands.iter().zip(expected_responses.iter()).enumerate() {
        // Verify command structure
        match command {
            ProducerCommand::Start { command_id, .. } => assert_eq!(*command_id, 1),
            ProducerCommand::UpdateConfig { command_id, .. } => assert_eq!(*command_id, 2),
            ProducerCommand::Stop { command_id } => assert_eq!(*command_id, 3),
            _ => {}
        }

        // Create expected response
        let response = UpdateFactory::status_update(
            producer_id.clone(),
            *expected_status,
            Some(format!("Command {} processed", i + 1)),
            *expected_status == ProcessStatus::Running,
        );

        match response {
            shared::ProducerUpdate::StatusUpdate { status, .. } => {
                assert_eq!(status, *expected_status);
            }
            _ => panic!("Expected StatusUpdate"),
        }
    }
}

/// Test invalid command scenarios
#[tokio::test]
async fn test_invalid_command_handling() {
    let producer_id = ProcessIdFactory::create();

    // Test error responses for various scenarios
    let error_scenarios = vec![
        ("INVALID_STATE", "Received Start command while already running", Some(1)),
        ("INVALID_CONFIG", "Invalid generation configuration", Some(2)),
        ("SYNC_FAILED", "Bloom filter sync failed", Some(3)),
        ("API_ERROR", "OpenAI API key invalid", None),
    ];

    for (error_code, message, command_id) in error_scenarios {
        let error_response = UpdateFactory::error(producer_id.clone(), error_code, message, command_id);

        match error_response {
            shared::ProducerUpdate::Error {
                error_code: code,
                message: msg,
                command_id: cmd_id,
                ..
            } => {
                assert_eq!(code, error_code);
                assert_eq!(msg, message);
                assert_eq!(cmd_id, command_id);
            }
            _ => panic!("Expected Error update"),
        }
    }
}

/// Test performance stats in status updates
#[tokio::test]
async fn test_performance_stats_reporting() {
    let producer_id = ProcessIdFactory::create();

    // Create status update with detailed performance stats
    let status_update = UpdateFactory::status_update(
        producer_id,
        ProcessStatus::Running,
        Some("High performance generation".to_string()),
        true,
    );

    match status_update {
        shared::ProducerUpdate::StatusUpdate { performance_stats, .. } => {
            let stats = performance_stats.unwrap();

            // Verify performance metrics
            assert_eq!(stats.attributes_generated_last_minute, 50);
            assert_eq!(stats.unique_contributed_last_minute, 48);
            assert_eq!(stats.requests_made_last_minute, 2);
            assert_eq!(stats.current_batch_rate, 25.0);
            assert_eq!(stats.memory_usage_mb, Some(64));
            assert_eq!(stats.bloom_filter_size_mb, Some(2.5));

            // Provider usage should be empty in basic test
            assert!(stats.provider_usage.is_empty());
        }
        _ => panic!("Expected StatusUpdate"),
    }
}

/// Test attribute batch processing
#[tokio::test]
async fn test_attribute_batch_processing() {
    let producer_id = ProcessIdFactory::create();

    // Create attribute batches from different providers
    let providers = vec![
        shared::types::ProviderId::OpenAI,
        shared::types::ProviderId::Anthropic,
        shared::types::ProviderId::Gemini,
    ];

    for (batch_id, provider) in providers.iter().enumerate() {
        let attributes = (0..10).map(|i| format!("attribute_{}_{}", batch_id, i)).collect();

        let batch_update = UpdateFactory::attribute_batch(producer_id.clone(), batch_id as u64, attributes, *provider);

        match batch_update {
            shared::ProducerUpdate::AttributeBatch {
                batch_id: bid,
                attributes,
                provider_metadata,
                ..
            } => {
                assert_eq!(bid, batch_id as u64);
                assert_eq!(attributes.len(), 10);
                assert_eq!(provider_metadata.provider_id, *provider);
                assert_eq!(provider_metadata.model, "gpt-4o-mini");
                assert_eq!(provider_metadata.tokens.total(), 100);
                assert!(provider_metadata.response_time_ms > 0);
                assert!(provider_metadata.request_timestamp > 0);
            }
            _ => panic!("Expected AttributeBatch"),
        }
    }
}
