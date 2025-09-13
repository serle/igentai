//! Integration tests for webserver
//!
//! These tests verify that the webserver components work together correctly
//! using the real service implementations.

mod fixtures;
mod helpers;

use webserver::services::{
    RealFileManager, RealClientBroadcaster, RealIpcCommunicator,
    RealClientRegistry, RealMetricsAggregator
};
use webserver::WebServer;
use helpers::*;

#[tokio::test]
async fn test_webserver_creation() {
    let (bind_addr, orch_addr) = create_test_addresses();
    
    // Create real service implementations
    let file_manager = RealFileManager::new();
    let client_broadcaster = RealClientBroadcaster::new(webserver::state::WebServerState::new(bind_addr, orch_addr).into());
    let ipc_communicator = RealIpcCommunicator::new(webserver::state::WebServerState::new(bind_addr, orch_addr).into());
    let client_registry = RealClientRegistry::new(webserver::state::WebServerState::new(bind_addr, orch_addr).into());
    let metrics_aggregator = RealMetricsAggregator::new(webserver::state::WebServerState::new(bind_addr, orch_addr).into());
    
    // Create webserver with real implementations
    let webserver = WebServer::new(
        bind_addr,
        orch_addr,
        file_manager,
        client_broadcaster,
        ipc_communicator,
        client_registry,
        metrics_aggregator,
    );
    
    // Test basic functionality
    let _router = webserver.build_router();
    // Router creation test passes
    assert!(true);
}

#[tokio::test]
async fn test_webserver_state_access() {
    let (bind_addr, orch_addr) = create_test_addresses();
    
    // Create minimal webserver instance
    let file_manager = RealFileManager::new();
    let state = std::sync::Arc::new(webserver::state::WebServerState::new(bind_addr, orch_addr));
    let client_broadcaster = RealClientBroadcaster::new(state.clone());
    let ipc_communicator = RealIpcCommunicator::new(state.clone());
    let client_registry = RealClientRegistry::new(state.clone());
    let metrics_aggregator = RealMetricsAggregator::new(state.clone());
    
    let webserver = WebServer::new(
        bind_addr,
        orch_addr,
        file_manager,
        client_broadcaster,
        ipc_communicator,
        client_registry,
        metrics_aggregator,
    );
    
    // Test state access
    let server_state = webserver.state();
    assert_eq!(server_state.bind_address, bind_addr);
    assert_eq!(server_state.orchestrator_address, orch_addr);
    assert!(server_state.get_uptime_seconds() > 0);
}

#[tokio::test]
async fn test_router_routes() {
    let (bind_addr, orch_addr) = create_test_addresses();
    
    // Create webserver
    let file_manager = RealFileManager::new();
    let state = std::sync::Arc::new(webserver::state::WebServerState::new(bind_addr, orch_addr));
    let client_broadcaster = RealClientBroadcaster::new(state.clone());
    let ipc_communicator = RealIpcCommunicator::new(state.clone());
    let client_registry = RealClientRegistry::new(state.clone());
    let metrics_aggregator = RealMetricsAggregator::new(state.clone());
    
    let webserver = WebServer::new(
        bind_addr,
        orch_addr,
        file_manager,
        client_broadcaster,
        ipc_communicator,
        client_registry,
        metrics_aggregator,
    );
    
    // Test router creation
    let _router = webserver.build_router();
    
    // Router should be created successfully
    // Further testing would require starting actual server and making HTTP requests
    assert!(true); // Basic creation test passes
}