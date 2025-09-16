//! Tests for seen_values synchronization between orchestrator and producer

use crate::fixtures::{CommandFactory, UpdateFactory, ProducerIdFactory};
// ProcessStatus import not needed for these tests
use shared::messages::producer::ProducerSyncStatus;

/// Test SyncCheck command with seen_values
#[tokio::test]
async fn test_sync_check_with_seen_values() {
    let producer_id = ProducerIdFactory::create();
    let seen_values = vec![
        "Japanese Dish 1".to_string(),
        "Japanese Dish 2".to_string(),
        "Japanese Dish 3".to_string(),
    ];
    
    // Create SyncCheck with seen values
    let sync_command = CommandFactory::sync_check_command_with_seen(
        1,
        Some(vec![0u8; 1024]), // Mock bloom filter
        true,
        Some(seen_values.clone()),
    );
    
    // Verify command structure
    match &sync_command {
        shared::ProducerCommand::SyncCheck { 
            sync_id, 
            bloom_filter, 
            seen_values: cmd_seen_values, 
            requires_dedup,
            .. 
        } => {
            assert_eq!(*sync_id, 1);
            assert!(bloom_filter.is_some());
            assert!(*requires_dedup);
            assert_eq!(cmd_seen_values.as_ref().unwrap(), &seen_values);
        }
        _ => panic!("Expected SyncCheck command"),
    }
    
    // Create expected producer response
    let sync_ack = UpdateFactory::sync_ack(
        producer_id,
        1,
        ProducerSyncStatus::BloomUpdated { filter_size_bytes: 1024 },
    );
    
    match sync_ack {
        shared::ProducerUpdate::SyncAck { sync_id, status, .. } => {
            assert_eq!(sync_id, 1);
            match status {
                ProducerSyncStatus::BloomUpdated { filter_size_bytes } => {
                    assert_eq!(filter_size_bytes, 1024);
                }
                _ => panic!("Expected BloomUpdated status"),
            }
        }
        _ => panic!("Expected SyncAck"),
    }
}

/// Test SyncCheck without seen_values (initial sync)
#[tokio::test]
async fn test_sync_check_without_seen_values() {
    let sync_command = CommandFactory::sync_check_command(1, None, false);
    
    match sync_command {
        shared::ProducerCommand::SyncCheck { seen_values, requires_dedup, .. } => {
            assert!(seen_values.is_none());
            assert!(!requires_dedup);
        }
        _ => panic!("Expected SyncCheck command"),
    }
}

/// Test large seen_values list handling
#[tokio::test]
async fn test_large_seen_values_list() {
    // Create a large list of seen values
    let large_seen_values: Vec<String> = (0..10000)
        .map(|i| format!("Seen Item {}", i))
        .collect();
    
    let sync_command = CommandFactory::sync_check_command_with_seen(
        2,
        Some(vec![1u8; 4096]), // Larger bloom filter
        true,
        Some(large_seen_values.clone()),
    );
    
    // Test serialization/deserialization of large message
    let serialized = bincode::serialize(&sync_command).unwrap();
    assert!(serialized.len() > 100000); // Should be substantial
    
    let deserialized: shared::ProducerCommand = bincode::deserialize(&serialized).unwrap();
    
    match deserialized {
        shared::ProducerCommand::SyncCheck { seen_values, .. } => {
            let deserialized_values = seen_values.unwrap();
            assert_eq!(deserialized_values.len(), 10000);
            assert_eq!(deserialized_values[0], "Seen Item 0");
            assert_eq!(deserialized_values[9999], "Seen Item 9999");
        }
        _ => panic!("Expected SyncCheck command"),
    }
}

/// Test seen_values integration with bloom filter sync
#[tokio::test]
async fn test_seen_values_bloom_filter_integration() {
    let producer_id = ProducerIdFactory::create();
    
    // Scenario: Orchestrator has 100 seen values and wants to sync them
    let orchestrator_seen_values: Vec<String> = (0..100)
        .map(|i| format!("Global Unique Item {}", i))
        .collect();
    
    // Step 1: Orchestrator sends SyncCheck with both bloom filter and seen values
    let sync_command = CommandFactory::sync_check_command_with_seen(
        10,
        Some(vec![42u8; 2048]), // Mock bloom filter data
        true,
        Some(orchestrator_seen_values.clone()),
    );
    
    // Step 2: Verify the sync command contains both pieces of data
    match &sync_command {
        shared::ProducerCommand::SyncCheck { 
            bloom_filter, 
            seen_values, 
            requires_dedup,
            .. 
        } => {
            assert!(bloom_filter.is_some());
            assert_eq!(bloom_filter.as_ref().unwrap().len(), 2048);
            
            assert!(seen_values.is_some());
            assert_eq!(seen_values.as_ref().unwrap().len(), 100);
            assert_eq!(seen_values.as_ref().unwrap()[0], "Global Unique Item 0");
            
            assert!(*requires_dedup);
        }
        _ => panic!("Expected SyncCheck command"),
    }
    
    // Step 3: Producer acknowledges successful sync
    let _success_ack = UpdateFactory::sync_ack(
        producer_id.clone(),
        10,
        ProducerSyncStatus::BloomUpdated { filter_size_bytes: 2048 },
    );
    
    // Step 4: Producer can now use both bloom filter and seen values for deduplication
    // This would be verified in the actual producer implementation
    
    // Step 5: Test failure scenario - producer is busy
    let busy_ack = UpdateFactory::sync_ack(
        producer_id.clone(),
        10,
        ProducerSyncStatus::Busy,
    );
    
    match busy_ack {
        shared::ProducerUpdate::SyncAck { status, .. } => {
            assert!(matches!(status, ProducerSyncStatus::Busy));
        }
        _ => panic!("Expected SyncAck"),
    }
    
    // Step 6: Test sync failure scenario
    let failed_ack = UpdateFactory::sync_ack(
        producer_id,
        10,
        ProducerSyncStatus::Failed { 
            reason: "Invalid seen_values format".to_string() 
        },
    );
    
    match failed_ack {
        shared::ProducerUpdate::SyncAck { status, .. } => {
            match status {
                ProducerSyncStatus::Failed { reason } => {
                    assert!(reason.contains("seen_values"));
                }
                _ => panic!("Expected Failed status"),
            }
        }
        _ => panic!("Expected SyncAck"),
    }
}

/// Test empty seen_values handling
#[tokio::test]
async fn test_empty_seen_values() {
    let sync_command = CommandFactory::sync_check_command_with_seen(
        3,
        Some(vec![0u8; 512]),
        true,
        Some(vec![]), // Empty but present
    );
    
    match sync_command {
        shared::ProducerCommand::SyncCheck { seen_values, .. } => {
            assert!(seen_values.is_some());
            assert_eq!(seen_values.unwrap().len(), 0);
        }
        _ => panic!("Expected SyncCheck command"),
    }
}

/// Test seen_values with special characters and encoding
#[tokio::test]
async fn test_seen_values_special_characters() {
    let special_values = vec![
        "Item with spaces and punctuation!".to_string(),
        "Unicode: 日本料理 (Japanese Cuisine)".to_string(),
        "Symbols: @#$%^&*()".to_string(),
        "Newlines\nand\ttabs".to_string(),
        "".to_string(), // Empty string
    ];
    
    let sync_command = CommandFactory::sync_check_command_with_seen(
        4,
        None, // Just seen values, no bloom filter
        false,
        Some(special_values.clone()),
    );
    
    // Test serialization/deserialization with special characters
    let serialized = bincode::serialize(&sync_command).unwrap();
    let deserialized: shared::ProducerCommand = bincode::deserialize(&serialized).unwrap();
    
    match deserialized {
        shared::ProducerCommand::SyncCheck { seen_values, .. } => {
            let deserialized_values = seen_values.unwrap();
            assert_eq!(deserialized_values, special_values);
            
            // Verify specific special characters are preserved
            assert!(deserialized_values[1].contains("日本料理"));
            assert!(deserialized_values[3].contains("\n"));
            assert_eq!(deserialized_values[4], "");
        }
        _ => panic!("Expected SyncCheck command"),
    }
}

/// Test message size limits with seen_values
#[tokio::test]
async fn test_seen_values_message_size_limits() {
    // Test reasonable size limit (e.g., 1MB message limit)
    let large_values: Vec<String> = (0..50000)
        .map(|i| format!("Very long attribute name that represents item number {} with additional descriptive text to increase size", i))
        .collect();
    
    let sync_command = CommandFactory::sync_check_command_with_seen(
        5,
        Some(vec![0u8; 8192]), // 8KB bloom filter
        true,
        Some(large_values),
    );
    
    let serialized = bincode::serialize(&sync_command).unwrap();
    
    // Message should be large but not exceed reasonable limits
    assert!(serialized.len() > 1_000_000); // > 1MB
    assert!(serialized.len() < 10_000_000); // < 10MB (reasonable limit)
    
    // Should still deserialize correctly
    let deserialized: shared::ProducerCommand = bincode::deserialize(&serialized).unwrap();
    match deserialized {
        shared::ProducerCommand::SyncCheck { seen_values, .. } => {
            assert_eq!(seen_values.unwrap().len(), 50000);
        }
        _ => panic!("Expected SyncCheck command"),
    }
}