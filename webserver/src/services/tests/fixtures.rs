//! Test fixtures for webserver service tests

use crate::types::{ClientId, AttributeUpdate, BrowserMessage, ClientConnection};
use shared::{SystemMetrics, LLMPerformance, ProviderStatus};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use axum::extract::ws::Message;

/// Generate a unique client ID for testing
pub fn generate_client_id() -> ClientId {
    ClientId::new()
}

/// Create a test client connection with bounded channel
pub fn create_test_client_connection() -> ClientConnection {
    let (tx, _rx) = mpsc::channel::<Message>(100);
    let id = generate_client_id();
    ClientConnection {
        id,
        websocket_tx: tx,
        connected_at: std::time::Instant::now(),
        last_ping: std::time::Instant::now(),
        user_agent: Some("test-agent".to_string()),
    }
}

/// Create a test browser message for topic generation
pub fn create_test_browser_message() -> BrowserMessage {
    BrowserMessage::StartTopic {
        topic: "test-topic".to_string(),
        producer_count: 5,
        prompt: Some("test prompt".to_string()),
    }
}

/// Create test system metrics
pub fn create_test_system_metrics() -> SystemMetrics {
    let mut per_llm_performance = HashMap::new();
    per_llm_performance.insert(
        "gpt-4".to_string(),
        LLMPerformance {
            unique_generated: 100,
            success_rate: 0.95,
            average_response_time_ms: 1500,
            uniqueness_ratio: 0.88,
            efficiency_score: 0.92,
            consecutive_failures: 0,
            last_success_timestamp: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
            current_status: ProviderStatus::Healthy,
        }
    );

    SystemMetrics {
        total_unique_entries: 1000,
        entries_per_minute: 50.0,
        per_llm_performance,
        current_topic: Some("test-topic".to_string()),
        active_producers: 5,
        uptime_seconds: 3600,
        last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }
}

/// Create a test attribute update
pub fn create_test_attribute_update() -> AttributeUpdate {
    AttributeUpdate {
        attribute: "quality".to_string(),
        topic: "test-topic".to_string(),
        producer_id: "producer-1".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }
}

/// Create multiple test attribute updates
pub fn create_test_attribute_updates(count: usize) -> Vec<AttributeUpdate> {
    (0..count)
        .map(|i| AttributeUpdate {
            attribute: format!("attribute-{}", i),
            topic: format!("topic-{}", i % 3),
            producer_id: format!("producer-{}", i % 5),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (count - i) as u64,
        })
        .collect()
}