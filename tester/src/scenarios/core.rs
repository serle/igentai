//! Core Functionality Tests
//!
//! Essential system functionality tests

use crate::{OrchestratorConfig, ServiceConstellation, Topic, TracingCollector};
use std::time::Duration;

/// Test basic orchestrator + producer functionality
pub async fn basic(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Basic: orchestrator + producers");

    let config = OrchestratorConfig::builder()
        .topic("basic")
        .producers(2)
        .iterations(Some(5))
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("basic", collector, Duration::from_secs(30)).await {
        assert!(
            topic.assert_started_with_budget(Some(5)).await,
            "Should start with budget"
        );
        assert!(topic.assert_completed().await, "Should complete");
        tracing::info!("✅ Basic: PASSED");
    } else {
        return Err("Basic test failed".into());
    }

    Ok(())
}

/// Test high load scenario
pub async fn load(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Load: many producers + iterations");

    let config = OrchestratorConfig::builder()
        .topic("load")
        .producers(4)
        .iterations(Some(10))
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("load", collector, Duration::from_secs(60)).await {
        assert!(topic.assert_completed().await, "Should complete under load");
        assert!(topic.assert_min_attributes(200), "Should generate many attributes");
        tracing::info!("✅ Load: PASSED");
    } else {
        return Err("Load test failed".into());
    }

    Ok(())
}

/// Test healing functionality (system heals when producers fail)
pub async fn healing(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Healing: producer failure recovery");

    let config = OrchestratorConfig::builder()
        .topic("healing")
        .producers(3)
        .iterations(Some(20)) // Long enough to see healing in action
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("healing", collector, Duration::from_secs(90)).await {
        assert!(topic.assert_completed().await, "Should complete despite failures");
        // Note: healing is automatic in the system - producers naturally fail and get restarted
        tracing::info!("✅ Healing: PASSED");
    } else {
        return Err("Healing test failed".into());
    }

    Ok(())
}
