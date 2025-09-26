//! Integration tests for WebServer functionality
//!
//! Tests the key user interactions: typing topics, start/stop, and unique list updates

use tokio::sync::mpsc;
use uuid::Uuid;

use webserver::{services::RealWebSocketManager, traits::WebSocketManager, types::ClientMessage};

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


#[test]
fn test_client_message_serialization() {
    // Test that attribute updates can be serialized
    let attr_update = ClientMessage::AttributeUpdate {
        attributes: vec!["unique1".to_string(), "unique2".to_string()],
        producer_id: shared::ProcessId::init_webserver().clone(),
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
