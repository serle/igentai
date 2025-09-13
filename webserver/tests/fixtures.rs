//! Test fixtures for webserver integration tests

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use shared::{SystemMetrics, LLMPerformance, ProviderStatus};
use webserver::{BrowserMessage, AttributeUpdate, ClientId};

/// Create a test browser message for starting topic generation
pub fn create_test_start_topic_message() -> BrowserMessage {
    BrowserMessage::StartTopic {
        topic: "integration-test-topic".to_string(),
        producer_count: 3,
        prompt: Some("integration test prompt".to_string()),
    }
}

/// Create a test browser message for stopping generation
pub fn create_test_stop_message() -> BrowserMessage {
    BrowserMessage::StopGeneration
}

/// Create a test browser message for requesting dashboard
pub fn create_test_dashboard_request() -> BrowserMessage {
    BrowserMessage::RequestDashboard
}

/// Create test system metrics for integration tests
pub fn create_test_system_metrics() -> SystemMetrics {
    let mut per_llm_performance = HashMap::new();
    per_llm_performance.insert(
        "gpt-4".to_string(),
        LLMPerformance {
            unique_generated: 50,
            success_rate: 0.9,
            average_response_time_ms: 1200,
            uniqueness_ratio: 0.85,
            efficiency_score: 0.88,
            consecutive_failures: 0,
            last_success_timestamp: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
            current_status: ProviderStatus::Healthy,
        }
    );

    SystemMetrics {
        total_unique_entries: 500,
        entries_per_minute: 25.0,
        per_llm_performance,
        current_topic: Some("integration-test-topic".to_string()),
        active_producers: 3,
        uptime_seconds: 1800,
        last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }
}

/// Create test attribute updates for integration tests
pub fn create_test_attribute_updates(count: usize) -> Vec<AttributeUpdate> {
    (0..count)
        .map(|i| AttributeUpdate {
            attribute: format!("integration-attribute-{}", i),
            topic: "integration-test-topic".to_string(),
            producer_id: format!("integration-producer-{}", i % 3),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (count - i) as u64 * 10,
        })
        .collect()
}

/// Generate a test client ID
pub fn generate_test_client_id() -> ClientId {
    ClientId::new()
}

/// Create test message payload as JSON
pub fn create_test_json_message() -> serde_json::Value {
    serde_json::json!({
        "type": "integration_test",
        "data": "test payload",
        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
    })
}