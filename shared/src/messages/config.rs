//! Configuration and state management types
//!
//! Types for configuring system components and managing session state.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::GenerationConfig;

/// Producer configuration for initialization
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProducerConfig {
    pub producer_id: String,
    pub topic: String,
    pub prompt: String,
    pub provider_weights: HashMap<String, f32>,
    pub generation_config: GenerationConfig,
    pub tcp_port: u16,
}

/// Web server configuration
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WebServerConfig {
    pub bind_port: u16,
    pub orchestrator_address: String,
    pub static_file_root: String,
    pub websocket_path: String,
}

/// Session state for persistence
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionState {
    pub session_id: String,
    pub current_topic: Option<String>,
    pub total_unique_count: u64,
    pub active_producer_count: u32,
    pub start_time: u64,
    pub last_updated: u64,
    pub status: SessionStatus,
}

/// Session status enumeration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Idle,
    Initializing,
    Active,
    Stopping,
    Completed,
    Failed(String),
}