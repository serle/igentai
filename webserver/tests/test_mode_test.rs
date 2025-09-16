//! Test mode functionality tests
//! 
//! Tests that the webserver can start and run in test mode without an orchestrator

use std::time::Duration;
use tokio::time::timeout;

use webserver::{
    core::{WebServerState, AnalyticsEngine},
    services::{
        RealOrchestratorClient,
        RealWebSocketManager,
        RealStaticFileServer,
    },
    WebServer,
};

#[tokio::test]
async fn test_webserver_starts_in_test_mode() {
    // Create services with dummy addresses (won't be used in test mode)
    let api_addr = "127.0.0.1:0".parse().unwrap();
    let orchestrator_addr = "127.0.0.1:9999".parse().unwrap(); // Non-existent
    
    let orchestrator_client = RealOrchestratorClient::new(api_addr, orchestrator_addr);
    let websocket_manager = RealWebSocketManager::new();
    let static_server = RealStaticFileServer::new("./static".to_string());
    
    let state = WebServerState::new();
    let analytics = AnalyticsEngine::new();
    
    let mut webserver = WebServer::new(
        state,
        analytics,
        orchestrator_client,
        websocket_manager,
        static_server,
    );
    
    // Test that webserver starts in test mode without crashing
    let http_addr = "127.0.0.1:0".parse().unwrap();
    
    // Start webserver in test mode in a separate task
    let shutdown_sender = webserver.get_shutdown_sender();
    let webserver_task = tokio::spawn(async move {
        webserver.run(http_addr, true).await // test_mode = true
    });
    
    // Give it a moment to start up
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Send shutdown signal
    shutdown_sender.send(()).await.unwrap();
    
    // Verify that webserver shuts down gracefully
    let result = timeout(Duration::from_secs(5), webserver_task).await;
    assert!(result.is_ok(), "Webserver should shut down gracefully");
    
    let webserver_result = result.unwrap();
    assert!(webserver_result.is_ok(), "Webserver should not return an error in test mode");
}

#[tokio::test]
async fn test_webserver_fails_without_test_mode_and_no_orchestrator() {
    // Create services with non-existent orchestrator
    let api_addr = "127.0.0.1:0".parse().unwrap();
    let orchestrator_addr = "127.0.0.1:9999".parse().unwrap(); // Non-existent
    
    let orchestrator_client = RealOrchestratorClient::new(api_addr, orchestrator_addr);
    let websocket_manager = RealWebSocketManager::new();
    let static_server = RealStaticFileServer::new("./static".to_string());
    
    let state = WebServerState::new();
    let analytics = AnalyticsEngine::new();
    
    let mut webserver = WebServer::new(
        state,
        analytics,
        orchestrator_client,
        websocket_manager,
        static_server,
    );
    
    // Test that webserver continues in offline mode (should not crash)
    let http_addr = "127.0.0.1:0".parse().unwrap();
    
    // Start webserver in normal mode (should gracefully handle orchestrator connection failure)
    let shutdown_sender = webserver.get_shutdown_sender();
    let webserver_task = tokio::spawn(async move {
        webserver.run(http_addr, false).await // test_mode = false
    });
    
    // Give it a moment to start up and fail to connect to orchestrator
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Send shutdown signal
    shutdown_sender.send(()).await.unwrap();
    
    // Verify that webserver shuts down gracefully even when orchestrator is unavailable
    let result = timeout(Duration::from_secs(5), webserver_task).await;
    assert!(result.is_ok(), "Webserver should shut down gracefully even in offline mode");
    
    let webserver_result = result.unwrap();
    assert!(webserver_result.is_ok(), "Webserver should continue in offline mode without crashing");
}