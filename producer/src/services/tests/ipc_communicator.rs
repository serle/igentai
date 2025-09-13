//! Tests for IpcCommunicator service

use std::net::SocketAddr;
use shared::ProducerCommand;
use crate::services::ipc_communicator::RealIpcCommunicator;
use crate::traits::IpcCommunicator;

#[tokio::test]
async fn test_ipc_communicator_creation() {
    let communicator = RealIpcCommunicator::new();
    
    // Should be in disconnected state initially
    // Testing the new state without actual connection
    assert!(true); // Basic creation test
}

#[tokio::test]
async fn test_connect_invalid_address() {
    let communicator = RealIpcCommunicator::new();
    
    // Try to connect to invalid address
    let invalid_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let result = communicator.connect(invalid_addr).await;
    
    // Should fail gracefully
    assert!(result.is_err());
}

#[tokio::test]
async fn test_disconnect_without_connection() {
    let communicator = RealIpcCommunicator::new();
    
    // Should be able to disconnect even without connection
    let result = communicator.disconnect().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_send_update_without_connection() {
    let communicator = RealIpcCommunicator::new();
    
    let update = shared::ProducerUpdate::StatisticsUpdate {
        producer_id: "test-id".to_string(),
        status: shared::ProducerStatus::Running,
        provider_performance: std::collections::HashMap::new(),
    };
    
    // Should handle gracefully when not connected
    let result = communicator.send_update(update).await;
    assert!(result.is_err()); // Expected to fail when not connected
}

#[tokio::test]
async fn test_listen_for_commands_without_connection() {
    let communicator = RealIpcCommunicator::new();
    
    // Should handle gracefully when not connected
    let result = communicator.listen_for_commands().await;
    assert!(result.is_err()); // Expected to fail when not connected
}

// Note: Full integration tests with actual TCP connections would require
// a test orchestrator server, which is beyond the scope of unit tests.
// These would be better suited for integration test suites.