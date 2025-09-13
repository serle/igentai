//! Service-specific tests
//!
//! This module contains comprehensive tests for all orchestrator services.
//! Each service has its own test file with dedicated fixtures and helpers.

#[cfg(test)]
mod api_keys;
#[cfg(test)]
mod communication;
#[cfg(test)]
mod file_system;
#[cfg(test)]
mod process_management;

// Common test utilities for services
#[cfg(test)]
pub mod common {
    use std::time::Duration;
    use tokio::time::timeout;
    
    /// Standard timeout for async operations in tests
    pub const TEST_TIMEOUT: Duration = Duration::from_millis(100);
    
    /// Helper to run async operations with timeout
    pub async fn with_timeout<T, F>(future: F) -> Result<T, tokio::time::error::Elapsed>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(TEST_TIMEOUT, future).await
    }
    
    /// Generate test producer IDs
    pub fn test_producer_id(suffix: &str) -> shared::ProducerId {
        shared::ProducerId::from_string(&format!("550e8400-e29b-41d4-a716-44665544{:0>4}", suffix))
            .expect("Valid test producer ID")
    }
}