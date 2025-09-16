//! Minimal producer tests to demonstrate architecture

use producer::{ProducerError, ProducerConfig, Processor, Metrics};

// Include integration test modules
mod fixtures;
mod integration;

#[test]
fn test_producer_architecture_basic() {
    // Test error creation
    let error = ProducerError::config("test error");
    match error {
        ProducerError::ConfigError { message } => {
            assert_eq!(message, "test error");
        }
        _ => panic!("Wrong error type"),
    }

    // Test config creation
    let addr = "127.0.0.1:6001".parse().unwrap();
    let config = ProducerConfig::new(addr, "test topic".to_string());
    
    assert_eq!(config.orchestrator_addr, addr);
    assert_eq!(config.topic, "test topic");
    assert_eq!(config.max_concurrent_requests, 10);

    println!("✅ Producer architecture basics work!");
}

#[test]
fn test_producer_metrics_calculation() {
    use producer::types::ProducerMetrics;
    
    let mut metrics = ProducerMetrics::new();
    metrics.attributes_extracted = 120;
    metrics.unique_attributes = 100;
    metrics.total_tokens_used = 5000;
    metrics.total_cost = 2.5;
    metrics.uptime_seconds = 60; // 1 minute

    assert_eq!(metrics.attributes_per_minute(), 120.0);
    assert_eq!(metrics.cost_efficiency(), 40.0); // 100 attributes / $2.5
    assert_eq!(metrics.token_efficiency(), 20.0); // (100 * 1000) / 5000

    println!("✅ Producer metrics calculations work!");
}

#[test]
fn test_processor_creation() {
    let processor = Processor::new();
    let stats = processor.get_stats();
    
    assert_eq!(stats.total_unique_attributes, 0);
    assert_eq!(stats.bloom_filter_false_positive_rate, 0.01);
    assert_eq!(stats.bloom_filter_enabled, true); // Always enabled now
    assert_eq!(stats.duplicate_count, 0);
    assert_eq!(stats.total_processed, 0);
    
    println!("✅ Processor creation works!");
}

#[test]
fn test_metrics_creation() {
    let metrics = Metrics::new();
    let current = metrics.get_current_metrics();
    
    assert_eq!(current.requests_sent, 0);
    assert_eq!(current.responses_received, 0);
    assert_eq!(current.attributes_extracted, 0);
    
    println!("✅ Metrics creation works!");
}