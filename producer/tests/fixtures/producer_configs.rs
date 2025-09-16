//! Producer configuration utilities for testing

#![allow(dead_code)] // Test utilities may not all be used currently

use std::net::SocketAddr;
use producer::types::ProducerConfig;
use uuid::Uuid;

/// Create a test producer configuration
pub fn create_test_config(orchestrator_addr: SocketAddr, topic: &str) -> ProducerConfig {
    ProducerConfig::new(orchestrator_addr, topic.to_string())
}

/// Create a test producer configuration with default settings
pub fn create_default_test_config() -> ProducerConfig {
    let addr = "127.0.0.1:6001".parse().unwrap();
    ProducerConfig::new(addr, "test topic".to_string())
}

/// Create a test producer configuration with custom ID
pub fn create_test_config_with_id(orchestrator_addr: SocketAddr, topic: &str, id: Uuid) -> ProducerConfig {
    let mut config = ProducerConfig::new(orchestrator_addr, topic.to_string());
    config.id = id;
    config
}