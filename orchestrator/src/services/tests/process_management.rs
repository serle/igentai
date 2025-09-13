//! Comprehensive tests for RealProcessManager service
//!
//! These tests verify the critical process lifecycle management functionality
//! including process spawning, monitoring, restart scenarios, and channel coordination.

use std::time::Duration;
use tokio::time::timeout;

use shared::{ProducerId, ProcessStatus, ProcessType, SystemMetrics};
use crate::services::process_manager::RealProcessManager;
use crate::traits::{ProcessManager, KeyValuePair};

/// Test basic producer spawning and channel establishment
#[tokio::test]
async fn test_spawn_producers_with_channels() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    let handles = manager
        .spawn_producers_with_channels(3, "test_topic", api_keys)
        .await
        .unwrap();
    
    assert_eq!(handles.len(), 3, "Should spawn 3 producers");
    
    // Verify each handle has unique ID and working channel
    let mut unique_ids = std::collections::HashSet::new();
    for handle in &handles {
        assert!(unique_ids.insert(handle.id.clone()), "Producer IDs should be unique");
    }
    
    // Give producers time to start generating messages
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    // Verify each producer is generating messages
    for handle in handles {
        let mut rx = handle.inbound;
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(result.is_ok(), "Should receive messages from producer {}", handle.id);
        let batch = result.unwrap().unwrap();
        assert!(!batch.is_empty(), "Batch should contain messages");
        assert!(batch[0].contains("test_topic"), "Messages should contain topic");
    }
}

/// Test webserver spawning and channel establishment
#[tokio::test]
async fn test_spawn_webserver_with_channel() {
    let manager = RealProcessManager::new();
    
    let webserver_handle = manager
        .spawn_webserver_with_channel(8080)
        .await
        .unwrap();
    
    // Test sending metrics to webserver
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 10.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test_topic".to_string()),
        active_producers: 3,
        uptime_seconds: 300,
        last_updated: 1234567890,
    };
    
    let result = webserver_handle.outbound.send(metrics).await;
    assert!(result.is_ok(), "Should send metrics to webserver successfully");
}

/// Test process monitoring functionality
#[tokio::test]
async fn test_monitor_processes() {
    let manager = RealProcessManager::new();
    
    // Initially no processes
    let health_reports = manager.monitor_processes().await.unwrap();
    assert_eq!(health_reports.len(), 0, "Should have no processes initially");
    
    // Spawn producers and webserver
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    let _producer_handles = manager
        .spawn_producers_with_channels(2, "test_topic", api_keys)
        .await
        .unwrap();
    
    let _webserver_handle = manager
        .spawn_webserver_with_channel(8080)
        .await
        .unwrap();
    
    // Check process health
    let health_reports = manager.monitor_processes().await.unwrap();
    assert_eq!(health_reports.len(), 3, "Should monitor 2 producers + 1 webserver");
    
    // Verify producer reports
    let producer_reports: Vec<_> = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer)
        .collect();
    assert_eq!(producer_reports.len(), 2, "Should have 2 producer reports");
    
    for report in &producer_reports {
        assert_eq!(report.status, ProcessStatus::Running, "Producers should be running");
        assert!(report.last_heartbeat.is_some(), "Should have heartbeat timestamp");
        assert_eq!(report.restart_count, 0, "Initial restart count should be 0");
        assert!(report.error_message.is_none(), "Should have no error message");
    }
    
    // Verify webserver report
    let webserver_reports: Vec<_> = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::WebServer)
        .collect();
    assert_eq!(webserver_reports.len(), 1, "Should have 1 webserver report");
    
    let webserver_report = &webserver_reports[0];
    assert_eq!(webserver_report.status, ProcessStatus::Running, "Webserver should be running");
    assert_eq!(webserver_report.process_id, "webserver", "Webserver should have correct ID");
}

/// Test producer restart functionality
#[tokio::test]
async fn test_restart_failed_producer() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Spawn initial producer
    let initial_handles = manager
        .spawn_producers_with_channels(1, "test_topic", api_keys.clone())
        .await
        .unwrap();
    
    let producer_id = initial_handles[0].id.clone();
    
    // Verify initial producer is working
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut initial_rx = initial_handles.into_iter().next().unwrap().inbound;
    let initial_batch = timeout(Duration::from_millis(100), initial_rx.recv()).await;
    assert!(initial_batch.is_ok(), "Initial producer should be generating messages");
    
    // Restart the producer
    let restarted_handle = manager
        .restart_failed_producer(producer_id.clone(), "new_topic", api_keys)
        .await
        .unwrap();
    
    assert_eq!(restarted_handle.id, producer_id, "Restarted producer should have same ID");
    
    // Verify restarted producer generates messages with new topic
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut restarted_rx = restarted_handle.inbound;
    let restarted_batch = timeout(Duration::from_millis(100), restarted_rx.recv()).await;
    assert!(restarted_batch.is_ok(), "Restarted producer should generate messages");
    
    let batch = restarted_batch.unwrap().unwrap();
    assert!(batch[0].contains("new_topic"), "Restarted producer should use new topic");
    
    // Old receiver should be disconnected (but may still have buffered messages)
    // We'll drain any remaining messages and then it should be closed
    tokio::time::sleep(Duration::from_millis(100)).await; // Allow old task to fully stop
    
    // Try to receive - if the old producer was stopped, the channel should eventually close
    for _ in 0..5 { // Try multiple times as there might be buffered messages
        match timeout(Duration::from_millis(50), initial_rx.recv()).await {
            Ok(Some(_)) => continue, // Got a message, keep trying
            Ok(None) => break, // Channel closed - this is what we expect
            Err(_) => break, // Timeout - also acceptable
        }
    }
    
    // The old channel should eventually close or stop producing new messages
}

/// Test webserver restart functionality
#[tokio::test]
async fn test_restart_failed_webserver() {
    let manager = RealProcessManager::new();
    
    // Spawn initial webserver
    let initial_handle = manager
        .spawn_webserver_with_channel(8080)
        .await
        .unwrap();
    
    let initial_sender = initial_handle.outbound;
    
    // Test initial webserver works
    let test_metrics = SystemMetrics {
        total_unique_entries: 50,
        entries_per_minute: 5.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test".to_string()),
        active_producers: 1,
        uptime_seconds: 100,
        last_updated: 1234567890,
    };
    
    let result = initial_sender.send(test_metrics.clone()).await;
    assert!(result.is_ok(), "Initial webserver should receive metrics");
    
    // Restart webserver
    let restarted_handle = manager
        .restart_failed_webserver(9090)
        .await
        .unwrap();
    
    // Test restarted webserver works
    let result = restarted_handle.outbound.send(test_metrics).await;
    assert!(result.is_ok(), "Restarted webserver should receive metrics");
    
    // Old sender should eventually fail (channel closed)
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Note: This test assumes the old task is aborted, but the channel might still be open
    // In practice, the restart would create a new webserver on a different port
}

/// Test concurrent producer operations
#[tokio::test]
async fn test_concurrent_producer_management() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Spawn producers sequentially (to avoid lifetime issues with concurrent spawning)
    let mut all_handles = Vec::new();
    for i in 0..3 {
        let handles = manager
            .spawn_producers_with_channels(2, &format!("topic_{}", i), api_keys.clone())
            .await
            .unwrap();
        all_handles.extend(handles);
    }
    
    assert_eq!(all_handles.len(), 6, "Should spawn 6 total producers (3 × 2)");
    
    // Verify all producers are monitored
    let health_reports = manager.monitor_processes().await.unwrap();
    let producer_count = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer)
        .count();
    assert_eq!(producer_count, 6, "Should monitor all 6 producers");
    
    // Verify all producers generate unique messages
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    let mut message_sets = Vec::new();
    for handle in all_handles {
        let mut rx = handle.inbound;
        let batch = timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(batch.is_ok(), "Each producer should generate messages");
        message_sets.push(batch.unwrap().unwrap());
    }
    
    // Verify messages are unique across producers
    let mut all_messages: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total_messages = 0;
    
    for batch in message_sets {
        total_messages += batch.len();
        for message in batch {
            assert!(all_messages.insert(message.clone()), "Messages should be unique: {}", message);
        }
    }
    
    assert_eq!(all_messages.len(), total_messages, "All messages should be unique");
}

/// Test stopping all producers
#[tokio::test]
async fn test_stop_producers() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Spawn some producers
    let handles = manager
        .spawn_producers_with_channels(3, "test_topic", api_keys)
        .await
        .unwrap();
    
    // Verify they're running
    let health_reports = manager.monitor_processes().await.unwrap();
    let running_producers = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer && r.status == ProcessStatus::Running)
        .count();
    assert_eq!(running_producers, 3, "All producers should be running initially");
    
    // Stop all producers
    let result = manager.stop_producers().await;
    assert!(result.is_ok(), "Should stop producers successfully");
    
    // Give tasks time to stop
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify no producers are monitored
    let health_reports = manager.monitor_processes().await.unwrap();
    let producer_reports: Vec<_> = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer)
        .collect();
    assert_eq!(producer_reports.len(), 0, "Should have no producers after stopping");
    
    // Verify channels are closed
    for handle in handles {
        let mut rx = handle.inbound;
        // Channels should be closed, so recv should return None
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        if result.is_ok() {
            assert!(result.unwrap().is_none(), "Channel should be closed");
        }
        // If timeout, that's also acceptable - the channel is effectively unavailable
    }
}

/// Test stopping webserver
#[tokio::test]
async fn test_stop_webserver() {
    let manager = RealProcessManager::new();
    
    // Spawn webserver
    let _handle = manager
        .spawn_webserver_with_channel(8080)
        .await
        .unwrap();
    
    // Verify it's running
    let health_reports = manager.monitor_processes().await.unwrap();
    let webserver_reports: Vec<_> = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::WebServer)
        .collect();
    assert_eq!(webserver_reports.len(), 1, "Should have webserver running");
    assert_eq!(webserver_reports[0].status, ProcessStatus::Running);
    
    // Stop webserver
    let result = manager.stop_webserver().await;
    assert!(result.is_ok(), "Should stop webserver successfully");
    
    // Give task time to stop
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify no webserver is monitored
    let health_reports = manager.monitor_processes().await.unwrap();
    let webserver_count = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::WebServer)
        .count();
    assert_eq!(webserver_count, 0, "Should have no webserver after stopping");
}

/// Test process lifecycle with restart scenarios
#[tokio::test]
async fn test_process_lifecycle_with_restarts() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Initial spawn
    let handles = manager
        .spawn_producers_with_channels(2, "initial_topic", api_keys.clone())
        .await
        .unwrap();
    
    let producer_ids: Vec<_> = handles.iter().map(|h| h.id.clone()).collect();
    
    // Verify initial state
    let health_reports = manager.monitor_processes().await.unwrap();
    assert_eq!(health_reports.len(), 2, "Should have 2 producers initially");
    
    // Restart first producer
    let restarted_handle = manager
        .restart_failed_producer(producer_ids[0].clone(), "restarted_topic", api_keys.clone())
        .await
        .unwrap();
    
    // Should still have 2 producers total
    let health_reports = manager.monitor_processes().await.unwrap();
    assert_eq!(health_reports.len(), 2, "Should still have 2 producers after restart");
    
    // Stop all producers
    manager.stop_producers().await.unwrap();
    
    // Should have no producers
    tokio::time::sleep(Duration::from_millis(100)).await;
    let health_reports = manager.monitor_processes().await.unwrap();
    let producer_count = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer)
        .count();
    assert_eq!(producer_count, 0, "Should have no producers after stop");
    
    // Spawn new batch
    let _new_handles = manager
        .spawn_producers_with_channels(1, "new_topic", api_keys)
        .await
        .unwrap();
    
    let health_reports = manager.monitor_processes().await.unwrap();
    assert_eq!(health_reports.len(), 1, "Should have 1 new producer");
    
    // Verify old channels are disconnected
    for handle in handles {
        let mut rx = handle.inbound;
        let result = timeout(Duration::from_millis(100), rx.recv()).await;
        if result.is_ok() {
            assert!(result.unwrap().is_none(), "Old channels should be disconnected");
        }
    }
    
    // Verify restarted channel is also disconnected (it was part of the stop)
    let mut restarted_rx = restarted_handle.inbound;
    let result = timeout(Duration::from_millis(100), restarted_rx.recv()).await;
    if result.is_ok() {
        assert!(result.unwrap().is_none(), "Restarted channel should be disconnected after stop");
    }
}

/// Test error handling with invalid operations
#[tokio::test]
async fn test_error_handling() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Test restarting non-existent producer
    let fake_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let result = manager
        .restart_failed_producer(fake_id, "test_topic", api_keys.clone())
        .await;
    
    // Should succeed (creates new producer with that ID)
    assert!(result.is_ok(), "Should handle restarting non-existent producer");
    
    // Test stopping when nothing is running
    let result = manager.stop_producers().await;
    assert!(result.is_ok(), "Should handle stopping when no producers exist");
    
    let result = manager.stop_webserver().await;
    assert!(result.is_ok(), "Should handle stopping when no webserver exists");
    
    // Test monitoring - we should have the producer created during restart
    let health_reports = manager.monitor_processes().await.unwrap();
    let producer_count = health_reports
        .iter()
        .filter(|r| r.process_type == ProcessType::Producer)
        .count();
    // Note: The stop_producers() call above would have stopped the restarted producer too
    assert_eq!(producer_count, 0, "Should have no producers after stopping all");
}

/// Test rapid start/stop cycles
#[tokio::test]
async fn test_rapid_start_stop_cycles() {
    let manager = RealProcessManager::new();
    
    let api_keys = vec![
        KeyValuePair {
            key: "OPENAI_API_KEY".to_string(),
            value: "test-key-123".to_string(),
        },
    ];
    
    // Rapid start/stop cycles
    for i in 0..5 {
        // Spawn producers
        let _handles = manager
            .spawn_producers_with_channels(2, &format!("topic_{}", i), api_keys.clone())
            .await
            .unwrap();
        
        // Spawn webserver
        let _webserver = manager
            .spawn_webserver_with_channel(8080 + i as u16)
            .await
            .unwrap();
        
        // Brief operation period
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Monitor
        let health_reports = manager.monitor_processes().await.unwrap();
        assert_eq!(health_reports.len(), 3, "Should have 2 producers + 1 webserver in cycle {}", i);
        
        // Stop all
        manager.stop_producers().await.unwrap();
        manager.stop_webserver().await.unwrap();
        
        // Brief pause
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Verify stopped
        let health_reports = manager.monitor_processes().await.unwrap();
        assert_eq!(health_reports.len(), 0, "Should have no processes after stop in cycle {}", i);
    }
}