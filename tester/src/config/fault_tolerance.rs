//! Fault Tolerance Testing Configuration
//!
//! Extends the existing OrchestratorConfig with fault injection and healing test capabilities

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Fault injection scenarios for testing system resilience
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FaultScenario {
    /// Kill specific number of producers at specified iteration
    KillProducers {
        after_iteration: u32,
        num_producers: u32,
        heal_within_seconds: u64,
    },
    /// Kill the orchestrator at specified iteration
    KillOrchestrator { after_iteration: u32 },
    /// Kill all producers at specified iteration
    KillAllProducers {
        after_iteration: u32,
        heal_within_seconds: u64,
    },
}

/// Fault tolerance test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultToleranceConfig {
    /// Fault scenarios to inject
    pub scenarios: Vec<FaultScenario>,
    /// Total test iterations to run
    pub test_iterations: u32,
    /// Enable detailed fault injection logging
    pub verbose_fault_logging: bool,
}

impl Default for FaultToleranceConfig {
    fn default() -> Self {
        Self {
            scenarios: vec![],
            test_iterations: 20,
            verbose_fault_logging: true,
        }
    }
}
