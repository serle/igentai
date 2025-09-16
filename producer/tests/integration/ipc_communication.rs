//! IPC communication tests between orchestrator and producer

use crate::fixtures::{CommandFactory, MockOrchestrator, ProcessIdFactory, UpdateFactory};
use shared::messages::producer::ProducerSyncStatus;
use shared::{ProcessStatus, ProducerCommand, ProducerUpdate};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Test basic message serialization and deserialization
#[tokio::test]
async fn test_message_serialization() {
    // Test all command types
    let commands = vec![
        CommandFactory::start_command(1, "test topic", "test prompt"),
        CommandFactory::stop_command(2),
        CommandFactory::update_config_command(3, Some("updated prompt".to_string())),
        CommandFactory::sync_check_command(4, Some(vec![0u8; 1024]), true),
        CommandFactory::ping_command(5),
    ];

    for command in commands {
        // Serialize and deserialize
        let serialized = bincode::serialize(&command).unwrap();
        let deserialized: ProducerCommand = bincode::deserialize(&serialized).unwrap();

        // Commands should be equal (we can't directly compare due to complex types)
        // Instead, verify they serialize to the same bytes
        let reserialized = bincode::serialize(&deserialized).unwrap();
        assert_eq!(serialized, reserialized);
    }

    // Test all update types
    let producer_id = ProcessIdFactory::create();
    let updates = vec![
        UpdateFactory::attribute_batch(
            producer_id.clone(),
            1,
            vec!["attr1".to_string(), "attr2".to_string()],
            shared::types::ProviderId::OpenAI,
        ),
        UpdateFactory::status_update(
            producer_id.clone(),
            ProcessStatus::Running,
            Some("Test status".to_string()),
            true,
        ),
        UpdateFactory::sync_ack(producer_id.clone(), 4, ProducerSyncStatus::Ready),
        UpdateFactory::pong(producer_id.clone(), 5),
        UpdateFactory::error(producer_id.clone(), "TEST_ERROR", "Test error message", Some(1)),
    ];

    for update in updates {
        let serialized = bincode::serialize(&update).unwrap();
        let deserialized: ProducerUpdate = bincode::deserialize(&serialized).unwrap();
        let reserialized = bincode::serialize(&deserialized).unwrap();
        assert_eq!(serialized, reserialized);
    }
}

/// Test TCP connection establishment and message exchange
#[tokio::test]
async fn test_tcp_message_exchange() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let listener = TcpListener::bind(addr).await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Spawn server task
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();

        // Read command
        let mut buffer = vec![0; 4096];
        let n = stream.read(&mut buffer).await.unwrap();
        let command: ProducerCommand = bincode::deserialize(&buffer[..n]).unwrap();

        // Send response
        let producer_id = ProcessIdFactory::create();
        let update = UpdateFactory::status_update(
            producer_id,
            ProcessStatus::Running,
            Some("Test response".to_string()),
            false,
        );
        let serialized = bincode::serialize(&update).unwrap();
        stream.write_all(&serialized).await.unwrap();

        command
    });

    // Client connection
    let mut stream = TcpStream::connect(server_addr).await.unwrap();

    // Send command
    let command = CommandFactory::start_command(1, "test topic", "test prompt");
    let serialized = bincode::serialize(&command).unwrap();
    stream.write_all(&serialized).await.unwrap();

    // Read response
    let mut buffer = vec![0; 4096];
    let n = stream.read(&mut buffer).await.unwrap();
    let update: ProducerUpdate = bincode::deserialize(&buffer[..n]).unwrap();

    // Verify response
    match update {
        ProducerUpdate::StatusUpdate { status, message, .. } => {
            assert_eq!(status, ProcessStatus::Running);
            assert_eq!(message, Some("Test response".to_string()));
        }
        _ => panic!("Expected StatusUpdate"),
    }

    // Wait for server to complete
    let received_command = server_task.await.unwrap();
    match received_command {
        ProducerCommand::Start { command_id, topic, .. } => {
            assert_eq!(command_id, 1);
            assert_eq!(topic, "test topic");
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test message size limits and large message handling
#[tokio::test]
async fn test_large_message_handling() {
    // Create a large attribute batch
    let producer_id = ProcessIdFactory::create();
    let large_attributes: Vec<String> = (0..1000)
        .map(|i| format!("Large attribute number {} with extra padding text", i))
        .collect();

    let large_update =
        UpdateFactory::attribute_batch(producer_id, 1, large_attributes, shared::types::ProviderId::OpenAI);

    // Should be able to serialize/deserialize large messages
    let serialized = bincode::serialize(&large_update).unwrap();
    assert!(serialized.len() > 10000); // Ensure it's actually large

    let deserialized: ProducerUpdate = bincode::deserialize(&serialized).unwrap();

    match deserialized {
        ProducerUpdate::AttributeBatch { attributes, .. } => {
            assert_eq!(attributes.len(), 1000);
        }
        _ => panic!("Expected AttributeBatch"),
    }
}

/// Test connection error scenarios
#[tokio::test]
async fn test_connection_failures() {
    // Try to connect to non-existent server
    let invalid_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);
    let result = tokio::time::timeout(Duration::from_millis(100), TcpStream::connect(invalid_addr)).await;

    // Should timeout or fail to connect
    assert!(result.is_err() || result.unwrap().is_err());
}

/// Test concurrent connections
#[tokio::test]
async fn test_concurrent_connections() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let listener = TcpListener::bind(addr).await.unwrap();
    let server_addr = listener.local_addr().unwrap();

    // Spawn server that accepts multiple connections
    let server_task = tokio::spawn(async move {
        let mut connection_count = 0;

        while connection_count < 3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            connection_count += 1;

            tokio::spawn(async move {
                let mut buffer = vec![0; 4096];
                if let Ok(n) = stream.read(&mut buffer).await {
                    if let Ok(_command) = bincode::deserialize::<ProducerCommand>(&buffer[..n]) {
                        let producer_id = ProcessIdFactory::create();
                        let response = UpdateFactory::pong(producer_id, 999);
                        let serialized = bincode::serialize(&response).unwrap();
                        let _ = stream.write_all(&serialized).await;
                    }
                }
            });
        }

        connection_count
    });

    // Create multiple concurrent client connections
    let mut client_tasks = Vec::new();

    for i in 0..3 {
        let client_task = tokio::spawn(async move {
            let mut stream = TcpStream::connect(server_addr).await.unwrap();

            let command = CommandFactory::ping_command(i);
            let serialized = bincode::serialize(&command).unwrap();
            stream.write_all(&serialized).await.unwrap();

            let mut buffer = vec![0; 4096];
            let n = stream.read(&mut buffer).await.unwrap();
            let update: ProducerUpdate = bincode::deserialize(&buffer[..n]).unwrap();

            matches!(update, ProducerUpdate::Pong { .. })
        });

        client_tasks.push(client_task);
    }

    // Wait for all clients to complete
    for task in client_tasks {
        let success = task.await.unwrap();
        assert!(success);
    }

    // Wait for server
    let connection_count = server_task.await.unwrap();
    assert_eq!(connection_count, 3);
}

/// Test mock orchestrator functionality
#[tokio::test]
async fn test_mock_orchestrator() {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let mut mock_orch = MockOrchestrator::new(addr);
    let shutdown_tx = mock_orch.start().await.unwrap();

    // Give mock orchestrator time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test queueing commands
    let command = CommandFactory::ping_command(123);
    mock_orch.queue_command(command).await;

    // Test getting updates (should be empty initially)
    let updates = mock_orch.get_received_updates().await;
    assert_eq!(updates.len(), 0);

    // Shutdown
    shutdown_tx.send(()).await.unwrap();
}
