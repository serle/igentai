//! Orchestrator communication integration tests
//!
//! Tests IPC protocol, message handling, and error scenarios

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use shared::{OrchestratorUpdate, WebServerRequest};
use webserver::{
    WebServerResult, core::WebServerState, services::RealOrchestratorClient, traits::OrchestratorClient,
    types::ClientMessage,
};

#[tokio::test]
async fn test_orchestrator_client_listener_failure() {
    // Try to use an already occupied port or invalid bind address
    // First bind to a port, then try to bind to the same port again
    let test_addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let _listener = tokio::net::TcpListener::bind(test_addr).await.unwrap();
    let occupied_addr = _listener.local_addr().unwrap();

    let orchestrator_addr = "127.0.0.1:9999".parse().unwrap();

    let mut orchestrator_client = RealOrchestratorClient::new(occupied_addr, orchestrator_addr);

    // Initialize should fail due to occupied address
    let result = orchestrator_client.initialize().await;
    assert!(result.is_err(), "Initialize with occupied address should fail");
}

#[tokio::test]
async fn test_orchestrator_client_health_check() {
    let api_addr = "127.0.0.1:0".parse().unwrap(); // Use ephemeral port
    let orchestrator_addr = "127.0.0.1:8890".parse().unwrap();
    let mut orchestrator_client = RealOrchestratorClient::new(api_addr, orchestrator_addr);

    // Initialize the listener
    orchestrator_client.initialize().await.unwrap();

    // Initially, health check should return false (no orchestrator connected)
    let is_healthy = orchestrator_client.health_check().await.unwrap();
    assert!(
        !is_healthy,
        "Health check should return false when orchestrator hasn't connected yet"
    );

    // The health check only returns true when the orchestrator connects to the webserver
    // This test validates the initial state
}

#[tokio::test]
async fn test_orchestrator_update_error_handling() {
    let mut state = WebServerState::new();

    // Test error notification
    let error_update = OrchestratorUpdate::ErrorNotification("Producer failed to start".to_string());

    let client_messages = state.process_orchestrator_update(error_update);

    // Should generate alert message
    assert!(!client_messages.is_empty());
    assert!(matches!(client_messages[0], ClientMessage::Alert { .. }));

    if let ClientMessage::Alert {
        level, title, message, ..
    } = &client_messages[0]
    {
        assert!(matches!(level, webserver::types::AlertLevel::Error));
        assert_eq!(title, "System Error");
        assert_eq!(message, "Producer failed to start");
    }
}

// Helper to create a mock orchestrator client for testing
#[allow(dead_code)]
fn create_mock_orchestrator_client() -> TestOrchestratorClient {
    TestOrchestratorClient::new()
}

#[derive(Clone)]
#[allow(dead_code)]
struct TestOrchestratorClient {
    requests: Arc<Mutex<Vec<WebServerRequest>>>,
}

#[allow(dead_code)]
impl TestOrchestratorClient {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn get_requests(&self) -> Vec<WebServerRequest> {
        let requests = self.requests.lock().await;
        requests.clone()
    }
}

#[async_trait::async_trait]
impl OrchestratorClient for TestOrchestratorClient {
    async fn initialize(&mut self) -> WebServerResult<()> {
        Ok(())
    }

    async fn send_request(&self, request: WebServerRequest) -> WebServerResult<()> {
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
