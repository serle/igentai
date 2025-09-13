//! WebServer ↔ Orchestrator communication messages
//!
//! Messages for communication between web server and orchestrator.
//! Note: Browser ↔ WebServer messages are internal to the webserver component.

use serde::{Serialize, Deserialize};
use super::metrics::{SystemMetrics, AttributeUpdate};

/// Task requests from web server to orchestrator
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TaskRequest {
    TopicRequest {
        topic: String,
        producer_count: u32,
        prompt: Option<String>,
    },
    StopGeneration,
    RequestStatus,
}

/// Task updates from orchestrator to web server
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TaskUpdate {
    SystemMetrics(SystemMetrics),
    NewAttributes(Vec<AttributeUpdate>),
    SystemStatus {
        active_producers: u32,
        current_topic: Option<String>,
        total_unique: u64,
    },
    ErrorNotification(String),
}