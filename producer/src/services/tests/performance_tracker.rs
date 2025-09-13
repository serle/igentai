//! Tests for PerformanceTracker service

use std::time::Duration;
use shared::ApiFailure;
use crate::services::performance_tracker::RealPerformanceTracker;
use crate::traits::PerformanceTracker;

#[tokio::test]
async fn test_performance_tracker_creation() {
    let tracker = RealPerformanceTracker::new();
    
    // Should start with empty stats
    let stats = tracker.get_stats().await.unwrap();
    assert!(stats.is_empty());
}

#[tokio::test]
async fn test_record_success() {
    let tracker = RealPerformanceTracker::new();
    
    let result = tracker.record_success(
        "openai",
        Duration::from_millis(500),
        100
    ).await;
    
    assert!(result.is_ok());
    
    // Verify stats were updated
    let stats = tracker.get_stats().await.unwrap();
    assert!(stats.contains_key("openai"));
    
    let openai_stats = &stats["openai"];
    assert_eq!(openai_stats.total_requests, 1);
    assert_eq!(openai_stats.successful_requests, 1);
    assert_eq!(openai_stats.failed_requests, 0);
}

#[tokio::test]
async fn test_record_failure() {
    let tracker = RealPerformanceTracker::new();
    
    let result = tracker.record_failure(
        "anthropic",
        ApiFailure::RateLimitExceeded
    ).await;
    
    assert!(result.is_ok());
    
    // Verify stats were updated
    let stats = tracker.get_stats().await.unwrap();
    assert!(stats.contains_key("anthropic"));
    
    let anthropic_stats = &stats["anthropic"];
    assert_eq!(anthropic_stats.total_requests, 1);
    assert_eq!(anthropic_stats.successful_requests, 0);
    assert_eq!(anthropic_stats.failed_requests, 1);
}

#[tokio::test]
async fn test_multiple_provider_tracking() {
    let tracker = RealPerformanceTracker::new();
    
    // Record success for OpenAI
    tracker.record_success("openai", Duration::from_millis(300), 50).await.unwrap();
    tracker.record_success("openai", Duration::from_millis(700), 75).await.unwrap();
    
    // Record failure for Anthropic
    tracker.record_failure("anthropic", ApiFailure::AuthenticationFailed).await.unwrap();
    
    // Record success for Gemini
    tracker.record_success("gemini", Duration::from_millis(400), 60).await.unwrap();
    
    let stats = tracker.get_stats().await.unwrap();
    
    // Should have stats for all three providers
    assert_eq!(stats.len(), 3);
    assert!(stats.contains_key("openai"));
    assert!(stats.contains_key("anthropic"));
    assert!(stats.contains_key("gemini"));
    
    // Check OpenAI stats
    let openai_stats = &stats["openai"];
    assert_eq!(openai_stats.total_requests, 2);
    assert_eq!(openai_stats.successful_requests, 2);
    assert_eq!(openai_stats.failed_requests, 0);
    assert_eq!(openai_stats.total_response_time_ms, 1000); // 300 + 700
    
    // Check Anthropic stats
    let anthropic_stats = &stats["anthropic"];
    assert_eq!(anthropic_stats.total_requests, 1);
    assert_eq!(anthropic_stats.successful_requests, 0);
    assert_eq!(anthropic_stats.failed_requests, 1);
    
    // Check Gemini stats
    let gemini_stats = &stats["gemini"];
    assert_eq!(gemini_stats.total_requests, 1);
    assert_eq!(gemini_stats.successful_requests, 1);
    assert_eq!(gemini_stats.failed_requests, 0);
}

#[tokio::test]
async fn test_response_time_tracking() {
    let tracker = RealPerformanceTracker::new();
    
    let fast_time = Duration::from_millis(100);
    let slow_time = Duration::from_millis(2000);
    
    tracker.record_success("test_provider", fast_time, 50).await.unwrap();
    tracker.record_success("test_provider", slow_time, 75).await.unwrap();
    
    let stats = tracker.get_stats().await.unwrap();
    let provider_stats = &stats["test_provider"];
    
    // Total response time should be sum of both
    assert_eq!(provider_stats.total_response_time_ms, 2100);
    assert_eq!(provider_stats.total_requests, 2);
}

#[tokio::test]
async fn test_different_failure_types() {
    let tracker = RealPerformanceTracker::new();
    
    let failures = vec![
        ApiFailure::AuthenticationFailed,
        ApiFailure::RateLimitExceeded,
        ApiFailure::QuotaExceeded,
        ApiFailure::Timeout,
        ApiFailure::ServiceUnavailable,
    ];
    
    for (i, failure) in failures.into_iter().enumerate() {
        let provider = format!("provider_{}", i);
        tracker.record_failure(&provider, failure).await.unwrap();
    }
    
    let stats = tracker.get_stats().await.unwrap();
    assert_eq!(stats.len(), 5); // One provider per failure type
    
    for (_, provider_stats) in stats {
        assert_eq!(provider_stats.failed_requests, 1);
        assert_eq!(provider_stats.successful_requests, 0);
    }
}

#[tokio::test]
async fn test_concurrent_updates() {
    let tracker = RealPerformanceTracker::new();
    
    // Simulate concurrent requests
    let mut handles = vec![];
    
    for i in 0..10 {
        let tracker_clone = tracker.clone();
        let handle = tokio::spawn(async move {
            tracker_clone.record_success(
                "concurrent_provider",
                Duration::from_millis(100),
                50
            ).await
        });
        handles.push(handle);
    }
    
    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    
    let stats = tracker.get_stats().await.unwrap();
    let provider_stats = &stats["concurrent_provider"];
    
    // Should have recorded all 10 requests
    assert_eq!(provider_stats.total_requests, 10);
    assert_eq!(provider_stats.successful_requests, 10);
    assert_eq!(provider_stats.failed_requests, 0);
}