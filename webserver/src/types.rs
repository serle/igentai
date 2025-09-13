//! Type definitions for webserver
//!
//! This module contains all the data types, enums, and structs used by the webserver
//! that are not service traits.

use tokio::sync::mpsc;
use axum::extract::ws::Message;
use shared::SystemMetrics;

/// Client identifier for WebSocket connections
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ClientId(pub String);

impl ClientId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

/// WebSocket message types between browser and webserver
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum BrowserMessage {
    #[serde(rename = "start_topic")]
    StartTopic {
        topic: String,
        producer_count: u32,
        prompt: Option<String>,
    },
    #[serde(rename = "stop_generation")]
    StopGeneration,
    #[serde(rename = "request_dashboard")]
    RequestDashboard,
    #[serde(rename = "metrics_update")]
    MetricsUpdate {
        metrics: SystemMetrics,
    },
    #[serde(rename = "attribute_batch")]
    AttributeBatch {
        attributes: Vec<String>,
        topic: String,
    },
    #[serde(rename = "system_alert")]
    SystemAlert {
        level: AlertLevel,
        message: String,
        timestamp: u64,
    },
    #[serde(rename = "dashboard_data")]
    DashboardData {
        metrics: SystemMetrics,
        recent_attributes: Vec<AttributeUpdate>,
        system_health: SystemHealth,
    },
}

/// Alert levels for system messages
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AlertLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Attribute update from orchestrator
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AttributeUpdate {
    pub attribute: String,
    pub topic: String,
    pub producer_id: String,
    pub timestamp: u64,
}

/// System health information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemHealth {
    pub orchestrator_connected: bool,
    pub active_clients: u32,
    pub last_update: Option<u64>,
    pub server_uptime_seconds: u64,
}

/// Dashboard data bundle
#[derive(Debug, Clone)]
pub struct DashboardData {
    pub metrics: SystemMetrics,
    pub recent_attributes: Vec<AttributeUpdate>,
    pub system_health: SystemHealth,
}

/// Client connection information
#[derive(Debug)]
pub struct ClientConnection {
    pub id: ClientId,
    pub websocket_tx: mpsc::Sender<Message>,
    pub connected_at: std::time::Instant,
    pub last_ping: std::time::Instant,
    pub user_agent: Option<String>,
}

/// Key-value pair for configuration
#[derive(Debug, Clone)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_id_generation() {
        let id1 = ClientId::new();
        let id2 = ClientId::new();
        assert_ne!(id1, id2, "Client IDs should be unique");
    }

    #[test]
    fn test_browser_message_serialization() {
        let message = BrowserMessage::StartTopic {
            topic: "test topic".to_string(),
            producer_count: 5,
            prompt: Some("custom prompt".to_string()),
        };
        
        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: BrowserMessage = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            BrowserMessage::StartTopic { topic, producer_count, prompt } => {
                assert_eq!(topic, "test topic");
                assert_eq!(producer_count, 5);
                assert_eq!(prompt, Some("custom prompt".to_string()));
            }
            _ => panic!("Wrong message type after deserialization"),
        }
    }
}