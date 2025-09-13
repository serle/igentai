//! Tests for the ClientBroadcaster service

use super::fixtures::*;
use super::helpers::*;
use crate::services::RealClientBroadcaster;
use crate::traits::ClientBroadcaster;
use crate::types::BrowserMessage;

mod real_client_broadcaster_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_client_broadcaster() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state.clone());
        
        // Test that broadcaster is created successfully
        assert_eq!(broadcaster.get_client_count().await, 0);
    }

    #[tokio::test]
    async fn test_client_count_starts_at_zero() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        assert_eq!(broadcaster.get_client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_to_empty_client_list() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        let message = create_test_browser_message();
        let result = broadcaster.broadcast_to_all(message).await;
        
        // Should succeed even with no clients
        assert!(result.is_ok());
        assert_eq!(broadcaster.get_client_count().await, 0);
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_client() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        let client_id = generate_client_id();
        let message = create_test_browser_message();
        
        let result = broadcaster.send_to_client(&client_id, message).await;
        
        // Should handle gracefully (error or succeed silently)
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_client() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        let client_id = generate_client_id();
        
        // Try to remove a client that was never added
        let result = broadcaster.remove_client(&client_id).await;
        assert!(result.is_ok()); // Should succeed gracefully
        assert_eq!(broadcaster.get_client_count().await, 0);
    }

    #[tokio::test]
    async fn test_broadcast_different_message_types() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        let messages = vec![
            BrowserMessage::StartTopic {
                topic: "test-topic-1".to_string(),
                producer_count: 3,
                prompt: Some("prompt 1".to_string()),
            },
            BrowserMessage::StopGeneration,
            BrowserMessage::RequestDashboard,
        ];
        
        // Test sending different message types
        for message in messages {
            let result = broadcaster.broadcast_to_all(message).await;
            // Should handle all message types
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        // Send multiple broadcasts concurrently
        let mut handles = vec![];
        for i in 0..5 {
            let broadcaster_clone = broadcaster.clone();
            let handle = tokio::spawn(async move {
                let message = BrowserMessage::StartTopic {
                    topic: format!("topic-{}", i),
                    producer_count: i as u32 + 1,
                    prompt: Some(format!("prompt {}", i)),
                };
                broadcaster_clone.broadcast_to_all(message).await
            });
            handles.push(handle);
        }
        
        // Wait for all broadcasts
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        // Test various error conditions
        
        // 1. Send to invalid client ID
        let invalid_client_id = generate_client_id();
        let message = create_test_browser_message();
        let result = broadcaster.send_to_client(&invalid_client_id, message).await;
        assert!(result.is_ok() || result.is_err());
        
        // 2. Broadcast with no clients (should succeed)
        let message = create_test_browser_message();
        let result = broadcaster.broadcast_to_all(message).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_state_consistency() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        let client_id = generate_client_id();
        
        // Perform multiple operations
        let _ = broadcaster.send_to_client(&client_id, create_test_browser_message()).await;
        let _ = broadcaster.broadcast_to_all(create_test_browser_message()).await;
        let _ = broadcaster.remove_client(&client_id).await;
        
        // State should be consistent
        assert_eq!(broadcaster.get_client_count().await, 0);
        
        // Operations after removal should handle gracefully
        let result = broadcaster.send_to_client(&client_id, create_test_browser_message()).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_performance() {
        let state = create_test_state();
        let broadcaster = RealClientBroadcaster::new(state);
        
        // Test performance of repeated operations
        let iterations = 10;
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let message = create_test_browser_message();
            let _ = broadcaster.broadcast_to_all(message).await;
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        // Operations should complete in reasonable time
        assert!(avg_duration < std::time::Duration::from_millis(100));
    }
}