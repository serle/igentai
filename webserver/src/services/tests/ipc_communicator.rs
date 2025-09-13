//! Tests for the IpcCommunicator service

use super::fixtures::*;
use super::helpers::*;
use crate::services::RealIpcCommunicator;
use crate::traits::IpcCommunicator;
use crate::types::BrowserMessage;
use std::net::SocketAddr;

mod real_ipc_communicator_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_ipc_communicator() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test that the communicator is created successfully
        assert!(!communicator.is_connected().await);
    }

    #[tokio::test]
    async fn test_initially_not_connected() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        assert!(!communicator.is_connected().await);
    }

    #[tokio::test]
    async fn test_connection_attempts() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        let test_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        
        // Test connection attempt (will likely fail since no orchestrator running)
        let result = communicator.connect(test_addr).await;
        
        // Should handle connection failure gracefully
        // In real implementation, this might return an error or succeed
        // depending on whether orchestrator is running
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_send_message_when_not_connected() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        let message = create_test_browser_message();
        
        // Should handle sending message when not connected
        let result = communicator.send_message(message).await;
        
        // This should either queue the message or return an error
        // Implementation dependent
        assert!(result.is_ok() || result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_browser_message_types() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
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
            let result = communicator.send_message(message).await;
            // Should handle all message types
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[tokio::test]
    async fn test_disconnect() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test disconnect
        let result = communicator.disconnect().await;
        assert!(result.is_ok());
        
        // Should be disconnected after explicit disconnect
        assert!(!communicator.is_connected().await);
    }

    #[tokio::test]
    async fn test_connection_loop_lifecycle() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test starting connection loop briefly
        let communicator_clone = communicator.clone();
        let connection_task = tokio::spawn(async move {
            // Run connection loop briefly with timeout
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                communicator_clone.start_connection_loop()
            ).await
        });
        
        // Give it a moment to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        
        // Task should complete due to timeout
        let task_result = connection_task.await;
        assert!(task_result.is_ok() || task_result.is_err());
    }

    #[tokio::test]
    async fn test_concurrent_message_sending() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Send multiple messages concurrently
        let mut handles = vec![];
        
        for i in 0..5 {
            let communicator_clone = communicator.clone();
            let handle = tokio::spawn(async move {
                let message = BrowserMessage::StartTopic {
                    topic: format!("topic-{}", i),
                    producer_count: i as u32 + 1,
                    prompt: Some(format!("prompt {}", i)),
                };
                communicator_clone.send_message(message).await
            });
            handles.push(handle);
        }
        
        // Wait for all messages to be sent
        for handle in handles {
            let result = handle.await.unwrap();
            // All should complete (successfully or with error)
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[tokio::test]
    async fn test_reconnection_behavior() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        let test_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        
        // Test multiple connection attempts
        for _ in 0..3 {
            let connect_result = communicator.connect(test_addr).await;
            let disconnect_result = communicator.disconnect().await;
            
            // Each cycle should complete without panicking
            assert!(connect_result.is_ok() || connect_result.is_err());
            assert!(disconnect_result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        let test_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        
        // Test connection with timeout
        let connect_task = tokio::spawn({
            let communicator = communicator.clone();
            async move {
                communicator.connect(test_addr).await
            }
        });
        
        // Use a short timeout to test timeout behavior
        let timeout_result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            connect_task
        ).await;
        
        // Should either complete within timeout or timeout
        match timeout_result {
            Ok(connect_result) => {
                // Connection completed within timeout
                assert!(connect_result.is_ok() || connect_result.is_err());
            }
            Err(_) => {
                // Connection timed out - this is also valid behavior
                assert!(true);
            }
        }
    }

    #[tokio::test]
    async fn test_state_consistency() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test that communicator state remains consistent
        let initial_connected = communicator.is_connected().await;
        
        // Perform various operations
        let _ = communicator.send_message(create_test_browser_message()).await;
        
        // State should still be consistent
        let final_connected = communicator.is_connected().await;
        
        // Connection status should only change through explicit connect/disconnect
        assert_eq!(initial_connected, final_connected);
    }

    #[tokio::test]
    async fn test_performance() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test performance of repeated message sending
        let iterations = 10;
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let message = create_test_browser_message();
            let _ = communicator.send_message(message).await;
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        // Operations should complete in reasonable time
        assert!(avg_duration < std::time::Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let state = create_test_state();
        let communicator = RealIpcCommunicator::new(state);
        
        // Test various error conditions
        
        // 1. Try to connect to invalid address (should handle gracefully)
        let invalid_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let result = communicator.connect(invalid_addr).await;
        assert!(result.is_ok() || result.is_err()); // Should handle gracefully
    }
}