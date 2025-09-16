//! Fault Injection Engine
//! 
//! Handles controlled fault injection for testing system resilience

use std::process::{Child, Command};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time::{sleep, timeout};
use sysinfo::{ProcessExt, System, SystemExt};
use crate::config::fault_tolerance::FaultScenario;

pub struct FaultInjector {
    system: System,
    tracked_processes: HashMap<String, u32>, // process_name -> PID
    injection_log: Vec<InjectionEvent>,
    current_iteration: u32,
    pending_scenarios: Vec<FaultScenario>,
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
            system: System::new_all(),
            tracked_processes: HashMap::new(),
            injection_log: Vec::new(),
            current_iteration: 0,
            pending_scenarios: Vec::new(),
        }
    }
    
    /// Add fault scenarios to be triggered at specific iterations
    pub fn add_scenarios(&mut self, scenarios: Vec<FaultScenario>) {
        self.pending_scenarios.extend(scenarios);
        tracing::info!("📋 Added {} fault scenarios", self.pending_scenarios.len());
    }
    
    /// Register a process for fault injection
    pub fn register_process(&mut self, name: String, pid: u32) {
        tracing::info!("🎯 Registering process for fault injection: {} (PID: {})", name, pid);
        self.tracked_processes.insert(name, pid);
    }
    
    /// Discover and register orchestrator and producer processes
    pub async fn discover_processes(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.system.refresh_all();
        
        for process in self.system.processes().values() {
            let name = process.name();
            let pid = process.pid().as_u32();
            
            if name.contains("orchestrator") {
                self.register_process("orchestrator".to_string(), pid);
            } else if name.contains("producer") {
                // Extract producer ID if possible
                let cmd_line = process.cmd().join(" ");
                if let Some(id) = extract_producer_id(&cmd_line) {
                    self.register_process(format!("producer_{}", id), pid);
                }
            }
        }
        
        tracing::info!("🔍 Discovered {} processes for fault injection", self.tracked_processes.len());
        Ok(())
    }
    
    /// Check if any faults should be triggered at the current iteration
    pub async fn on_iteration(&mut self, iteration: u32) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.current_iteration = iteration;
        let mut triggered_faults = Vec::new();
        
        // Find scenarios that should trigger at this iteration
        let mut scenarios_to_trigger = Vec::new();
        let mut indices_to_remove = Vec::new();
        
        for (i, scenario) in self.pending_scenarios.iter().enumerate() {
            if self.should_trigger_at_iteration(scenario, iteration) {
                scenarios_to_trigger.push(scenario.clone());
                indices_to_remove.push(i);
            }
        }
        
        // Remove triggered scenarios from pending list (in reverse order to maintain indices)
        for &i in indices_to_remove.iter().rev() {
            self.pending_scenarios.remove(i);
        }
        
        // Execute triggered scenarios
        for scenario in scenarios_to_trigger {
            tracing::info!("💉 Triggering fault at iteration {}: {:?}", iteration, scenario);
            
            match scenario {
                FaultScenario::KillProducers { num_producers, .. } => {
                    let fault_desc = self.kill_producers(num_producers).await?;
                    triggered_faults.push(fault_desc);
                }
                FaultScenario::KillOrchestrator { .. } => {
                    self.kill_orchestrator().await?;
                    triggered_faults.push("KillOrchestrator".to_string());
                }
                FaultScenario::KillAllProducers { .. } => {
                    let fault_desc = self.kill_all_producers().await?;
                    triggered_faults.push(fault_desc);
                }
            }
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
    
    /// Kill specified number of producers
    async fn kill_producers(&mut self, num_producers: u32) -> Result<String, Box<dyn std::error::Error>> {
        let producer_names: Vec<String> = self.tracked_processes.keys()
            .filter(|k| k.starts_with("producer_"))
            .cloned()
            .collect();
            
        if producer_names.is_empty() {
            return Err("No producers found to kill".into());
        }
        
        let to_kill = std::cmp::min(num_producers as usize, producer_names.len());
        let mut killed_count = 0;
        
        for target_name in producer_names.iter().take(to_kill) {
            if let Some(&pid) = self.tracked_processes.get(target_name) {
                tracing::warn!("💀 Killing producer process: {} (PID: {})", target_name, pid);
                
                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;
                    
                    match signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
                        Ok(()) => {
                            self.log_injection("KillProducers", target_name, true, "Process killed successfully");
                            tracing::info!("✅ Successfully killed producer {}", target_name);
                            killed_count += 1;
                        }
                        Err(e) => {
                            self.log_injection("KillProducers", target_name, false, &format!("Failed to kill: {}", e));
                            tracing::error!("❌ Failed to kill producer {}: {}", target_name, e);
                        }
                    }
                }
                
                #[cfg(windows)]
                {
                    tracing::warn!("⚠️ Windows process killing not implemented");
                }
            }
        }
        
        Ok(format!("KillProducers({})", killed_count))
    }
    
    /// Kill all producers
    async fn kill_all_producers(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let producer_names: Vec<String> = self.tracked_processes.keys()
            .filter(|k| k.starts_with("producer_"))
            .cloned()
            .collect();
            
        let total = producer_names.len();
        self.kill_producers(total as u32).await
    }
    
    /// Kill orchestrator process
    async fn kill_orchestrator(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(&pid) = self.tracked_processes.get("orchestrator") {
            tracing::warn!("💀 Killing orchestrator process (PID: {})", pid);
            
            #[cfg(unix)]
            {
                use nix::sys::signal::{self, Signal};
                use nix::unistd::Pid;
                
                match signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL) {
                    Ok(()) => {
                        self.log_injection("KillOrchestrator", "orchestrator", true, "Process killed successfully");
                        tracing::info!("✅ Successfully killed orchestrator");
                    }
                    Err(e) => {
                        self.log_injection("KillOrchestrator", "orchestrator", false, &format!("Failed to kill: {}", e));
                        tracing::error!("❌ Failed to kill orchestrator: {}", e);
                    }
                }
            }
        } else {
            tracing::error!("❌ Orchestrator process not found");
        }
        
        Ok(())
    }
    
    
    /// Log fault injection event
    fn log_injection(&mut self, fault_type: &str, target: &str, success: bool, details: &str) {
        let event = InjectionEvent {
            timestamp: chrono::Utc::now(),
            fault_type: fault_type.to_string(),
            target: target.to_string(),
            success,
            details: details.to_string(),
        };
        
        self.injection_log.push(event);
    }
    
    /// Get injection log for analysis
    pub fn get_injection_log(&self) -> &[InjectionEvent] {
        &self.injection_log
    }
}

/// Extract producer ID from command line arguments
fn extract_producer_id(cmd_line: &str) -> Option<String> {
    // Look for --id argument
    if let Some(pos) = cmd_line.find("--id") {
        let remainder = &cmd_line[pos + 4..];
        remainder.split_whitespace().next().map(|s| s.to_string())
    } else {
        None
    }
}