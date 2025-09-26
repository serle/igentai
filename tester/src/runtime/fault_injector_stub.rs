//! Fault Injection Engine - Stub Implementation
//!
//! Simple stub for development - doesn't actually inject faults

use crate::config::fault_tolerance::FaultScenario;

#[derive(Clone)]
pub struct FaultInjector {
    pending_scenarios: Vec<FaultScenario>,
    injection_log: Vec<InjectionEvent>,
}

#[derive(Debug, Clone)]
pub struct InjectionEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub fault_type: String,
    pub target: String,
    pub success: bool,
    pub details: String,
}

impl FaultInjector {
    pub fn new() -> Self {
        Self {
            pending_scenarios: Vec::new(),
            injection_log: Vec::new(),
        }
    }

    /// Add fault scenarios to be triggered at specific iterations
    pub fn add_scenarios(&mut self, scenarios: Vec<FaultScenario>) {
        self.pending_scenarios.extend(scenarios);
        tracing::info!("📋 Added {} fault scenarios (stub mode)", self.pending_scenarios.len());
    }

    /// Discover and register orchestrator and producer processes (stub)
    pub async fn discover_processes(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("🔍 Process discovery (stub mode) - no actual processes tracked");
        Ok(())
    }

    /// Check if any faults should be triggered at the current iteration (stub)
    pub async fn on_iteration(&mut self, iteration: u32) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // In stub mode, just log what would be triggered but don't actually do anything
        let mut triggered_faults = Vec::new();

        let mut indices_to_remove = Vec::new();
        for (i, scenario) in self.pending_scenarios.iter().enumerate() {
            if self.should_trigger_at_iteration(scenario, iteration) {
                let fault_desc = match scenario {
                    FaultScenario::KillProducers { num_producers, .. } => {
                        format!("KillProducers({})", num_producers)
                    }
                    FaultScenario::KillOrchestrator { .. } => "KillOrchestrator".to_string(),
                    FaultScenario::KillAllProducers { .. } => "KillAllProducers".to_string(),
                };

                tracing::info!(
                    "💉 STUB: Would trigger fault at iteration {}: {}",
                    iteration,
                    fault_desc
                );
                triggered_faults.push(fault_desc);
                indices_to_remove.push(i);
            }
        }

        // Remove triggered scenarios
        for &i in indices_to_remove.iter().rev() {
            self.pending_scenarios.remove(i);
        }

        Ok(triggered_faults)
    }

    /// Check if a scenario should trigger at the given iteration
    fn should_trigger_at_iteration(&self, scenario: &FaultScenario, iteration: u32) -> bool {
        match scenario {
            FaultScenario::KillProducers { after_iteration, .. } => *after_iteration == iteration,
            FaultScenario::KillOrchestrator { after_iteration } => *after_iteration == iteration,
            FaultScenario::KillAllProducers { after_iteration, .. } => *after_iteration == iteration,
        }
    }

    /// Get injection log for analysis
    pub fn get_injection_log(&self) -> &[InjectionEvent] {
        &self.injection_log
    }
}
