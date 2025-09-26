//! Builder Extensions for Fault Tolerance Testing
//!
//! Extends the existing OrchestratorConfigBuilder with fault tolerance capabilities

use crate::config::fault_tolerance::{FaultScenario, FaultToleranceConfig};
use crate::config::OrchestratorConfigBuilder;

/// Extension trait for adding fault tolerance configuration to the builder
pub trait FaultToleranceConfigBuilder {
    /// Add fault tolerance testing configuration
    fn fault_tolerance(self, config: FaultToleranceConfig) -> Self;

    /// Kill producers after specified iteration: kill_after(iteration, num_producers, heal_within_seconds)
    fn kill_after(self, iteration: u32, num_producers: u32, heal_within_seconds: u64) -> Self;

    /// Kill orchestrator after specified iteration
    fn kill_orchestrator_after(self, iteration: u32) -> Self;

    /// Kill all producers after specified iteration
    fn kill_all_after(self, iteration: u32, heal_within_seconds: u64) -> Self;

    /// Comprehensive multi-fault test over specified iterations
    fn with_multi_fault_test(self, total_iterations: u32) -> Self;
}

impl FaultToleranceConfigBuilder for OrchestratorConfigBuilder {
    fn fault_tolerance(mut self, config: FaultToleranceConfig) -> Self {
        self.fault_tolerance_config = Some(config);
        self
    }

    fn kill_after(self, iteration: u32, num_producers: u32, heal_within_seconds: u64) -> Self {
        let config = FaultToleranceConfig {
            scenarios: vec![FaultScenario::KillProducers {
                after_iteration: iteration,
                num_producers,
                heal_within_seconds,
            }],
            test_iterations: iteration + 10, // Run for 10 more iterations after fault
            verbose_fault_logging: true,
        };

        self.fault_tolerance(config)
    }

    fn kill_orchestrator_after(self, iteration: u32) -> Self {
        let config = FaultToleranceConfig {
            scenarios: vec![FaultScenario::KillOrchestrator {
                after_iteration: iteration,
            }],
            test_iterations: iteration + 5, // Short test since producers should terminate
            verbose_fault_logging: true,
        };

        self.fault_tolerance(config)
    }

    fn kill_all_after(self, iteration: u32, heal_within_seconds: u64) -> Self {
        let config = FaultToleranceConfig {
            scenarios: vec![FaultScenario::KillAllProducers {
                after_iteration: iteration,
                heal_within_seconds,
            }],
            test_iterations: iteration + 15, // Run longer to test full recovery
            verbose_fault_logging: true,
        };

        self.fault_tolerance(config)
    }

    fn with_multi_fault_test(self, total_iterations: u32) -> Self {
        let config = FaultToleranceConfig {
            scenarios: vec![
                // Kill 1 producer early
                FaultScenario::KillProducers {
                    after_iteration: total_iterations / 4,
                    num_producers: 1,
                    heal_within_seconds: 30,
                },
                // Kill 2 producers mid-way
                FaultScenario::KillProducers {
                    after_iteration: total_iterations / 2,
                    num_producers: 2,
                    heal_within_seconds: 45,
                },
                // Kill all producers near the end
                FaultScenario::KillAllProducers {
                    after_iteration: total_iterations * 3 / 4,
                    heal_within_seconds: 60,
                },
            ],
            test_iterations: total_iterations,
            verbose_fault_logging: true,
        };

        self.fault_tolerance(config)
    }
}
