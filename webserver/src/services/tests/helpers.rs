//! Test helpers for webserver service tests

use crate::state::WebServerState;
use std::net::SocketAddr;
use std::sync::Arc;

/// Create a test webserver state for testing
pub fn create_test_state() -> Arc<WebServerState> {
    let bind_addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    let orch_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    Arc::new(WebServerState::new(bind_addr, orch_addr))
}

/// Create a test webserver state with custom addresses
pub fn create_test_state_with_addresses(bind_addr: SocketAddr, orch_addr: SocketAddr) -> Arc<WebServerState> {
    Arc::new(WebServerState::new(bind_addr, orch_addr))
}

/// Helper to create random socket addresses for testing
pub fn create_random_socket_addr() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    let port = 3000 + (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() % 1000) as u16;
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
}

/// Test helper to wait for async operations
pub async fn wait_for_condition<F, Fut>(mut condition: F, timeout_ms: u64) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);
    
    loop {
        if condition().await {
            return true;
        }
        
        if start.elapsed() > timeout {
            return false;
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}