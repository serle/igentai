//! Test helper utilities for webserver integration tests

use std::net::SocketAddr;

/// Create test socket addresses for testing
pub fn create_test_addresses() -> (SocketAddr, SocketAddr) {
    let bind_addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    let orch_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    (bind_addr, orch_addr)
}

/// Create random test socket addresses to avoid conflicts
pub fn create_random_test_addresses() -> (SocketAddr, SocketAddr) {
    use std::net::{IpAddr, Ipv4Addr};
    let base_port = 3000 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() % 1000) as u16;
    
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), base_port);
    let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), base_port + 1);
    
    (bind_addr, orch_addr)
}

/// Helper to wait for async conditions with timeout
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