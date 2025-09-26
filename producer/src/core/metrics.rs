//! Performance metrics calculation and tracking

use crate::types::{ApiResponse, ProcessedAttribute, ProducerMetrics};
use chrono::Utc;
use shared::ProviderId;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Performance metrics calculator and tracker
pub struct Metrics {
    /// Current metrics state
    current_metrics: ProducerMetrics,

    /// Historical response times for moving averages
    response_times: VecDeque<Duration>,

    /// Per-provider statistics
    provider_stats: HashMap<ProviderId, ProviderStats>,

    /// Start time for uptime calculation
    start_time: Option<Instant>,

    /// Total cost accumulated
    total_cost: f64,

    /// History window size for moving averages
    history_window: usize,
}

/// Per-provider statistics
#[derive(Debug, Clone, Default)]
struct ProviderStats {
    requests_sent: u64,
    responses_received: u64,
    total_response_time_ms: u64,
    total_tokens_used: u64,
    total_cost: f64,
    success_count: u64,
    error_count: u64,
}

impl ProviderStats {
    fn success_rate(&self) -> f64 {
        let total = self.success_count + self.error_count;
        if total == 0 {
            0.0
        } else {
            self.success_count as f64 / total as f64
        }
    }

    fn avg_response_time_ms(&self) -> f64 {
        if self.responses_received == 0 {
            0.0
        } else {
            self.total_response_time_ms as f64 / self.responses_received as f64
        }
    }
}

impl Metrics {
    /// Create new metrics tracker
    pub fn new() -> Self {
        Self::with_history_window(100) // Keep last 100 response times
    }

    /// Create metrics tracker with custom history window
    pub fn with_history_window(window_size: usize) -> Self {
        Self {
            current_metrics: ProducerMetrics::new(),
            response_times: VecDeque::with_capacity(window_size),
            provider_stats: HashMap::new(),
            start_time: None,
            total_cost: 0.0,
            history_window: window_size,
        }
    }

    /// Start metrics tracking
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.current_metrics.last_updated = Utc::now();
        info!("Started metrics tracking");
    }

    /// Record an API request being sent
    pub fn record_request_sent(&mut self, provider: ProviderId) {
        self.current_metrics.requests_sent += 1;

        let stats = self.provider_stats.entry(provider).or_default();
        stats.requests_sent += 1;

        debug!("Recorded request sent to {:?}", provider);
        self.update_timestamp();
    }

    /// Record an API response received
    pub fn record_response_received(&mut self, response: &ApiResponse) {
        self.current_metrics.responses_received += 1;
        self.current_metrics.total_tokens_used += response.tokens_used.total();

        // Update response time tracking
        let response_time = Duration::from_millis(response.response_time_ms);
        self.add_response_time(response_time);

        // Update provider-specific stats
        let stats = self.provider_stats.entry(response.provider).or_default();
        stats.responses_received += 1;
        stats.total_response_time_ms += response.response_time_ms;
        stats.total_tokens_used += response.tokens_used.total();

        if response.success {
            stats.success_count += 1;
        } else {
            stats.error_count += 1;
        }

        debug!(
            "Recorded response from {:?}: {}ms, {} tokens (input: {}, output: {}), success: {}",
            response.provider, response.response_time_ms, response.tokens_used.total(),
            response.tokens_used.input_tokens, response.tokens_used.output_tokens, response.success
        );

        self.recalculate_derived_metrics();
        self.update_timestamp();
    }

    /// Record processed attributes
    pub fn record_attributes_processed(&mut self, attributes: &[ProcessedAttribute]) {
        self.current_metrics.attributes_extracted += attributes.len() as u64;

        let unique_count = attributes.iter().filter(|attr| attr.is_unique).count();
        self.current_metrics.unique_attributes += unique_count as u64;

        debug!(
            "Recorded {} attributes processed ({} unique)",
            attributes.len(),
            unique_count
        );
        self.update_timestamp();
    }

    /// Record processing statistics (more efficient approach)
    pub fn record_processing_stats(&mut self, stats: &crate::core::processor::ProcessingStats) {
        self.current_metrics.attributes_extracted += stats.total_extracted as u64;
        self.current_metrics.unique_attributes += stats.new_values.len() as u64;

        debug!(
            "Recorded {} attributes processed ({} new, {} duplicates)",
            stats.total_extracted,
            stats.new_values.len(),
            stats.duplicate_count
        );
        self.update_timestamp();
    }

    /// Record cost incurred
    pub fn record_cost(&mut self, provider: ProviderId, cost: f64) {
        self.total_cost += cost;
        self.current_metrics.total_cost = self.total_cost;

        let stats = self.provider_stats.entry(provider).or_default();
        stats.total_cost += cost;

        debug!("Recorded cost for {:?}: ${:.4}", provider, cost);
        self.update_timestamp();
    }

    /// Get current metrics snapshot
    pub fn get_current_metrics(&self) -> ProducerMetrics {
        let mut metrics = self.current_metrics.clone();

        // Update uptime
        if let Some(start) = self.start_time {
            metrics.uptime_seconds = start.elapsed().as_secs();
        }

        metrics
    }

    /// Get per-provider metrics
    pub fn get_provider_metrics(&self) -> HashMap<ProviderId, ProviderMetrics> {
        self.provider_stats
            .iter()
            .map(|(provider, stats)| {
                (
                    *provider,
                    ProviderMetrics {
                        requests_sent: stats.requests_sent,
                        responses_received: stats.responses_received,
                        total_tokens_used: stats.total_tokens_used,
                        total_cost: stats.total_cost,
                        avg_response_time_ms: stats.avg_response_time_ms(),
                        success_rate: stats.success_rate(),
                    },
                )
            })
            .collect()
    }

    /// Get performance insights
    pub fn get_insights(&self) -> Vec<PerformanceInsight> {
        let mut insights = Vec::new();

        // Check overall success rate
        if self.current_metrics.success_rate < 0.8 {
            insights.push(PerformanceInsight {
                category: InsightCategory::Reliability,
                message: format!("Success rate is low: {:.1}%", self.current_metrics.success_rate * 100.0),
                severity: InsightSeverity::Warning,
            });
        }

        // Check response times
        if self.current_metrics.avg_response_time_ms > 5000.0 {
            insights.push(PerformanceInsight {
                category: InsightCategory::Performance,
                message: format!(
                    "Average response time is high: {:.0}ms",
                    self.current_metrics.avg_response_time_ms
                ),
                severity: InsightSeverity::Warning,
            });
        }

        // Check cost efficiency
        let efficiency = self.current_metrics.cost_efficiency();
        if efficiency < 10.0 && self.current_metrics.total_cost > 1.0 {
            insights.push(PerformanceInsight {
                category: InsightCategory::Cost,
                message: format!("Cost efficiency is low: {efficiency:.1} attributes per dollar"),
                severity: InsightSeverity::Info,
            });
        }

        // Check provider-specific issues
        for (provider, stats) in &self.provider_stats {
            if stats.success_rate() < 0.7 && stats.requests_sent > 10 {
                insights.push(PerformanceInsight {
                    category: InsightCategory::Provider,
                    message: format!(
                        "{:?} has low success rate: {:.1}%",
                        provider,
                        stats.success_rate() * 100.0
                    ),
                    severity: InsightSeverity::Warning,
                });
            }

            if stats.avg_response_time_ms() > 8000.0 && stats.responses_received > 5 {
                insights.push(PerformanceInsight {
                    category: InsightCategory::Provider,
                    message: format!(
                        "{:?} has slow response times: {:.0}ms",
                        provider,
                        stats.avg_response_time_ms()
                    ),
                    severity: InsightSeverity::Info,
                });
            }
        }

        insights
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        self.current_metrics = ProducerMetrics::new();
        self.response_times.clear();
        self.provider_stats.clear();
        self.start_time = None;
        self.total_cost = 0.0;

        info!("Reset all metrics");
    }

    /// Add response time to moving average calculation
    fn add_response_time(&mut self, response_time: Duration) {
        if self.response_times.len() >= self.history_window {
            self.response_times.pop_front();
        }
        self.response_times.push_back(response_time);
    }

    /// Recalculate derived metrics
    fn recalculate_derived_metrics(&mut self) {
        // Calculate average response time from recent samples
        if !self.response_times.is_empty() {
            let total_ms: u64 = self.response_times.iter().map(|d| d.as_millis() as u64).sum();
            self.current_metrics.avg_response_time_ms = total_ms as f64 / self.response_times.len() as f64;
        }

        // Calculate overall success rate
        let total_success: u64 = self.provider_stats.values().map(|s| s.success_count).sum();
        let total_errors: u64 = self.provider_stats.values().map(|s| s.error_count).sum();
        let total_responses = total_success + total_errors;

        if total_responses > 0 {
            self.current_metrics.success_rate = total_success as f64 / total_responses as f64;
        }
    }

    /// Update timestamp
    fn update_timestamp(&mut self) {
        self.current_metrics.last_updated = Utc::now();
    }
}

/// Per-provider metrics for external reporting
#[derive(Debug, Clone)]
pub struct ProviderMetrics {
    pub requests_sent: u64,
    pub responses_received: u64,
    pub total_tokens_used: u64,
    pub total_cost: f64,
    pub avg_response_time_ms: f64,
    pub success_rate: f64,
}

/// Performance insight for monitoring
#[derive(Debug, Clone)]
pub struct PerformanceInsight {
    pub category: InsightCategory,
    pub message: String,
    pub severity: InsightSeverity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsightCategory {
    Performance,
    Reliability,
    Cost,
    Provider,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsightSeverity {
    Info,
    Warning,
    Error,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_response(provider: ProviderId, success: bool, response_time_ms: u64, tokens: u32) -> ApiResponse {
        crate::types::ApiResponse {
            provider,
            request_id: Uuid::new_v4(),
            content: "Test response".to_string(),
            tokens_used: tokens,
            response_time_ms,
            timestamp: Utc::now(),
            success,
            error_message: if success { None } else { Some("Test error".to_string()) },
        }
    }

    fn create_test_attribute(is_unique: bool) -> ProcessedAttribute {
        crate::types::ProcessedAttribute {
            value: "test@example.com".to_string(),
            source_provider: ProviderId::OpenAI,
            extraction_pattern: "email".to_string(),
            confidence_score: 0.9,
            timestamp: Utc::now(),
            is_unique,
        }
    }

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        assert_eq!(metrics.current_metrics.requests_sent, 0);
        assert_eq!(metrics.current_metrics.responses_received, 0);
        assert_eq!(metrics.history_window, 100);
        assert!(metrics.start_time.is_none());
    }

    #[test]
    fn test_custom_history_window() {
        let metrics = Metrics::with_history_window(50);
        assert_eq!(metrics.history_window, 50);
        assert_eq!(metrics.response_times.capacity(), 50);
    }

    #[test]
    fn test_start_tracking() {
        let mut metrics = Metrics::new();
        metrics.start();

        assert!(metrics.start_time.is_some());

        let current = metrics.get_current_metrics();
        assert!(current.uptime_seconds < 1000); // Should be very small since just started
    }

    #[test]
    fn test_request_tracking() {
        let mut metrics = Metrics::new();

        metrics.record_request_sent(ProviderId::OpenAI);
        metrics.record_request_sent(ProviderId::Anthropic);

        assert_eq!(metrics.current_metrics.requests_sent, 2);

        let provider_stats = metrics.provider_stats.get(&ProviderId::OpenAI).unwrap();
        assert_eq!(provider_stats.requests_sent, 1);
    }

    #[test]
    fn test_response_tracking() {
        let mut metrics = Metrics::new();

        let response = create_test_response(ProviderId::OpenAI, true, 500, 100);
        metrics.record_response_received(&response);

        assert_eq!(metrics.current_metrics.responses_received, 1);
        assert_eq!(metrics.current_metrics.total_tokens_used, 100);
        assert_eq!(metrics.current_metrics.avg_response_time_ms, 500.0);
        assert_eq!(metrics.current_metrics.success_rate, 1.0);
    }

    #[test]
    fn test_response_time_moving_average() {
        let mut metrics = Metrics::with_history_window(3);

        // Add response times: 100ms, 200ms, 300ms
        let responses = vec![
            create_test_response(ProviderId::OpenAI, true, 100, 50),
            create_test_response(ProviderId::OpenAI, true, 200, 50),
            create_test_response(ProviderId::OpenAI, true, 300, 50),
        ];

        for response in responses {
            metrics.record_response_received(&response);
        }

        // Average should be (100 + 200 + 300) / 3 = 200
        assert_eq!(metrics.current_metrics.avg_response_time_ms, 200.0);

        // Add another response (400ms) - should push out 100ms
        let response4 = create_test_response(ProviderId::OpenAI, true, 400, 50);
        metrics.record_response_received(&response4);

        // New average should be (200 + 300 + 400) / 3 = 300
        assert_eq!(metrics.current_metrics.avg_response_time_ms, 300.0);
    }

    #[test]
    fn test_success_rate_calculation() {
        let mut metrics = Metrics::new();

        // Add 3 successful and 1 failed response
        let responses = vec![
            create_test_response(ProviderId::OpenAI, true, 100, 50),
            create_test_response(ProviderId::OpenAI, true, 200, 50),
            create_test_response(ProviderId::OpenAI, true, 300, 50),
            create_test_response(ProviderId::OpenAI, false, 400, 0),
        ];

        for response in responses {
            metrics.record_response_received(&response);
        }

        // Success rate should be 3/4 = 0.75
        assert_eq!(metrics.current_metrics.success_rate, 0.75);
    }

    #[test]
    fn test_attribute_processing() {
        let mut metrics = Metrics::new();

        let attributes = vec![
            create_test_attribute(true),  // unique
            create_test_attribute(false), // not unique
            create_test_attribute(true),  // unique
        ];

        metrics.record_attributes_processed(&attributes);

        assert_eq!(metrics.current_metrics.attributes_extracted, 3);
        assert_eq!(metrics.current_metrics.unique_attributes, 2);
    }

    #[test]
    fn test_cost_tracking() {
        let mut metrics = Metrics::new();

        metrics.record_cost(ProviderId::OpenAI, 1.5);
        metrics.record_cost(ProviderId::Anthropic, 2.0);

        assert_eq!(metrics.current_metrics.total_cost, 3.5);

        let provider_metrics = metrics.get_provider_metrics();
        assert_eq!(provider_metrics[&ProviderId::OpenAI].total_cost, 1.5);
        assert_eq!(provider_metrics[&ProviderId::Anthropic].total_cost, 2.0);
    }

    #[test]
    fn test_provider_specific_metrics() {
        let mut metrics = Metrics::new();

        // Add responses from different providers
        let openai_response = create_test_response(ProviderId::OpenAI, true, 300, 100);
        let anthropic_response = create_test_response(ProviderId::Anthropic, false, 500, 150);

        metrics.record_response_received(&openai_response);
        metrics.record_response_received(&anthropic_response);

        let provider_metrics = metrics.get_provider_metrics();

        let openai_stats = &provider_metrics[&ProviderId::OpenAI];
        assert_eq!(openai_stats.success_rate, 1.0);
        assert_eq!(openai_stats.avg_response_time_ms, 300.0);
        assert_eq!(openai_stats.total_tokens_used, 100);

        let anthropic_stats = &provider_metrics[&ProviderId::Anthropic];
        assert_eq!(anthropic_stats.success_rate, 0.0);
        assert_eq!(anthropic_stats.avg_response_time_ms, 500.0);
        assert_eq!(anthropic_stats.total_tokens_used, 150);
    }

    #[test]
    fn test_performance_insights() {
        let mut metrics = Metrics::new();

        // Create scenario with low success rate
        for _ in 0..7 {
            let response = create_test_response(ProviderId::OpenAI, false, 1000, 50);
            metrics.record_response_received(&response);
        }
        for _ in 0..3 {
            let response = create_test_response(ProviderId::OpenAI, true, 1000, 50);
            metrics.record_response_received(&response);
        }

        let insights = metrics.get_insights();
        assert!(!insights.is_empty());

        let reliability_insights: Vec<_> = insights
            .iter()
            .filter(|i| i.category == InsightCategory::Reliability)
            .collect();
        assert!(!reliability_insights.is_empty());
    }

    #[test]
    fn test_cost_efficiency_calculation() {
        let mut metrics = Metrics::new();

        // Process some unique attributes
        let attributes = vec![
            create_test_attribute(true),
            create_test_attribute(true),
            create_test_attribute(true),
        ];
        metrics.record_attributes_processed(&attributes);

        // Add some cost
        metrics.record_cost(ProviderId::OpenAI, 1.0);

        let current = metrics.get_current_metrics();
        assert_eq!(current.cost_efficiency(), 3.0); // 3 unique attributes / $1.0
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = Metrics::new();
        metrics.start();

        // Add some data
        let response = create_test_response(ProviderId::OpenAI, true, 500, 100);
        metrics.record_response_received(&response);
        metrics.record_cost(ProviderId::OpenAI, 1.0);

        assert_ne!(metrics.current_metrics.responses_received, 0);
        assert_ne!(metrics.current_metrics.total_cost, 0.0);
        assert!(metrics.start_time.is_some());

        // Reset and verify
        metrics.reset();

        assert_eq!(metrics.current_metrics.responses_received, 0);
        assert_eq!(metrics.current_metrics.total_cost, 0.0);
        assert!(metrics.start_time.is_none());
        assert!(metrics.response_times.is_empty());
        assert!(metrics.provider_stats.is_empty());
    }

    #[test]
    fn test_token_efficiency_calculation() {
        let mut metrics = Metrics::new();

        // Process unique attributes and record token usage
        let attributes = vec![create_test_attribute(true); 5]; // 5 unique attributes
        metrics.record_attributes_processed(&attributes);

        let response = create_test_response(ProviderId::OpenAI, true, 500, 2000); // 2000 tokens
        metrics.record_response_received(&response);

        let current = metrics.get_current_metrics();
        // Token efficiency: (5 unique * 1000) / 2000 tokens = 2.5
        assert_eq!(current.token_efficiency(), 2.5);
    }
}
