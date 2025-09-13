//! Comprehensive tests for RealMessageTransport service
//!
//! These tests verify the critical communication coordination functionality
//! including channel management, message processing, and error handling.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

use shared::{ProducerId, SystemMetrics};
use crate::services::communication::RealMessageTransport;
use crate::traits::{MessageTransport, ProducerHandle, WebServerHandle};

/// Test basic producer channel establishment and message processing
#[tokio::test]
async fn test_establish_producer_channels() {
    let transport = RealMessageTransport::new();
    
    // Create test producer channels
    let (tx1, rx1) = mpsc::channel(10);
    let (tx2, rx2) = mpsc::channel(10);
    
    let producer1 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let producer2 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440002").unwrap();
    
    let handles = vec![
        ProducerHandle { id: producer1.clone(), inbound: rx1 },
        ProducerHandle { id: producer2.clone(), inbound: rx2 },
    ];
    
    // Establish channels
    let result = transport.establish_producer_channels(handles).await;
    assert!(result.is_ok(), "Should establish producer channels successfully");
    
    // Send messages from producers
    tx1.send(vec!["message1".to_string(), "message2".to_string()]).await.unwrap();
    tx2.send(vec!["message3".to_string()]).await.unwrap();
    
    // Process messages
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 2, "Should receive messages from both producers");
    
    // Verify message content (order may vary)
    let messages: Vec<String> = batches.into_iter()
        .flat_map(|(_, batch)| batch)
        .collect();
    assert!(messages.contains(&"message1".to_string()));
    assert!(messages.contains(&"message2".to_string()));
    assert!(messages.contains(&"message3".to_string()));
}

/// Test webserver channel establishment and metrics sending
#[tokio::test]
async fn test_webserver_channel_communication() {
    let transport = RealMessageTransport::new();
    
    // Create webserver channel
    let (tx, mut rx) = mpsc::channel(10);
    let webserver_handle = WebServerHandle { outbound: tx };
    
    // Establish webserver channel
    let result = transport.establish_webserver_channel(webserver_handle).await;
    assert!(result.is_ok(), "Should establish webserver channel successfully");
    
    // Create test metrics
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 10.5,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test_topic".to_string()),
        active_producers: 2,
        uptime_seconds: 300,
        last_updated: 1234567890,
    };
    
    // Send metrics update
    let result = transport.send_updates(metrics.clone()).await;
    assert!(result.is_ok(), "Should send metrics update successfully");
    
    // Verify metrics received
    let received = timeout(Duration::from_millis(100), rx.recv()).await;
    assert!(received.is_ok(), "Should receive metrics within timeout");
    let received_metrics = received.unwrap().unwrap();
    assert_eq!(received_metrics.total_unique_entries, 100);
    assert_eq!(received_metrics.current_topic, Some("test_topic".to_string()));
}

/// Test sending metrics when no webserver channel is established
#[tokio::test]
async fn test_send_metrics_no_webserver() {
    let transport = RealMessageTransport::new();
    
    let metrics = SystemMetrics {
        total_unique_entries: 50,
        entries_per_minute: 5.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: None,
        active_producers: 1,
        uptime_seconds: 150,
        last_updated: 1234567890,
    };
    
    // Should succeed even without webserver channel (graceful degradation)
    let result = transport.send_updates(metrics).await;
    assert!(result.is_ok(), "Should handle missing webserver channel gracefully");
}

/// Test producer channel reestablishment (restart scenario)
#[tokio::test]
async fn test_reestablish_producer_channel() {
    let transport = RealMessageTransport::new();
    
    // Initial setup
    let (tx1, rx1) = mpsc::channel(10);
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    
    let initial_handle = ProducerHandle { id: producer_id.clone(), inbound: rx1 };
    transport.establish_producer_channels(vec![initial_handle]).await.unwrap();
    
    // Send initial message
    tx1.send(vec!["initial_message".to_string()]).await.unwrap();
    
    // Process and verify initial message
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1[0], "initial_message");
    
    // Simulate producer restart - create new channel
    let (tx2, rx2) = mpsc::channel(10);
    let new_handle = ProducerHandle { id: producer_id.clone(), inbound: rx2 };
    
    // Reestablish channel
    let result = transport.reestablish_producer_channel(new_handle).await;
    assert!(result.is_ok(), "Should reestablish producer channel successfully");
    
    // Send message via new channel
    tx2.send(vec!["restarted_message".to_string()]).await.unwrap();
    
    // Process and verify new message
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1[0], "restarted_message");
}

/// Test message processing with multiple batches from same producer
#[tokio::test]
async fn test_multiple_batches_from_producer() {
    let transport = RealMessageTransport::new();
    
    let (tx, rx) = mpsc::channel(10);
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    
    let handle = ProducerHandle { id: producer_id.clone(), inbound: rx };
    transport.establish_producer_channels(vec![handle]).await.unwrap();
    
    // Send multiple batches quickly
    tx.send(vec!["batch1_msg1".to_string()]).await.unwrap();
    tx.send(vec!["batch2_msg1".to_string(), "batch2_msg2".to_string()]).await.unwrap();
    tx.send(vec!["batch3_msg1".to_string()]).await.unwrap();
    
    // Process all available messages
    let batches = transport.process_messages().await.unwrap();
    
    // Should get all batches
    assert_eq!(batches.len(), 3, "Should receive all 3 batches");
    
    // Verify total message count
    let total_messages: usize = batches.iter().map(|(_, batch)| batch.len()).sum();
    assert_eq!(total_messages, 4, "Should have 4 total messages");
}

/// Test handling disconnected producer channels
#[tokio::test]
async fn test_disconnected_producer_channel() {
    let transport = RealMessageTransport::new();
    
    let (tx, rx) = mpsc::channel(10);
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    
    let handle = ProducerHandle { id: producer_id.clone(), inbound: rx };
    transport.establish_producer_channels(vec![handle]).await.unwrap();
    
    // Send a message first
    tx.send(vec!["before_disconnect".to_string()]).await.unwrap();
    
    // Drop the sender to simulate producer disconnection
    drop(tx);
    
    // Process messages - should handle disconnection gracefully
    let batches = transport.process_messages().await.unwrap();
    
    // Should still get the message that was sent before disconnection
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1[0], "before_disconnect");
    
    // Second call should handle disconnected channel without error
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should return empty batches after disconnection");
}

/// Test concurrent message processing from multiple producers
#[tokio::test]
async fn test_concurrent_message_processing() {
    let transport = RealMessageTransport::new();
    
    // Create multiple producers
    let producer_count = 5;
    let mut senders = Vec::new();
    let mut handles = Vec::new();
    
    for i in 0..producer_count {
        let (tx, rx) = mpsc::channel(10);
        let producer_id = ProducerId::from_string(&format!("550e8400-e29b-41d4-a716-44665544000{}", i)).unwrap();
        
        handles.push(ProducerHandle { id: producer_id, inbound: rx });
        senders.push(tx);
    }
    
    transport.establish_producer_channels(handles).await.unwrap();
    
    // Send messages concurrently from all producers
    let send_tasks: Vec<_> = senders.into_iter().enumerate().map(|(i, tx)| {
        tokio::spawn(async move {
            for j in 0..3 {
                let msg = format!("producer_{}_message_{}", i, j);
                tx.send(vec![msg]).await.unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await; // Small delay
            }
        })
    }).collect();
    
    // Wait for all sends to complete
    for task in send_tasks {
        task.await.unwrap();
    }
    
    // Process all messages
    tokio::time::sleep(Duration::from_millis(10)).await; // Allow messages to queue
    let batches = transport.process_messages().await.unwrap();
    
    // Should receive messages from all producers
    let total_messages: usize = batches.iter().map(|(_, batch)| batch.len()).sum();
    assert_eq!(total_messages, producer_count * 3, "Should receive all messages from all producers");
}

/// Test shutdown communication cleanup
#[tokio::test]
async fn test_shutdown_communication() {
    let transport = RealMessageTransport::new();
    
    // Set up some channels
    let (prod_tx, prod_rx) = mpsc::channel(10);
    let (ws_tx, _ws_rx) = mpsc::channel(10);
    
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let producer_handle = ProducerHandle { id: producer_id, inbound: prod_rx };
    let webserver_handle = WebServerHandle { outbound: ws_tx };
    
    transport.establish_producer_channels(vec![producer_handle]).await.unwrap();
    transport.establish_webserver_channel(webserver_handle).await.unwrap();
    
    // Send a message to verify setup
    prod_tx.send(vec!["test".to_string()]).await.unwrap();
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1);
    
    // Shutdown
    let result = transport.shutdown_communication().await;
    assert!(result.is_ok(), "Should shutdown communication successfully");
    
    // After shutdown, should get no messages
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should have no channels after shutdown");
}

/// Test webserver channel backpressure handling
#[tokio::test]
async fn test_webserver_backpressure() {
    let transport = RealMessageTransport::new();
    
    // Create small capacity channel to force backpressure
    let (tx, _rx) = mpsc::channel(1);
    let webserver_handle = WebServerHandle { outbound: tx };
    
    transport.establish_webserver_channel(webserver_handle).await.unwrap();
    
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 10.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test".to_string()),
        active_producers: 1,
        uptime_seconds: 100,
        last_updated: 1234567890,
    };
    
    // Fill the channel and cause backpressure
    transport.send_updates(metrics.clone()).await.unwrap();
    transport.send_updates(metrics.clone()).await.unwrap(); // This should cause backpressure
    
    // Should handle backpressure gracefully (implementation ignores send errors)
    let result = transport.send_updates(metrics).await;
    assert!(result.is_ok(), "Should handle backpressure gracefully");
}

/// Test process_messages with empty channels
#[tokio::test]
async fn test_process_messages_empty_channels() {
    let transport = RealMessageTransport::new();
    
    // Create channels but don't send any messages
    let (_tx1, rx1) = mpsc::channel(10);
    let (_tx2, rx2) = mpsc::channel(10);
    
    let producer1 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let producer2 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440002").unwrap();
    
    let handles = vec![
        ProducerHandle { id: producer1, inbound: rx1 },
        ProducerHandle { id: producer2, inbound: rx2 },
    ];
    
    transport.establish_producer_channels(handles).await.unwrap();
    
    // Process messages - should return empty vector
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should return empty batches when no messages available");
}

/// Test producer channel unavailable scenarios
#[tokio::test]
async fn test_producer_channel_unavailable() {
    let transport = RealMessageTransport::new();
    
    // Try to process messages when no producer channels are established
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should handle no producer channels gracefully");
    
    // Establish channels, then simulate all channels becoming unavailable
    let (tx1, rx1) = mpsc::channel(10);
    let (tx2, rx2) = mpsc::channel(10);
    
    let producer1 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let producer2 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440002").unwrap();
    
    let handles = vec![
        ProducerHandle { id: producer1.clone(), inbound: rx1 },
        ProducerHandle { id: producer2.clone(), inbound: rx2 },
    ];
    
    transport.establish_producer_channels(handles).await.unwrap();
    
    // Send messages
    tx1.send(vec!["msg1".to_string()]).await.unwrap();
    tx2.send(vec!["msg2".to_string()]).await.unwrap();
    
    // Process messages successfully
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 2, "Should receive messages from both channels");
    
    // Drop senders to make channels unavailable
    drop(tx1);
    drop(tx2);
    
    // Process again - should handle disconnected channels gracefully
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should handle all unavailable channels gracefully");
    
    // Multiple calls should continue to work without errors
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should continue handling unavailable channels");
}

/// Test webserver channel unavailable scenarios
#[tokio::test]
async fn test_webserver_channel_unavailable() {
    let transport = RealMessageTransport::new();
    
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 10.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test".to_string()),
        active_producers: 1,
        uptime_seconds: 100,
        last_updated: 1234567890,
    };
    
    // Test 1: No webserver channel established
    let result = transport.send_updates(metrics.clone()).await;
    assert!(result.is_ok(), "Should handle missing webserver channel gracefully");
    
    // Test 2: Establish channel then make it unavailable
    let (tx, rx) = mpsc::channel(10);
    let webserver_handle = WebServerHandle { outbound: tx };
    transport.establish_webserver_channel(webserver_handle).await.unwrap();
    
    // Send successfully first
    let result = transport.send_updates(metrics.clone()).await;
    assert!(result.is_ok(), "Should send successfully when channel available");
    
    // Drop receiver to make channel unavailable
    drop(rx);
    
    // Send should still succeed (ignores send errors due to backpressure handling)
    let result = transport.send_updates(metrics.clone()).await;
    assert!(result.is_ok(), "Should handle unavailable webserver channel gracefully");
    
    // Multiple sends to unavailable channel should continue working
    let result = transport.send_updates(metrics).await;
    assert!(result.is_ok(), "Should continue handling unavailable webserver channel");
}

/// Test mixed availability - some channels available, some unavailable
#[tokio::test]
async fn test_mixed_channel_availability() {
    let transport = RealMessageTransport::new();
    
    // Set up multiple producer channels
    let (tx1, rx1) = mpsc::channel(10);
    let (tx2, rx2) = mpsc::channel(10);
    let (tx3, rx3) = mpsc::channel(10);
    
    let producer1 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    let producer2 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440002").unwrap();
    let producer3 = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440003").unwrap();
    
    let handles = vec![
        ProducerHandle { id: producer1.clone(), inbound: rx1 },
        ProducerHandle { id: producer2.clone(), inbound: rx2 },
        ProducerHandle { id: producer3.clone(), inbound: rx3 },
    ];
    
    transport.establish_producer_channels(handles).await.unwrap();
    
    // Send messages from all producers
    tx1.send(vec!["msg1".to_string()]).await.unwrap();
    tx2.send(vec!["msg2".to_string()]).await.unwrap();
    tx3.send(vec!["msg3".to_string()]).await.unwrap();
    
    // All channels available
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 3, "Should receive from all available channels");
    
    // Make some channels unavailable
    drop(tx1); // Producer 1 unavailable
    drop(tx3); // Producer 3 unavailable
    // Keep tx2 available
    
    // Send from remaining available channel
    tx2.send(vec!["msg4".to_string()]).await.unwrap();
    
    // Should only receive from available channel
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1, "Should receive only from available channel");
    assert_eq!(batches[0].0, producer2, "Should receive from producer2");
    assert_eq!(batches[0].1[0], "msg4");
    
    // Make remaining channel unavailable
    drop(tx2);
    
    // Should handle all channels being unavailable
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Should handle all channels being unavailable");
}

/// Test channel availability after reestablishment
#[tokio::test]
async fn test_channel_availability_after_reestablishment() {
    let transport = RealMessageTransport::new();
    
    // Initial setup
    let (tx1, rx1) = mpsc::channel(10);
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    
    let handle = ProducerHandle { id: producer_id.clone(), inbound: rx1 };
    transport.establish_producer_channels(vec![handle]).await.unwrap();
    
    // Channel available
    tx1.send(vec!["msg1".to_string()]).await.unwrap();
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1, "Channel should be available initially");
    
    // Make channel unavailable
    drop(tx1);
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 0, "Channel should be unavailable after disconnect");
    
    // Reestablish channel (simulate producer restart)
    let (tx2, rx2) = mpsc::channel(10);
    let new_handle = ProducerHandle { id: producer_id.clone(), inbound: rx2 };
    transport.reestablish_producer_channel(new_handle).await.unwrap();
    
    // Channel available again
    tx2.send(vec!["msg2".to_string()]).await.unwrap();
    let batches = transport.process_messages().await.unwrap();
    assert_eq!(batches.len(), 1, "Channel should be available after reestablishment");
    assert_eq!(batches[0].1[0], "msg2");
}

/// Test webserver channel availability after reestablishment
#[tokio::test]
async fn test_webserver_channel_reestablishment() {
    let transport = RealMessageTransport::new();
    
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 10.0,
        per_llm_performance: std::collections::HashMap::new(),
        current_topic: Some("test".to_string()),
        active_producers: 1,
        uptime_seconds: 100,
        last_updated: 1234567890,
    };
    
    // Initial webserver channel
    let (tx1, mut rx1) = mpsc::channel(10);
    let webserver_handle = WebServerHandle { outbound: tx1 };
    transport.establish_webserver_channel(webserver_handle).await.unwrap();
    
    // Channel available
    transport.send_updates(metrics.clone()).await.unwrap();
    let received = timeout(Duration::from_millis(100), rx1.recv()).await;
    assert!(received.is_ok(), "Should receive metrics when channel available");
    
    // Make channel unavailable
    drop(rx1);
    
    // Should handle unavailable channel gracefully
    let result = transport.send_updates(metrics.clone()).await;
    assert!(result.is_ok(), "Should handle unavailable webserver channel gracefully");
    
    // Reestablish webserver channel
    let (tx2, mut rx2) = mpsc::channel(10);
    let new_webserver_handle = WebServerHandle { outbound: tx2 };
    transport.establish_webserver_channel(new_webserver_handle).await.unwrap();
    
    // Channel available again
    transport.send_updates(metrics).await.unwrap();
    let received = timeout(Duration::from_millis(100), rx2.recv()).await;
    assert!(received.is_ok(), "Should receive metrics after webserver channel reestablishment");
}

/// Test rapid channel availability changes
#[tokio::test]
async fn test_rapid_channel_availability_changes() {
    let transport = RealMessageTransport::new();
    
    let producer_id = ProducerId::from_string("550e8400-e29b-41d4-a716-446655440001").unwrap();
    
    // Rapidly establish and drop channels
    for i in 0..5 {
        let (tx, rx) = mpsc::channel(10);
        let handle = ProducerHandle { id: producer_id.clone(), inbound: rx };
        
        if i == 0 {
            transport.establish_producer_channels(vec![handle]).await.unwrap();
        } else {
            transport.reestablish_producer_channel(handle).await.unwrap();
        }
        
        // Send message
        tx.send(vec![format!("msg_{}", i)]).await.unwrap();
        
        // Process messages
        let batches = transport.process_messages().await.unwrap();
        assert_eq!(batches.len(), 1, "Should handle rapid channel changes");
        assert_eq!(batches[0].1[0], format!("msg_{}", i));
        
        // Drop sender to make channel unavailable
        drop(tx);
        
        // Verify channel is unavailable
        let batches = transport.process_messages().await.unwrap();
        assert_eq!(batches.len(), 0, "Channel should be unavailable after drop");
    }
}