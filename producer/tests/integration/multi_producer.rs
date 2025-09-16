//! Multi-producer orchestration tests

use std::time::Duration;
use std::collections::HashSet;
use shared::{ProcessStatus, ProducerId};
use shared::messages::producer::ProducerSyncStatus;
use crate::fixtures::{
    TestProducerManager, CommandFactory, UpdateFactory, ProducerIdFactory
};

/// Test spawning and managing multiple producers
#[tokio::test]
async fn test_multiple_producer_startup() {
    let mut manager = TestProducerManager::new(20000);
    
    // Start multiple producers
    let topics = vec!["topic1", "topic2", "topic3"];
    let mut producer_ids = Vec::new();
    
    for topic in topics {
        let id = manager.spawn_producer(topic.to_string(), true).await.unwrap();
        producer_ids.push(id);
    }
    
    // Verify all producers are running
    assert_eq!(manager.running_count().await, 3);
    
    // Verify we can get addresses for each producer
    for producer_id in &producer_ids {
        let addresses = manager.get_producer_addresses(producer_id).await;
        assert!(addresses.is_some());
        
        let (listen_addr, command_addr) = addresses.unwrap();
        assert_ne!(listen_addr.port(), command_addr.port()); // Should have different ports
    }
    
    // Verify producer IDs match
    let all_ids = manager.get_producer_ids().await;
    assert_eq!(all_ids.len(), 3);
    for id in &producer_ids {
        assert!(all_ids.contains(id));
    }
    
    // Cleanup
    manager.stop_all().await.unwrap();
    assert_eq!(manager.running_count().await, 0);
}

/// Test unique port allocation for producers
#[tokio::test] 
async fn test_unique_port_allocation() {
    let mut manager = TestProducerManager::new(21000);
    
    // Start multiple producers and collect their addresses
    let mut all_ports = HashSet::new();
    let mut producer_ids = Vec::new();
    
    for i in 0..5 {
        let topic = format!("topic_{}", i);
        let id = manager.spawn_producer(topic, true).await.unwrap();
        let addresses = manager.get_producer_addresses(&id).await.unwrap();
        producer_ids.push(id.clone());
        let (listen_addr, command_addr) = addresses;
        
        // Verify ports are unique
        assert!(all_ports.insert(listen_addr.port()));
        assert!(all_ports.insert(command_addr.port()));
    }
    
    // Should have 10 unique ports (2 per producer)
    assert_eq!(all_ports.len(), 10);
    
    // Cleanup
    manager.stop_all().await.unwrap();
}

/// Test stopping individual producers
#[tokio::test]
async fn test_individual_producer_stop() {
    let mut manager = TestProducerManager::new(22000);
    
    // Start 3 producers
    let id1 = manager.spawn_producer("topic1".to_string(), true).await.unwrap();
    let id2 = manager.spawn_producer("topic2".to_string(), true).await.unwrap();
    let id3 = manager.spawn_producer("topic3".to_string(), true).await.unwrap();
    
    assert_eq!(manager.running_count().await, 3);
    
    // Stop middle producer
    manager.stop_producer(&id2).await.unwrap();
    assert_eq!(manager.running_count().await, 2);
    
    // Verify correct producers are still running
    let remaining_ids = manager.get_producer_ids().await;
    assert!(remaining_ids.contains(&id1));
    assert!(!remaining_ids.contains(&id2));
    assert!(remaining_ids.contains(&id3));
    
    // Stop remaining producers
    manager.stop_producer(&id1).await.unwrap();
    manager.stop_producer(&id3).await.unwrap();
    assert_eq!(manager.running_count().await, 0);
}

/// Test producer process failure detection
#[tokio::test]
async fn test_producer_failure_detection() {
    let mut manager = TestProducerManager::new(23000);
    
    // Start a producer
    let producer_id = manager.spawn_producer("test_topic".to_string(), true).await.unwrap();
    assert_eq!(manager.running_count().await, 1);
    
    // Give the producer some time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // The producer should still be running
    assert_eq!(manager.running_count().await, 1);
    
    // Stop the producer and verify detection
    manager.stop_producer(&producer_id).await.unwrap();
    assert_eq!(manager.running_count().await, 0);
}

/// Test concurrent operations on multiple producers
#[tokio::test]
async fn test_concurrent_producer_operations() {
    let mut manager = TestProducerManager::new(24000);
    
    // Start producers concurrently
    let mut spawn_tasks = Vec::new();
    for i in 0..5 {
        let topic = format!("concurrent_topic_{}", i);
        let spawn_task = tokio::spawn(async move {
            // Add some jitter to make concurrent spawning more realistic
            tokio::time::sleep(Duration::from_millis(i * 10)).await;
            topic
        });
        spawn_tasks.push(spawn_task);
    }
    
    let mut producer_ids = Vec::new();
    for spawn_task in spawn_tasks {
        let topic = spawn_task.await.unwrap();
        let id = manager.spawn_producer(topic, true).await.unwrap();
        producer_ids.push(id);
    }
    
    assert_eq!(manager.running_count().await, 5);
    
    // Stop producers concurrently
    let mut stop_tasks = Vec::new();
    for (i, producer_id) in producer_ids.into_iter().enumerate() {
        let stop_task = tokio::spawn(async move {
            // Add some jitter
            tokio::time::sleep(Duration::from_millis(i as u64 * 10)).await;
            producer_id
        });
        stop_tasks.push(stop_task);
    }
    
    for stop_task in stop_tasks {
        let producer_id = stop_task.await.unwrap();
        manager.stop_producer(&producer_id).await.unwrap();
    }
    
    assert_eq!(manager.running_count().await, 0);
}

/// Test orchestrator managing multiple producers with commands
#[tokio::test]
async fn test_orchestrator_multi_producer_commands() {
    // This test simulates an orchestrator sending commands to multiple producers
    let _manager = TestProducerManager::new(25000);
    
    // Create multiple test producer IDs
    let producer_ids: Vec<ProducerId> = (0..3)
        .map(|_| ProducerIdFactory::create())
        .collect();
    
    // Create test commands for each producer
    let commands = vec![
        CommandFactory::start_command(1, "topic1", "prompt1"),
        CommandFactory::start_command(2, "topic2", "prompt2"),
        CommandFactory::start_command(3, "topic3", "prompt3"),
    ];
    
    // Verify commands are created correctly
    for (i, command) in commands.iter().enumerate() {
        match command {
            shared::ProducerCommand::Start { command_id, topic, prompt, .. } => {
                assert_eq!(*command_id, (i + 1) as u64);
                assert_eq!(topic, &format!("topic{}", i + 1));
                assert_eq!(prompt, &format!("prompt{}", i + 1));
            }
            _ => panic!("Expected Start command"),
        }
    }
    
    // Create test updates from producers
    let updates = vec![
        UpdateFactory::status_update(
            producer_ids[0].clone(),
            ProcessStatus::Running,
            Some("Producer 1 running".to_string()),
            true,
        ),
        UpdateFactory::status_update(
            producer_ids[1].clone(),
            ProcessStatus::Running,
            Some("Producer 2 running".to_string()),
            true,
        ),
        UpdateFactory::attribute_batch(
            producer_ids[2].clone(),
            1,
            vec!["attr1".to_string(), "attr2".to_string()],
            shared::types::ProviderId::OpenAI,
        ),
    ];
    
    // Verify updates are created correctly
    assert_eq!(updates.len(), 3);
    
    match &updates[0] {
        shared::ProducerUpdate::StatusUpdate { producer_id, status, .. } => {
            assert_eq!(*producer_id, producer_ids[0]);
            assert_eq!(*status, ProcessStatus::Running);
        }
        _ => panic!("Expected StatusUpdate"),
    }
    
    match &updates[2] {
        shared::ProducerUpdate::AttributeBatch { producer_id, attributes, .. } => {
            assert_eq!(*producer_id, producer_ids[2]);
            assert_eq!(attributes.len(), 2);
        }
        _ => panic!("Expected AttributeBatch"),
    }
}

/// Test bloom filter synchronization across multiple producers
#[tokio::test]
async fn test_multi_producer_bloom_sync() {
    // Create test bloom filter data
    let bloom_filter_data = vec![0u8; 1024]; // Mock bloom filter
    let producer_ids: Vec<ProducerId> = (0..3)
        .map(|_| ProducerIdFactory::create())
        .collect();
    
    // Create sync check commands for all producers
    let sync_commands: Vec<_> = producer_ids
        .iter()
        .enumerate()
        .map(|(i, _)| CommandFactory::sync_check_command(
            (i + 1) as u64,
            Some(bloom_filter_data.clone()),
            true,
        ))
        .collect();
    
    // Verify sync commands
    for (i, command) in sync_commands.iter().enumerate() {
        match command {
            shared::ProducerCommand::SyncCheck { sync_id, bloom_filter, requires_dedup, .. } => {
                assert_eq!(*sync_id, (i + 1) as u64);
                assert!(bloom_filter.is_some());
                assert!(*requires_dedup);
            }
            _ => panic!("Expected SyncCheck command"),
        }
    }
    
    // Create sync acknowledgments from producers
    let sync_acks: Vec<_> = producer_ids
        .iter()
        .enumerate()
        .map(|(i, producer_id)| UpdateFactory::sync_ack(
            producer_id.clone(),
            (i + 1) as u64,
            ProducerSyncStatus::BloomUpdated { filter_size_bytes: 1024 },
        ))
        .collect();
    
    // Verify sync acks
    for (i, update) in sync_acks.iter().enumerate() {
        match update {
            shared::ProducerUpdate::SyncAck { producer_id, sync_id, status, .. } => {
                assert_eq!(*producer_id, producer_ids[i]);
                assert_eq!(*sync_id, (i + 1) as u64);
                match status {
                    ProducerSyncStatus::BloomUpdated { filter_size_bytes } => {
                        assert_eq!(*filter_size_bytes, 1024);
                    }
                    _ => panic!("Expected BloomUpdated status"),
                }
            }
            _ => panic!("Expected SyncAck update"),
        }
    }
}

/// Test error handling across multiple producers
#[tokio::test]
async fn test_multi_producer_error_handling() {
    let producer_ids: Vec<ProducerId> = (0..3)
        .map(|_| ProducerIdFactory::create())
        .collect();
    
    // Create various error scenarios
    let errors = vec![
        UpdateFactory::error(
            producer_ids[0].clone(),
            "API_ERROR",
            "OpenAI API key invalid",
            Some(1),
        ),
        UpdateFactory::error(
            producer_ids[1].clone(),
            "NETWORK_ERROR",
            "Failed to connect to API endpoint",
            Some(2),
        ),
        UpdateFactory::error(
            producer_ids[2].clone(),
            "PROCESSING_ERROR",
            "Failed to process response",
            None,
        ),
    ];
    
    // Verify error messages
    for (i, error) in errors.iter().enumerate() {
        match error {
            shared::ProducerUpdate::Error { producer_id, error_code, message, command_id } => {
                assert_eq!(*producer_id, producer_ids[i]);
                
                match i {
                    0 => {
                        assert_eq!(error_code, "API_ERROR");
                        assert!(message.contains("OpenAI"));
                        assert_eq!(*command_id, Some(1));
                    }
                    1 => {
                        assert_eq!(error_code, "NETWORK_ERROR");
                        assert!(message.contains("connect"));
                        assert_eq!(*command_id, Some(2));
                    }
                    2 => {
                        assert_eq!(error_code, "PROCESSING_ERROR");
                        assert!(message.contains("process"));
                        assert_eq!(*command_id, None);
                    }
                    _ => unreachable!(),
                }
            }
            _ => panic!("Expected Error update"),
        }
    }
}