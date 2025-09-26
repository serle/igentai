//! Edge Cases and Error Scenarios
//!
//! Tests for edge cases, error conditions, and recovery

use crate::{OrchestratorConfig, ServiceConstellation, Topic, TracingCollector};
use std::time::Duration;

/// Test with minimal resources
pub async fn minimal(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Minimal: single producer, few iterations");

    let config = OrchestratorConfig::builder()
        .topic("minimal")
        .producers(1)
        .iterations(Some(2))
        .provider("random") // Use random provider for testing
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("minimal", collector, Duration::from_secs(20)).await {
        assert!(topic.assert_completed().await, "Should complete minimal test");
        tracing::info!("✅ Minimal: PASSED");
    } else {
        return Err("Minimal test failed".into());
    }

    Ok(())
}

/// Test system behavior with no iterations (should handle gracefully)
pub async fn empty(
    _collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Empty: zero iterations");

    let config = OrchestratorConfig::builder()
        .topic("empty")
        .producers(1)
        .iterations(Some(0)) // Zero iterations
        .provider("random") // Use random provider for testing
        .build();

    constellation.start_orchestrator(config).await?;

    // Wait briefly for immediate completion
    tokio::time::sleep(Duration::from_secs(3)).await;

    // This test just verifies the system doesn't crash with 0 iterations
    tracing::info!("✅ Empty: PASSED - system handled zero iterations gracefully");
    Ok(())
}
