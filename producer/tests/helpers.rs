//! Test helper utilities for producer integration tests

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Create test socket addresses for testing
pub fn create_test_addresses() -> SocketAddr {
    "127.0.0.1:6000".parse().unwrap()
}

/// Create random test socket addresses to avoid conflicts
pub fn create_random_test_address() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    let base_port = 6000 + (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() % 1000) as u16;
    
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), base_port)
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

/// Generate a unique test topic name
pub fn generate_test_topic() -> String {
    format!("test-topic-{}", 
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    )
}

/// Generate test prompt
pub fn generate_test_prompt() -> String {
    "Generate creative attributes for testing purposes".to_string()
}