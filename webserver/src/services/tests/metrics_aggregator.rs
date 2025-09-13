//! Tests for the MetricsAggregator service

use crate::services::RealMetricsAggregator;
use crate::traits::MetricsAggregator;
use crate::types::{AttributeUpdate, ClientId};
use crate::state::WebServerState;
use shared::{SystemMetrics, LLMPerformance, ProviderStatus};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// Helper functions for tests
fn create_test_state() -> Arc<WebServerState> {
    let bind_addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    let orch_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    Arc::new(WebServerState::new(bind_addr, orch_addr))
}

fn generate_client_id() -> ClientId {
    ClientId::new()
}

fn create_test_system_metrics() -> SystemMetrics {
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

fn create_test_attribute_update() -> AttributeUpdate {
    AttributeUpdate {
        attribute: "quality".to_string(),
        topic: "test-topic".to_string(),
        producer_id: "producer-1".to_string(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }
}

fn create_test_attribute_updates(count: usize) -> Vec<AttributeUpdate> {
    (0..count)
        .map(|i| AttributeUpdate {
            attribute: format!("attribute-{}", i),
            topic: format!("topic-{}", i % 3),
            producer_id: format!("producer-{}", i % 5),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (count - i) as u64,
        })
        .collect()
}

mod real_metrics_aggregator_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_metrics_aggregator() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Test that aggregator is created successfully
        let health = aggregator.get_system_health().await;
        assert!(health.is_ok());
        
        let health = health.unwrap();
        assert_eq!(health.active_clients, 0);
        assert!(!health.orchestrator_connected);
        assert!(health.server_uptime_seconds > 0);
    }

    #[tokio::test]
    async fn test_update_metrics() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        let test_metrics = create_test_system_metrics();
        let result = aggregator.update_metrics(test_metrics.clone()).await;
        
        assert!(result.is_ok());
        
        // Verify metrics can be retrieved
        let current_metrics = aggregator.get_current_metrics().await;
        assert!(current_metrics.is_ok());
        
        let retrieved = current_metrics.unwrap();
        assert_eq!(retrieved.total_unique_entries, test_metrics.total_unique_entries);
        assert_eq!(retrieved.entries_per_minute, test_metrics.entries_per_minute);
        assert_eq!(retrieved.active_producers, test_metrics.active_producers);
    }

    #[tokio::test]
    async fn test_add_attribute_update() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        let update = create_test_attribute_update();
        let result = aggregator.add_attribute_update(update.clone()).await;
        
        assert!(result.is_ok());
        
        // Verify update is stored
        let dashboard = aggregator.get_dashboard_data().await;
        assert!(dashboard.is_ok());
        
        let dashboard = dashboard.unwrap();
        assert!(!dashboard.recent_attributes.is_empty());
        
        let first_update = &dashboard.recent_attributes[0];
        assert_eq!(first_update.attribute, update.attribute);
        assert_eq!(first_update.producer_id, update.producer_id);
        assert_eq!(first_update.topic, update.topic);
    }

    #[tokio::test]
    async fn test_add_multiple_attribute_updates() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        let updates = create_test_attribute_updates(5);
        
        // Add all updates
        for update in &updates {
            let result = aggregator.add_attribute_update(update.clone()).await;
            assert!(result.is_ok());
        }
        
        // Verify all updates are stored
        let dashboard = aggregator.get_dashboard_data().await.unwrap();
        assert_eq!(dashboard.recent_attributes.len(), 5);
        
        // Verify they're in the correct order (most recent first)
        for (i, stored_update) in dashboard.recent_attributes.iter().enumerate() {
            let original_update = &updates[4 - i]; // Reverse order
            assert_eq!(stored_update.attribute, original_update.attribute);
            assert_eq!(stored_update.producer_id, original_update.producer_id);
        }
    }

    #[tokio::test]
    async fn test_attribute_update_limit() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Add more updates than the limit (assuming limit is 100)
        let updates = create_test_attribute_updates(150);
        
        for update in &updates {
            let result = aggregator.add_attribute_update(update.clone()).await;
            assert!(result.is_ok());
        }
        
        // Should only keep the most recent 100
        let dashboard = aggregator.get_dashboard_data().await.unwrap();
        assert!(dashboard.recent_attributes.len() <= 100);
        
        // Verify most recent updates are kept
        let first_stored = &dashboard.recent_attributes[0];
        let last_original = &updates[149];
        assert_eq!(first_stored.attribute, last_original.attribute);
    }

    #[tokio::test]
    async fn test_get_system_health() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state.clone());
        
        // Add some clients to state (simulated)
        let _client1 = generate_client_id();
        let _client2 = generate_client_id();
        // Note: In real tests, connections would be added via ClientRegistry
        
        let health = aggregator.get_system_health().await;
        assert!(health.is_ok());
        
        let health = health.unwrap();
        assert_eq!(health.active_clients, 0); // Default state without actual connections
        assert!(!health.orchestrator_connected); // Default state
        assert!(health.server_uptime_seconds > 0);
        assert!(health.last_update.is_some());
        
        // Note: Cleanup would be handled by ClientRegistry service
    }

    #[tokio::test]
    async fn test_get_current_metrics() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Initially should return default/empty metrics
        let result = aggregator.get_current_metrics().await;
        assert!(result.is_ok());
        
        // Update with test metrics
        let test_metrics = create_test_system_metrics();
        let _ = aggregator.update_metrics(test_metrics.clone()).await;
        
        // Should now return updated metrics
        let result = aggregator.get_current_metrics().await;
        assert!(result.is_ok());
        
        let metrics = result.unwrap();
        assert_eq!(metrics.total_unique_entries, test_metrics.total_unique_entries);
        assert_eq!(metrics.entries_per_minute, test_metrics.entries_per_minute);
    }

    #[tokio::test]
    async fn test_get_dashboard_data() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state.clone());
        
        // Set up test data
        let test_metrics = create_test_system_metrics();
        let _ = aggregator.update_metrics(test_metrics.clone()).await;
        
        let updates = create_test_attribute_updates(3);
        for update in &updates {
            let _ = aggregator.add_attribute_update(update.clone()).await;
        }
        
        // Get dashboard data
        let result = aggregator.get_dashboard_data().await;
        assert!(result.is_ok());
        
        let dashboard = result.unwrap();
        assert_eq!(dashboard.metrics.total_unique_entries, test_metrics.total_unique_entries);
        assert_eq!(dashboard.recent_attributes.len(), 3);
        assert_eq!(dashboard.system_health.active_clients, 0);
    }

    #[tokio::test]
    async fn test_clear_metrics() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Add some data
        let test_metrics = create_test_system_metrics();
        let _ = aggregator.update_metrics(test_metrics).await;
        
        let update = create_test_attribute_update();
        let _ = aggregator.add_attribute_update(update).await;
        
        // Clear metrics
        let result = aggregator.clear_metrics().await;
        assert!(result.is_ok());
        
        // Verify cleared state
        let dashboard = aggregator.get_dashboard_data().await.unwrap();
        assert!(dashboard.recent_attributes.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_updates() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Spawn multiple concurrent metric updates
        let mut handles = vec![];
        
        for i in 0..10 {
            let aggregator_clone = aggregator.clone();
            let handle = tokio::spawn(async move {
                let mut metrics = create_test_system_metrics();
                metrics.total_unique_entries = 100 + i;
                metrics.entries_per_minute = 10.0 + i as f64;
                aggregator_clone.update_metrics(metrics).await
            });
            handles.push(handle);
        }
        
        // Wait for all updates
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify final state
        let metrics = aggregator.get_current_metrics().await;
        assert!(metrics.is_ok());
    }

    #[tokio::test]
    async fn test_concurrent_attribute_updates() {
        let state = create_test_state();
        let aggregator = RealMetricsAggregator::new(state);
        
        // Spawn multiple concurrent attribute updates
        let mut handles = vec![];
        
        for i in 0..20 {
            let aggregator_clone = aggregator.clone();
            let handle = tokio::spawn(async move {
                let mut update = create_test_attribute_update();
                update.attribute = format!("attribute-{}", i);
                update.producer_id = format!("producer-{}", i);
                aggregator_clone.add_attribute_update(update).await
            });
            handles.push(handle);
        }
        
        // Wait for all updates
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify final state
        let dashboard = aggregator.get_dashboard_data().await;
        assert!(dashboard.is_ok());
        assert_eq!(dashboard.unwrap().recent_attributes.len(), 20);
    }
}
