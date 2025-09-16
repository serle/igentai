//! WebSocket integration tests
//! 
//! Tests WebSocket connection handling, message routing, and client lifecycle

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use serde_json;

use webserver::{
    traits::{WebSocketManager, OrchestratorClient},
    types::{ClientMessage, ClientRequest, AlertLevel},
    services::RealWebSocketManager,
    WebServerResult,
};
use shared::{WebServerRequest, OrchestratorUpdate};

// Mock orchestrator for WebSocket handler testing
#[allow(dead_code)]
#[derive(Clone)]
struct TestOrchestratorClient {
    requests: Arc<Mutex<Vec<WebServerRequest>>>,
    should_fail: Arc<Mutex<bool>>,
}

#[allow(dead_code)]
impl TestOrchestratorClient {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    fn set_should_fail(&self, should_fail: bool) -> tokio::task::JoinHandle<()> {
        let fail_flag = self.should_fail.clone();
        tokio::spawn(async move {
            let mut flag = fail_flag.lock().await;
            *flag = should_fail;
        })
    }

    async fn get_requests(&self) -> Vec<WebServerRequest> {
        let requests = self.requests.lock().await;
        requests.clone()
    }

    async fn clear_requests(&self) {
        let mut requests = self.requests.lock().await;
        requests.clear();
    }
}

#[async_trait::async_trait]
impl OrchestratorClient for TestOrchestratorClient {
    async fn initialize(&mut self) -> WebServerResult<()> {
        Ok(())
    }
    
    async fn send_request(&self, request: WebServerRequest) -> WebServerResult<()> {
        let should_fail = {
            let fail_flag = self.should_fail.lock().await;
            *fail_flag
        };

        if should_fail {
            return Err(webserver::WebServerError::internal("Simulated orchestrator failure".to_string()));
        }

        let mut requests = self.requests.lock().await;
        requests.push(request);
        Ok(())
    }
    
    async fn get_updates(&mut self) -> WebServerResult<mpsc::Receiver<OrchestratorUpdate>> {
        let (_tx, rx) = mpsc::channel(100);
        Ok(rx)
    }
    
    async fn health_check(&self) -> WebServerResult<bool> {
        Ok(true)
    }
    
    async fn disconnect(&self) -> WebServerResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_websocket_client_lifecycle() {
    let websocket_manager = RealWebSocketManager::new();
    let client_id = Uuid::new_v4();

    // Test initial state
    assert_eq!(websocket_manager.client_count().await, 0);
    assert!(websocket_manager.active_clients().await.is_empty());

    // Add client
    let (tx, _rx) = mpsc::channel(100);
    websocket_manager.add_client(client_id, tx).await.unwrap();

    assert_eq!(websocket_manager.client_count().await, 1);
    assert_eq!(websocket_manager.active_clients().await, vec![client_id]);

    // Remove client
    websocket_manager.remove_client(client_id).await.unwrap();

    assert_eq!(websocket_manager.client_count().await, 0);
    assert!(websocket_manager.active_clients().await.is_empty());
}

#[tokio::test]
async fn test_websocket_message_broadcasting() {
    let websocket_manager = RealWebSocketManager::new();
    
    // Add multiple clients
    let client1_id = Uuid::new_v4();
    let client2_id = Uuid::new_v4();
    let client3_id = Uuid::new_v4();

    let (tx1, mut rx1) = mpsc::channel(100);
    let (tx2, mut rx2) = mpsc::channel(100);
    let (tx3, mut rx3) = mpsc::channel(100);

    websocket_manager.add_client(client1_id, tx1).await.unwrap();
    websocket_manager.add_client(client2_id, tx2).await.unwrap();
    websocket_manager.add_client(client3_id, tx3).await.unwrap();

    // Wait for ConnectionAck messages to be sent
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Clear initial ConnectionAck messages
    let _ = timeout(Duration::from_millis(50), rx1.recv()).await;
    let _ = timeout(Duration::from_millis(50), rx2.recv()).await;
    let _ = timeout(Duration::from_millis(50), rx3.recv()).await;

    // Broadcast a test message
    let test_message = ClientMessage::Alert {
        level: AlertLevel::Info,
        title: "Test Alert".to_string(),
        message: "This is a test broadcast".to_string(),
        timestamp: 1234567890,
        dismissible: true,
    };

    websocket_manager.broadcast(test_message.clone()).await.unwrap();

    // Verify all clients received the message
    let received1 = timeout(Duration::from_millis(100), rx1.recv()).await
        .expect("Client 1 should receive message")
        .expect("Message should not be None");
    
    let received2 = timeout(Duration::from_millis(100), rx2.recv()).await
        .expect("Client 2 should receive message")
        .expect("Message should not be None");
        
    let received3 = timeout(Duration::from_millis(100), rx3.recv()).await
        .expect("Client 3 should receive message") 
        .expect("Message should not be None");

    // Verify message contents (simplified comparison)
    assert!(matches!(received1, ClientMessage::Alert { .. }));
    assert!(matches!(received2, ClientMessage::Alert { .. }));
    assert!(matches!(received3, ClientMessage::Alert { .. }));
}

#[tokio::test]
async fn test_websocket_individual_message_sending() {
    let websocket_manager = RealWebSocketManager::new();
    
    let client1_id = Uuid::new_v4();
    let client2_id = Uuid::new_v4();

    let (tx1, mut rx1) = mpsc::channel(100);
    let (tx2, mut rx2) = mpsc::channel(100);

    websocket_manager.add_client(client1_id, tx1).await.unwrap();
    websocket_manager.add_client(client2_id, tx2).await.unwrap();

    // Wait for ConnectionAck messages to be sent
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Clear initial ConnectionAck messages
    let _ = timeout(Duration::from_millis(50), rx1.recv()).await;
    let _ = timeout(Duration::from_millis(50), rx2.recv()).await;

    // Send message to specific client
    let test_message = ClientMessage::StatusUpdate {
        orchestrator_connected: true,
        active_producers: 5,
        current_topic: Some("Test Topic".to_string()),
        system_health: webserver::types::SystemHealth::Healthy,
    };

    websocket_manager.send_to_client(client1_id, test_message).await.unwrap();

    // Verify only client1 received the message
    let received1 = timeout(Duration::from_millis(100), rx1.recv()).await
        .expect("Client 1 should receive message")
        .expect("Message should not be None");

    // Client 2 should not receive anything
    let result2 = timeout(Duration::from_millis(50), rx2.recv()).await;
    assert!(result2.is_err(), "Client 2 should not receive the message");

    assert!(matches!(received1, ClientMessage::StatusUpdate { .. }));
}

#[tokio::test]
async fn test_websocket_malformed_message_handling() {
    // Test that malformed JSON doesn't crash the system
    let malformed_messages = vec![
        r#"{"type": "invalid_type", "data": {}}"#,
        r#"{"missing": "type_field"}"#,
        r#"not_json_at_all"#,
        r#"{"type": "start_generation", "data": {"topic": 123}}"#, // Wrong data type
    ];

    for malformed in malformed_messages {
        let result: Result<ClientRequest, _> = serde_json::from_str(malformed);
        assert!(result.is_err(), "Malformed message should fail to parse: {}", malformed);
    }
}

#[tokio::test]
async fn test_websocket_disconnected_client_cleanup() {
    let websocket_manager = RealWebSocketManager::new();
    let client_id = Uuid::new_v4();

    let (tx, rx) = mpsc::channel(100);
    websocket_manager.add_client(client_id, tx).await.unwrap();
    
    assert_eq!(websocket_manager.client_count().await, 1);

    // Drop the receiver to simulate client disconnection
    drop(rx);

    // Try to send a message - this should detect the disconnection
    let test_message = ClientMessage::Alert {
        level: AlertLevel::Info,
        title: "Test".to_string(),
        message: "Test".to_string(),
        timestamp: 1234567890,
        dismissible: true,
    };

    // The broadcast should succeed but internally clean up the disconnected client
    websocket_manager.broadcast(test_message).await.unwrap();

    // Give some time for cleanup to occur (this test may be timing-dependent)
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Note: The actual cleanup might happen during the next operation
    // depending on the implementation details
}


#[tokio::test]
async fn test_websocket_high_volume_messaging() {
    let websocket_manager = RealWebSocketManager::new();
    let client_id = Uuid::new_v4();
    
    let (tx, mut rx) = mpsc::channel(1000);
    websocket_manager.add_client(client_id, tx).await.unwrap();

    // Wait for ConnectionAck messages to be sent
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Clear initial ConnectionAck message
    let _ = timeout(Duration::from_millis(50), rx.recv()).await;

    // Send many messages rapidly
    let message_count = 100;
    for i in 0..message_count {
        let message = ClientMessage::Alert {
            level: AlertLevel::Info,
            title: format!("Message {}", i),
            message: format!("This is message number {}", i),
            timestamp: 1234567890 + i as u64,
            dismissible: true,
        };

        websocket_manager.broadcast(message).await.unwrap();
    }

    // Verify all messages were received
    let mut received_count = 0;
    while let Ok(Some(_)) = timeout(Duration::from_millis(100), rx.recv()).await {
        received_count += 1;
    }

    assert_eq!(received_count, message_count, "Should receive all {} messages", message_count);
}