//! Core orchestrator state management
//!
//! This module provides the main state structure that coordinates
//! all aspects of the orchestrator system.

use super::{PerformanceTracker, UniquenessTracker};
use crate::error::OrchestratorResult;
use serde::{Deserialize, Serialize};
use shared::{process_debug, process_info, OrchestratorCommand, ProcessId, ProviderId, SystemMetrics};
use std::collections::HashMap;
use std::time::Instant;

/// Statistics for a single cycle/iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleStats {
    pub iteration: u32,
    pub total_values: u64,
    pub new_values: u64,
    pub duplicate_values: u64,
    pub efficiency: f64,       // new_values / total_attempted * 100
    pub efficiency_delta: f64, // change from previous cycle
    pub timestamp: String,
    pub duration_seconds: f64,
}

/// Provider performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPerformanceStats {
    pub provider_id: ProviderId,
    pub requests_per_minute: f64,
    pub unique_attributes_per_minute: f64,
    pub cost_per_minute: f64,
    pub efficiency_ratio: f64,
    pub total_requests: u64,
    pub total_unique_attributes: u64,
    pub total_cost: f64,
    pub average_response_time_ms: f64,
    pub success_rate: f64,
}

/// Cycle performance summary for JSON export
#[derive(Debug, Serialize, Deserialize)]
pub struct CyclePerformanceSummary {
    pub topic: String,
    pub total_cycles: u32,
    pub cycles: Vec<CycleStats>,
    pub summary: CycleSummaryStats,
}

/// Summary statistics across all cycles
#[derive(Debug, Serialize, Deserialize)]
pub struct CycleSummaryStats {
    pub total_unique_attributes: u64,
    pub average_efficiency: f64,
    pub peak_efficiency: f64,
    pub efficiency_decline_rate: f64,
    pub total_duration_seconds: f64,
    pub attributes_per_minute: f64,
}

/// Main orchestrator state containing all system information
pub struct OrchestratorState {
    /// Uniqueness tracking and bloom filter management
    uniqueness: UniquenessTracker,

    /// Performance metrics and cost tracking
    pub performance: PerformanceTracker,

    /// Current generation context
    pub context: GenerationContext,

    /// Active producer states
    producers: HashMap<ProcessId, ProducerState>,

    /// System timing information
    start_time: Instant,

    /// CLI mode: iteration limit and current count
    cli_iterations: Option<u32>,
    current_iteration: u32,

    /// Previous iteration stats for delta calculation
    previous_unique_count: u64,

    /// Cycle performance history
    cycle_history: Vec<CycleStats>,

    /// Pending start commands waiting for producers to be ready
    pending_start_commands: HashMap<ProcessId, OrchestratorCommand>,

    /// Default routing strategy from orchestrator args/env (global fallback)
    default_routing_strategy: Option<shared::RoutingStrategy>,
}

/// Current generation task configuration
#[derive(Debug, Clone)]
pub struct GenerationContext {
    /// Current topic being processed
    pub topic: Option<String>,

    /// Base prompt template
    pub prompt_template: String,

    /// Current routing strategy
    pub routing_strategy: shared::RoutingStrategy,

    /// Whether bloom filter deduplication is required
    pub requires_bloom_filter: bool,

    /// Optimization targets
    pub optimization_targets: OptimizationTargets,
}

/// Optimization targets and constraints
#[derive(Debug, Clone)]
pub struct OptimizationTargets {
    pub min_uam: f64,             // Minimum unique attributes per minute
    pub max_cost_per_minute: f64, // Budget constraint
    pub optimization_mode: shared::OptimizationMode,
}

/// State of an individual producer process
#[derive(Debug, Clone)]
pub struct ProducerState {
    pub id: ProcessId,
    pub status: shared::ProcessStatus,
    pub last_activity: Option<Instant>,
    pub last_sync_version: Option<u64>, // Last bloom filter version sent
    pub consecutive_failures: u32,
    pub started_for_current_topic: bool, // Track if producer has been sent Start command for current topic
}

impl OrchestratorState {
    /// Create new orchestrator state
    pub fn new() -> Self {
        Self {
            uniqueness: UniquenessTracker::new(),
            performance: PerformanceTracker::new(),
            context: GenerationContext::default(),
            producers: HashMap::new(),
            start_time: Instant::now(),
            cli_iterations: None,
            current_iteration: 0,
            previous_unique_count: 0,
            cycle_history: Vec::new(),
            pending_start_commands: HashMap::new(),
            default_routing_strategy: None,
        }
    }

    /// Set the default routing strategy from orchestrator args/env
    pub fn set_default_routing_strategy(&mut self, strategy: Option<shared::RoutingStrategy>) {
        self.default_routing_strategy = strategy;
    }

    /// Get the default routing strategy (orchestrator fallback)
    pub fn get_default_routing_strategy(&self) -> Option<&shared::RoutingStrategy> {
        self.default_routing_strategy.as_ref()
    }

    /// Initialize system for a new topic
    pub fn initialize_topic(
        &mut self,
        topic: String,
        producer_count: u32,
        optimization_targets: OptimizationTargets,
    ) -> OrchestratorResult<()> {
        // Reset state for new topic
        self.uniqueness.reset();
        self.performance.reset();

        // Update context
        self.context.topic = Some(topic.clone());
        self.context.optimization_targets = optimization_targets;

        // Initialize producer states
        self.producers.clear();
        for i in 0..producer_count {
            let producer_id = ProcessId::Producer(i + 1);
            let producer_state = ProducerState {
                id: producer_id.clone(),
                status: shared::ProcessStatus::Starting,
                last_activity: None,
                last_sync_version: None,
                consecutive_failures: 0,
                started_for_current_topic: false,
            };
            self.producers.insert(producer_id, producer_state);
        }

        // Determine if bloom filter is needed based on strategy
        self.context.requires_bloom_filter = self.should_use_bloom_filter();

        Ok(())
    }

    /// Process a batch of attributes from a producer
    pub fn process_attribute_batch(
        &mut self,
        producer_id: ProcessId,
        provider_metadata: shared::ProviderMetadata,
        attributes: Vec<String>,
    ) -> OrchestratorResult<ProcessResult> {
        // 1. Update producer activity
        if let Some(producer) = self.producers.get_mut(&producer_id) {
            producer.last_activity = Some(Instant::now());
            producer.status = shared::ProcessStatus::Running;
            producer.consecutive_failures = 0;
        }

        // 2. Check uniqueness
        let unique_attributes = self.uniqueness.filter_unique(attributes.clone())?;
        let unique_count = unique_attributes.len() as u64;
        let total_count = attributes.len() as u64;

        // 3. Track performance
        self.performance.record_contribution(
            producer_id,
            provider_metadata.provider_id,
            unique_count,
            total_count,
            provider_metadata.tokens.clone(),
        );

        // 4. Check if bloom filter needs distribution
        let should_sync = self.uniqueness.should_distribute_bloom_filter();

        Ok(ProcessResult {
            unique_attributes,
            should_sync_bloom: should_sync,
            bloom_version: if should_sync {
                Some(self.uniqueness.get_bloom_version())
            } else {
                None
            },
        })
    }

    /// Get current system metrics
    pub fn get_system_metrics(&self) -> SystemMetrics {
        let performance_stats = self.performance.get_current_stats();

        SystemMetrics {
            uam: performance_stats.overall.uam,
            cost_per_minute: performance_stats.overall.cost_per_minute,
            tokens_per_minute: performance_stats.overall.tokens_per_minute,
            unique_per_dollar: performance_stats.overall.unique_per_dollar,
            unique_per_1k_tokens: performance_stats.overall.unique_per_1k_tokens,
            by_producer: performance_stats
                .by_producer
                .iter()
                .map(|(id, metrics)| (id.to_string(), convert_to_producer_metrics(metrics.clone())))
                .collect(),
            by_provider: performance_stats
                .by_provider
                .iter()
                .map(|(id, metrics)| (*id, convert_to_provider_metrics(metrics.clone())))
                .collect(),
            active_producers: self.producers.len() as u32,
            current_topic: self.context.topic.clone(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Get bloom filter data for distribution
    pub fn get_bloom_filter_data(&self) -> Option<Vec<u8>> {
        if self.context.requires_bloom_filter {
            self.uniqueness.get_bloom_filter_data()
        } else {
            None
        }
    }

    /// Update producer sync status
    pub fn update_producer_sync(&mut self, producer_id: ProcessId, bloom_version: u64) {
        if let Some(producer) = self.producers.get_mut(&producer_id) {
            producer.last_sync_version = Some(bloom_version);
        }
    }

    /// Mark producer as failed
    pub fn mark_producer_failed(&mut self, producer_id: ProcessId) {
        if let Some(producer) = self.producers.get_mut(&producer_id) {
            producer.status = shared::ProcessStatus::Failed;
            producer.consecutive_failures += 1;
        }
    }

    /// Get producers that need sync updates
    pub fn get_producers_needing_sync(&self) -> Vec<ProcessId> {
        if !self.context.requires_bloom_filter {
            return Vec::new();
        }

        let current_version = self.uniqueness.get_bloom_version();
        self.producers
            .iter()
            .filter(|(_, state)| state.last_sync_version.map_or(true, |v| v < current_version))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Determine if bloom filter should be used based on routing strategy
    fn should_use_bloom_filter(&self) -> bool {
        // For now, always use bloom filter unless we have provider-specific partitioning
        // In the future, this could be more sophisticated based on the optimizer's decisions
        true
    }

    // Accessors for testing and debugging
    pub fn total_unique_count(&self) -> u64 {
        self.uniqueness.total_unique_count()
    }

    pub fn active_producer_count(&self) -> usize {
        self.producers.len()
    }

    pub fn current_topic(&self) -> Option<&str> {
        self.context.topic.as_deref()
    }

    /// Get active producer count for public API
    pub fn get_active_producer_count(&self) -> usize {
        self.active_producer_count()
    }

    /// Get unique attribute count for public API
    pub fn get_unique_attribute_count(&self) -> usize {
        self.total_unique_count() as usize
    }

    /// Set CLI iterations limit
    pub fn set_cli_iterations(&mut self, iterations: Option<u32>) {
        self.cli_iterations = iterations;
        self.current_iteration = 0;
    }

    /// Increment iteration count and check if limit reached
    pub fn increment_iteration(&mut self) -> bool {
        self.current_iteration += 1;

        // Calculate cycle statistics
        let current_unique_count = self.uniqueness.total_unique_count();
        let new_values = current_unique_count - self.previous_unique_count;
        let iteration_items = self.uniqueness.get_current_iteration_items();
        let attempted_this_cycle = iteration_items.len() as u64; // Items attempted this cycle

        // Calculate efficiency metrics
        let efficiency = if attempted_this_cycle > 0 {
            (new_values as f64 / attempted_this_cycle as f64) * 100.0
        } else {
            0.0
        };

        let efficiency_delta = if let Some(last_cycle) = self.cycle_history.last() {
            efficiency - last_cycle.efficiency
        } else {
            0.0 // First cycle has no delta
        };

        let duplicate_values = attempted_this_cycle - new_values;

        // Create cycle stats
        let cycle_stats = CycleStats {
            iteration: self.current_iteration,
            total_values: current_unique_count,
            new_values,
            duplicate_values,
            efficiency,
            efficiency_delta,
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_seconds: self.start_time.elapsed().as_secs_f64(),
        };

        // Add to history
        self.cycle_history.push(cycle_stats.clone());

        // Log detailed cycle statistics as trace event
        process_info!(ProcessId::current(), "📊 CYCLE_STATS: iteration={}, total_values={}, new_values={}, duplicate_values={}, efficiency={:.2}%, efficiency_delta={:.2}%",
            cycle_stats.iteration,
            cycle_stats.total_values,
            cycle_stats.new_values,
            cycle_stats.duplicate_values,
            cycle_stats.efficiency,
            cycle_stats.efficiency_delta
        );

        // Log iteration summary
        let iter_str = if let Some(limit) = self.cli_iterations {
            format!("{}/{}", self.current_iteration, limit)
        } else {
            format!("{} (unlimited)", self.current_iteration)
        };

        process_debug!(ProcessId::current(), "");
        process_debug!(
            ProcessId::current(),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );
        process_debug!(ProcessId::current(), "📊 ITERATION {} SUMMARY", iter_str);
        process_debug!(
            ProcessId::current(),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );

        // Get performance metrics
        let metrics = self.get_system_metrics();
        process_debug!(ProcessId::current(), "📈 Performance Metrics:");
        process_debug!(ProcessId::current(), "   • UAM (Unique/Min): {:.2}", metrics.uam);
        process_debug!(
            ProcessId::current(),
            "   • Cost/Minute: ${:.4}",
            metrics.cost_per_minute
        );
        process_debug!(
            ProcessId::current(),
            "   • Total Unique Attributes: {}",
            current_unique_count
        );
        process_debug!(ProcessId::current(), "   • New This Cycle: {}", new_values);
        process_debug!(ProcessId::current(), "   • Duplicates This Cycle: {}", duplicate_values);
        process_debug!(ProcessId::current(), "   • Cycle Efficiency: {:.2}%", efficiency);
        process_debug!(ProcessId::current(), "   • Efficiency Delta: {:.2}%", efficiency_delta);

        // Show unique attributes found in this iteration
        process_debug!(ProcessId::current(), "");
        process_debug!(
            ProcessId::current(),
            "🎯 New Unique Attributes This Iteration ({}):",
            new_values
        );
        if iteration_items.is_empty() {
            process_debug!(ProcessId::current(), "   (No new unique attributes found)");
        } else {
            for (i, attr) in iteration_items.iter().take(10).enumerate() {
                process_debug!(ProcessId::current(), "   {}. {}", i + 1, attr);
            }
            if iteration_items.len() > 10 {
                process_debug!(ProcessId::current(), "   ... and {} more", iteration_items.len() - 10);
            }
        }

        // Update previous count for next cycle
        self.previous_unique_count = current_unique_count;

        // Start next iteration (clear current iteration items)
        self.uniqueness.start_next_iteration();

        process_debug!(
            ProcessId::current(),
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );
        process_debug!(ProcessId::current(), "");

        // Check if limit reached
        if let Some(limit) = self.cli_iterations {
            if self.current_iteration >= limit {
                process_debug!(
                    ProcessId::current(),
                    "🏁 Reached iteration limit: {}/{}",
                    self.current_iteration,
                    limit
                );
                return true; // Limit reached
            }
        }

        false // Continue running
    }

    /// Check if we should stop due to iteration limit
    pub fn should_stop_iterations(&self) -> bool {
        if let Some(limit) = self.cli_iterations {
            self.current_iteration >= limit
        } else {
            false
        }
    }

    /// Get current iteration count
    pub fn get_current_iteration(&self) -> u32 {
        self.current_iteration
    }

    /// Get CLI iterations limit
    pub fn get_cli_iterations(&self) -> Option<u32> {
        self.cli_iterations
    }

    /// Get current iteration items for output
    pub fn get_current_iteration_items(&self) -> Vec<String> {
        self.uniqueness.get_current_iteration_items().to_vec()
    }

    /// Start generation for a topic (compatibility method)
    pub fn start_generation(
        &mut self,
        topic: String,
        optimization_mode: shared::OptimizationMode,
        constraints: shared::GenerationConstraints,
    ) {
        self.context.topic = Some(topic);
        self.context.optimization_targets.optimization_mode = optimization_mode;
        self.context.optimization_targets.max_cost_per_minute = constraints.max_cost_per_minute;
        self.context.optimization_targets.min_uam = constraints.target_uam;
    }

    /// Stop generation
    pub fn stop_generation(&mut self) {
        self.context.topic = None;
        self.producers.clear();
    }

    /// Add a producer to tracking
    pub fn add_producer(&mut self, producer_id: ProcessId, _process_id: u32, status: shared::ProcessStatus) {
        let producer_state = ProducerState {
            id: producer_id.clone(),
            status,
            last_activity: Some(Instant::now()),
            last_sync_version: None,
            consecutive_failures: 0,
            started_for_current_topic: false,
        };
        self.producers.insert(producer_id, producer_state);
    }

    /// Add attributes from producer (compatibility method)
    pub fn add_attributes(
        &mut self,
        producer_id: shared::ProcessId,
        attributes: Vec<String>,
        provider_metadata: &shared::ProviderMetadata,
    ) -> Vec<String> {
        let unique_attributes = match self.uniqueness.filter_unique(attributes.clone()) {
            Ok(attrs) => attrs,
            Err(_) => Vec::new(),
        };

        // Record performance
        let unique_count = unique_attributes.len() as u64;
        let total_count = attributes.len() as u64;

        self.performance.record_contribution(
            producer_id,
            provider_metadata.provider_id,
            unique_count,
            total_count,
            provider_metadata.tokens.clone(),
        );

        unique_attributes
    }

    /// Update producer status
    pub fn update_producer_status(&mut self, producer_id: ProcessId, status: shared::ProcessStatus) {
        if let Some(producer) = self.producers.get_mut(&producer_id) {
            producer.status = status;
        }
    }

    /// Mark producer as started for current topic
    pub fn mark_producer_started(&mut self, producer_id: ProcessId) {
        if let Some(producer) = self.producers.get_mut(&producer_id) {
            producer.started_for_current_topic = true;
        }
    }

    /// Check if producer has been started for current topic
    pub fn is_producer_started(&self, producer_id: &ProcessId) -> bool {
        self.producers.get(producer_id)
            .map(|p| p.started_for_current_topic)
            .unwrap_or(false)
    }

    /// Remove a failed producer from tracking
    pub fn remove_producer(&mut self, producer_id: &ProcessId) {
        self.producers.remove(producer_id);
    }


    /// Get performance statistics
    pub fn get_performance_stats(&self) -> crate::core::performance::PerformanceStats {
        self.performance.get_current_stats().clone()
    }

    /// Generate cycle performance summary for JSON export
    pub fn generate_cycle_performance_summary(&self) -> Option<CyclePerformanceSummary> {
        let topic = self.context.topic.as_ref()?.clone();

        if self.cycle_history.is_empty() {
            return None;
        }

        // Calculate summary statistics
        let total_unique_attributes = self.cycle_history.last()?.total_values;
        let average_efficiency =
            self.cycle_history.iter().map(|c| c.efficiency).sum::<f64>() / self.cycle_history.len() as f64;
        let peak_efficiency = self.cycle_history.iter().map(|c| c.efficiency).fold(0.0, f64::max);

        // Calculate efficiency decline rate (linear regression slope)
        let efficiency_decline_rate = if self.cycle_history.len() >= 2 {
            let n = self.cycle_history.len() as f64;
            let sum_x = (1..=self.cycle_history.len()).sum::<usize>() as f64;
            let sum_y = self.cycle_history.iter().map(|c| c.efficiency).sum::<f64>();
            let sum_xy = self
                .cycle_history
                .iter()
                .enumerate()
                .map(|(i, c)| ((i + 1) as f64) * c.efficiency)
                .sum::<f64>();
            let sum_x2 = (1..=self.cycle_history.len()).map(|i| (i * i) as f64).sum::<f64>();

            (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x)
        } else {
            0.0
        };

        let total_duration = self.start_time.elapsed().as_secs_f64();
        let attributes_per_minute = if total_duration > 0.0 {
            (total_unique_attributes as f64) / (total_duration / 60.0)
        } else {
            0.0
        };

        Some(CyclePerformanceSummary {
            topic,
            total_cycles: self.cycle_history.len() as u32,
            cycles: self.cycle_history.clone(),
            summary: CycleSummaryStats {
                total_unique_attributes,
                average_efficiency,
                peak_efficiency,
                efficiency_decline_rate,
                total_duration_seconds: total_duration,
                attributes_per_minute,
            },
        })
    }

    /// Generate provider performance statistics
    pub fn generate_provider_performance_stats(&self) -> Vec<ProviderPerformanceStats> {
        let performance_stats = self.performance.get_current_stats();
        let total_duration_minutes = self.start_time.elapsed().as_secs_f64() / 60.0;

        performance_stats
            .by_provider
            .iter()
            .map(|(provider_id, metrics)| {
                ProviderPerformanceStats {
                    provider_id: *provider_id,
                    requests_per_minute: metrics.request_rate,
                    unique_attributes_per_minute: metrics.uam,
                    cost_per_minute: metrics.cost_per_minute,
                    efficiency_ratio: metrics.uniqueness_ratio,
                    total_requests: (metrics.request_rate * total_duration_minutes) as u64,
                    total_unique_attributes: (metrics.uam * total_duration_minutes) as u64,
                    total_cost: metrics.cost_per_minute * total_duration_minutes,
                    average_response_time_ms: 0.0, // TODO: Add response time tracking
                    success_rate: 1.0,             // TODO: Add success rate tracking
                }
            })
            .collect()
    }

    /// Export cycle performance to JSON file
    pub async fn export_cycle_performance(
        &self,
        file_system: &dyn crate::traits::FileSystem,
    ) -> OrchestratorResult<()> {
        if let Some(summary) = self.generate_cycle_performance_summary() {
            let json_content = serde_json::to_string_pretty(&summary)?;

            file_system
                .write_file("cycle_performance.json", json_content.as_bytes())
                .await?;
            process_debug!(
                ProcessId::current(),
                "📊 Exported cycle performance to cycle_performance.json"
            );
        }
        Ok(())
    }

    /// Export provider performance to JSON file
    pub async fn export_provider_performance(
        &self,
        file_system: &dyn crate::traits::FileSystem,
    ) -> OrchestratorResult<()> {
        let provider_stats = self.generate_provider_performance_stats();
        let json_content = serde_json::to_string_pretty(&provider_stats)?;

        file_system
            .write_file("provider_performance.json", json_content.as_bytes())
            .await?;
        process_debug!(
            ProcessId::current(),
            "📊 Exported provider performance to provider_performance.json"
        );
        Ok(())
    }
}

/// Result of processing an attribute batch
pub struct ProcessResult {
    pub unique_attributes: Vec<String>,
    pub should_sync_bloom: bool,
    pub bloom_version: Option<u64>,
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self {
            topic: None,
            prompt_template: "Generate unique attributes for: {topic}".to_string(),
            routing_strategy: shared::RoutingStrategy::RoundRobin {
                providers: vec![
                    shared::types::ProviderConfig::with_default_model(ProviderId::OpenAI),
                    shared::types::ProviderConfig::with_default_model(ProviderId::Anthropic),
                    shared::types::ProviderConfig::with_default_model(ProviderId::Gemini),
                ],
            },
            requires_bloom_filter: true,
            optimization_targets: OptimizationTargets {
                min_uam: 5.0,
                max_cost_per_minute: 1.0,
                optimization_mode: shared::OptimizationMode::MaximizeEfficiency,
            },
        }
    }
}

/// Convert internal performance metrics to shared type
fn convert_to_producer_metrics(metrics: crate::core::performance::PerformanceMetrics) -> shared::ProducerMetrics {
    shared::ProducerMetrics {
        uam: metrics.uam,
        tokens_per_minute: metrics.tokens_per_minute,
        cost_per_minute: metrics.cost_per_minute,
        unique_per_dollar: metrics.unique_per_dollar,
        unique_per_1k_tokens: metrics.unique_per_1k_tokens,
        uniqueness_ratio: metrics.uniqueness_ratio,
        status: shared::ProcessStatus::Running, // TODO: Get from producer state
        last_activity: chrono::Utc::now().timestamp() as u64, // TODO: Use real timestamp
    }
}

/// Convert internal provider metrics to shared type
fn convert_to_provider_metrics(metrics: crate::core::performance::PerformanceMetrics) -> shared::ProviderMetrics {
    shared::ProviderMetrics {
        uam: metrics.uam,
        tokens_per_minute: metrics.tokens_per_minute,
        cost_per_minute: metrics.cost_per_minute,
        unique_per_dollar: metrics.unique_per_dollar,
        unique_per_1k_tokens: metrics.unique_per_1k_tokens,
        avg_response_time_ms: 0.0,                 // TODO: Calculate from metadata
        success_rate: 1.0,                         // TODO: Track from requests
        status: shared::ProviderStatus::Available, // TODO: Determine from recent activity
    }
}

impl OrchestratorState {
    /// Queue a start command for a producer (will be sent when producer is ready)
    pub fn queue_start_command(&mut self, producer_id: ProcessId, command: OrchestratorCommand) {
        process_debug!(
            ProcessId::current(),
            "📋 Queuing start command for producer {}",
            producer_id
        );
        self.pending_start_commands.insert(producer_id, command);
    }

    /// Get and remove pending start command for a producer (if any)
    pub fn take_pending_start_command(&mut self, producer_id: &ProcessId) -> Option<OrchestratorCommand> {
        self.pending_start_commands.remove(producer_id)
    }

    /// Check if there are any pending start commands
    pub fn has_pending_start_commands(&self) -> bool {
        !self.pending_start_commands.is_empty()
    }

    /// Get count of pending start commands
    pub fn pending_start_commands_count(&self) -> usize {
        self.pending_start_commands.len()
    }
}
