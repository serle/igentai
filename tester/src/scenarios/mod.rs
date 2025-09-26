//! Test Scenarios
//!
//! Clean, short scenario names for comprehensive E2E testing

// Re-export the assert_trace macro for use in scenario modules
pub use crate::assert_trace;

pub mod core;
pub mod edge;
pub mod web;

use crate::{
    runtime::{ServiceConstellation, TracingCollector},
    testing::TracingAssertions,
};

pub struct TestScenarios {
    assertions: TracingAssertions,
}

impl TestScenarios {
    pub fn new(collector: TracingCollector) -> Self {
        Self {
            assertions: TracingAssertions::new(collector),
        }
    }

    /// Run a specific scenario by name
    pub async fn run_scenario(
        &self,
        name: &str,
        constellation: &mut ServiceConstellation,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let collector = self.assertions.get_collector().clone();

        match name {
            // Core functionality tests
            "basic" => core::basic(collector, constellation).await,
            "load" => core::load(collector, constellation).await,
            "healing" => core::healing(collector, constellation).await,
            "single_start" => core::single_start_command(collector, constellation).await,
            "trace_capture" => core::trace_capture(collector, constellation).await,
            "real_api" => core::real_api(collector, constellation).await,

            // Web/HTTP interface tests
            "server" => web::server(collector, constellation).await,

            // Edge cases and error scenarios
            "minimal" => edge::minimal(collector, constellation).await,
            "empty" => edge::empty(collector, constellation).await,

            // Run all core tests
            "core" => {
                core::basic(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::load(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::healing(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::single_start_command(collector, constellation).await
            }

            // Run all tests
            "all" => {
                tracing::info!("🧪 Running FULL E2E Test Suite");

                // Core tests
                core::basic(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::load(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::healing(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                core::single_start_command(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                // Edge cases
                edge::minimal(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                edge::empty(collector.clone(), constellation).await?;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                // Web server test last (since it keeps running)
                web::server(collector, constellation).await?;

                tracing::info!("🏆 ALL E2E Tests COMPLETED Successfully!");
                Ok(())
            }

            _ => Err(format!(
                "Unknown test scenario: '{}'. Available: {}",
                name,
                Self::available_scenarios().join(", ")
            )
            .into()),
        }
    }

    /// Get list of available scenarios
    pub fn available_scenarios() -> Vec<&'static str> {
        vec![
            // Individual tests
            "basic", "load", "healing", "single_start", "trace_capture", "real_api", // Core functionality
            "server",  // Web interface
            "minimal", "empty", // Edge cases
            // Test suites
            "core", // All core tests
            "all",  // Complete test suite
        ]
    }
}
