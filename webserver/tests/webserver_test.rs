//! Integration tests for WebServer functionality
//! 
//! Tests the key user interactions: typing topics, start/stop, and unique list updates

use tokio::sync::mpsc;
use uuid::Uuid;

use webserver::{
    core::{WebServerState},
    services::{RealWebSocketManager},
    traits::{WebSocketManager},
    types::{ClientMessage},
};
use shared::{OrchestratorUpdate, SystemMetrics};

#[tokio::test]
async fn test_websocket_manager() {
    let manager = RealWebSocketManager::new();
    let client_id = Uuid::new_v4();
    
    // Test initial client count
    assert_eq!(manager.client_count().await, 0);
    
    // Test adding a client (with a dummy channel)
    let (tx, _rx) = mpsc::channel(100);
    manager.add_client(client_id, tx).await.unwrap();
    
    assert_eq!(manager.client_count().await, 1);
    
    // Test removing client
    manager.remove_client(client_id).await.unwrap();
    assert_eq!(manager.client_count().await, 0);
}

#[tokio::test] 
async fn test_orchestrator_update_processing() {
    let mut state = WebServerState::new();
    
    // Test SystemMetrics update
    let metrics = SystemMetrics {
        uam: 5.2,
        cost_per_minute: 0.01,
        tokens_per_minute: 1000.0,
        unique_per_dollar: 520.0,
        unique_per_1k_tokens: 25.0,
        active_producers: 3,
        by_provider: std::collections::HashMap::new(),
        by_producer: std::collections::HashMap::new(),
        current_topic: Some("test_topic".to_string()),
        uptime_seconds: 120,
        last_updated: chrono::Utc::now().timestamp() as u64,
    };
    
    let update = OrchestratorUpdate::SystemMetrics(metrics.clone());
    let client_messages = state.process_orchestrator_update(update);
    
    // Should generate a dashboard update
    assert!(!client_messages.is_empty());
    assert!(matches!(client_messages[0], ClientMessage::DashboardUpdate { .. }));
    
    // Test NewAttributes update (unique list update)
    let attributes = vec!["attr1".to_string(), "attr2".to_string(), "attr3".to_string()];
    let update = OrchestratorUpdate::NewAttributes(attributes.clone());
    let client_messages = state.process_orchestrator_update(update);
    
    // Should generate an attribute update
    assert!(!client_messages.is_empty());
    assert!(matches!(client_messages[0], ClientMessage::AttributeUpdate { .. }));
}

#[test]
fn test_client_message_serialization() {
    // Test that attribute updates can be serialized
    let attr_update = ClientMessage::AttributeUpdate {
        attributes: vec!["unique1".to_string(), "unique2".to_string()],
        producer_id: shared::ProcessId::current(),
        metadata: shared::ProviderMetadata {
            provider_id: shared::ProviderId::OpenAI,
            model: "gpt-4".to_string(),
            response_time_ms: 1500,
            tokens: shared::TokenUsage::default(),
            request_timestamp: 1234567890,
        },
        uniqueness_ratio: 0.85,
    };
    
    let json = serde_json::to_string(&attr_update).unwrap();
    let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
    
    assert!(matches!(parsed, ClientMessage::AttributeUpdate { .. }));
}