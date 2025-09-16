//! Fault Tolerance Testing Assertions
//! 
//! Extends the Topic API with fault tolerance and healing specific assertions

use std::time::Duration;
use crate::runtime::CollectedEvent;

/// Enhanced assertions for fault tolerance testing
pub trait FaultToleranceAssertions {
    /// Assert that healing occurred within expected timeframe
    async fn assert_healing_occurred(&self, within: Duration) -> bool;
    
    /// Assert that a specific number of processes were restarted
    async fn assert_processes_restarted(&self, expected_count: u32) -> bool;
    
    /// Assert that system continued generating attributes after fault
    async fn assert_continued_generation_after_fault(&self, min_attributes: u64) -> bool;
    
    /// Assert that no data was lost during healing
    async fn assert_no_data_loss(&self) -> bool;
    
    /// Assert that healing completed within acceptable time
    async fn assert_healing_performance(&self, max_healing_time: Duration) -> bool;
    
    /// Assert that specific fault was injected and detected
    async fn assert_fault_injected(&self, fault_type: &str) -> bool;
    
    /// Assert that error logs indicate proper fault handling
    async fn assert_proper_error_handling(&self) -> bool;
    
    /// Assert that producers terminated correctly when orchestrator died
    async fn assert_producer_termination_on_orchestrator_death(&self, max_delay: Duration) -> bool;
    
    /// Assert that system reached stable state after healing
    async fn assert_stable_state_after_healing(&self, stability_duration: Duration) -> bool;
}

/// Helper methods for fault tolerance event analysis
pub trait FaultToleranceAnalysis {
    /// Get timeline of fault injection and healing events
    fn get_fault_timeline(&self) -> Vec<FaultEvent>;
    
    /// Calculate healing time for each fault
    fn calculate_healing_times(&self) -> Vec<Duration>;
    
    /// Get process restart events
    fn get_restart_events(&self) -> Vec<RestartEvent>;
    
    /// Analyze attribute generation rate before/after faults
    fn analyze_generation_rate_impact(&self) -> GenerationRateAnalysis;
    
    /// Print detailed fault tolerance summary
    fn print_fault_tolerance_summary(&self);
}

#[derive(Debug, Clone)]
pub struct FaultEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub fault_type: String,
    pub target: String,
    pub detection_delay: Option<Duration>,
    pub healing_completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct RestartEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub process_type: String,
    pub process_id: String,
    pub restart_reason: String,
}

#[derive(Debug)]
pub struct GenerationRateAnalysis {
    pub rate_before_fault: f64,
    pub rate_during_healing: f64,
    pub rate_after_healing: f64,
    pub total_interruption_time: Duration,
    pub recovery_time: Duration,
}