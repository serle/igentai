//! Performance tracking with time-bucket metrics
//! 
//! This module tracks the three key metrics: UAM (Unique Attributes per Minute),
//! token usage, and cost across rolling time windows for optimization decisions.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use shared::{ProviderId, TokenUsage};

/// Tracks performance metrics over rolling time windows
pub struct PerformanceTracker {
    /// Time buckets for the last 5 minutes (30-second buckets)
    time_buckets: VecDeque<TimeBucket>,
    
    /// Current aggregated performance statistics
    current_stats: PerformanceStats,
    
    /// Cost model for each provider
    provider_costs: HashMap<ProviderId, CostModel>,
    
    /// When we last recalculated statistics
    last_stats_update: Instant,
    
    /// Configuration
    bucket_duration: Duration,
    max_buckets: usize,
}

/// Performance metrics for a 30-second time bucket
#[derive(Debug, Clone)]
pub struct TimeBucket {
    pub start_time: Instant,
    pub end_time: Instant,
    
    /// Metrics by producer
    pub producer_metrics: HashMap<shared::ProcessId, BucketMetrics>,
    
    /// Metrics by provider
    pub provider_metrics: HashMap<ProviderId, BucketMetrics>,
    
    /// Total metrics for this bucket
    pub total_metrics: BucketMetrics,
}

/// Core metrics tracked in each bucket
#[derive(Debug, Clone, Default)]
pub struct BucketMetrics {
    pub unique_attributes: u64,
    pub total_attributes: u64,
    pub tokens_used: TokenUsage,
    pub cost_usd: f64,
    pub request_count: u64,
}

/// Cost model for a provider
#[derive(Debug, Clone)]
pub struct CostModel {
    pub input_cost_per_1k: f64,    // USD per 1K input tokens
    pub output_cost_per_1k: f64,   // USD per 1K output tokens
    pub model_name: String,
}

/// Current performance statistics (last 5 minutes)
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub overall: PerformanceMetrics,
    pub by_producer: HashMap<shared::ProcessId, PerformanceMetrics>,
    pub by_provider: HashMap<ProviderId, PerformanceMetrics>,
    pub efficiency: EfficiencyMetrics,
    pub trends: TrendMetrics,
}

/// Individual performance metrics
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub uam: f64,                    // Unique attributes per minute
    pub tokens_per_minute: f64,
    pub cost_per_minute: f64,
    pub unique_per_dollar: f64,
    pub unique_per_1k_tokens: f64,
    pub uniqueness_ratio: f64,       // unique/total ratio
    pub request_rate: f64,           // requests per minute
}

/// Efficiency comparison metrics
#[derive(Debug, Clone, Default)]
pub struct EfficiencyMetrics {
    pub best_cost_efficiency: Option<(ProviderId, f64)>,     // (provider, unique/dollar)
    pub best_token_efficiency: Option<(ProviderId, f64)>,    // (provider, unique/1k tokens)
    pub best_uam: Option<(ProviderId, f64)>,                 // (provider, UAM)
    pub overall_efficiency_score: f64,                        // Combined efficiency score
}

/// Trend analysis over time
#[derive(Debug, Clone, Default)]
pub struct TrendMetrics {
    pub uam_trend: TrendDirection,
    pub cost_trend: TrendDirection,
    pub efficiency_trend: TrendDirection,
    pub stability_score: f64,        // How stable the metrics are (0.0-1.0)
}

#[derive(Debug, Clone, Default)]
pub enum TrendDirection {
    #[default]
    Stable,
    Improving,
    Declining,
}

impl PerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            time_buckets: VecDeque::new(),
            current_stats: PerformanceStats::default(),
            provider_costs: Self::default_cost_models(),
            last_stats_update: Instant::now(),
            bucket_duration: Duration::from_secs(30),    // 30-second buckets
            max_buckets: 10,                             // 5 minutes of history
        }
    }
    
    /// Reset for new topic
    pub fn reset(&mut self) {
        self.time_buckets.clear();
        self.current_stats = PerformanceStats::default();
        self.last_stats_update = Instant::now();
    }
    
    /// Record a contribution from a producer
    pub fn record_contribution(
        &mut self,
        producer_id: shared::ProcessId,
        provider_id: ProviderId,
        unique_count: u64,
        total_count: u64,
        tokens: TokenUsage,
    ) {
        let now = Instant::now();
        
        // Calculate cost for this contribution
        let cost = self.calculate_cost(&provider_id, &tokens);
        
        // Create bucket metrics
        let metrics = BucketMetrics {
            unique_attributes: unique_count,
            total_attributes: total_count,
            tokens_used: tokens,
            cost_usd: cost,
            request_count: 1,
        };
        
        // Get or create current bucket
        let current_bucket = self.get_or_create_current_bucket(now);
        
        // Update producer metrics
        current_bucket.producer_metrics
            .entry(producer_id)
            .and_modify(|m| m.add(&metrics))
            .or_insert(metrics.clone());
        
        // Update provider metrics
        current_bucket.provider_metrics
            .entry(provider_id)
            .and_modify(|m| m.add(&metrics))
            .or_insert(metrics.clone());
        
        // Update total metrics
        current_bucket.total_metrics.add(&metrics);
        
        // Recalculate statistics if enough time has passed
        if now.duration_since(self.last_stats_update) > Duration::from_secs(5) {
            self.recalculate_stats();
            self.last_stats_update = now;
        }
    }
    
    /// Get current performance statistics
    pub fn get_current_stats(&self) -> &PerformanceStats {
        &self.current_stats
    }
    
    /// Force recalculation of statistics
    pub fn recalculate_stats(&mut self) {
        self.cleanup_old_buckets();
        
        if self.time_buckets.is_empty() {
            self.current_stats = PerformanceStats::default();
            return;
        }
        
        // Calculate duration covered by buckets
        let duration_minutes = self.calculate_duration_minutes();
        
        // Calculate overall metrics
        self.current_stats.overall = self.calculate_overall_metrics(duration_minutes);
        
        // Calculate per-producer metrics
        self.current_stats.by_producer = self.calculate_producer_metrics(duration_minutes);
        
        // Calculate per-provider metrics
        self.current_stats.by_provider = self.calculate_provider_metrics(duration_minutes);
        
        // Calculate efficiency metrics
        self.current_stats.efficiency = self.calculate_efficiency_metrics();
        
        // Calculate trend metrics
        self.current_stats.trends = self.calculate_trend_metrics();
    }
    
    /// Get or create the current time bucket
    fn get_or_create_current_bucket(&mut self, now: Instant) -> &mut TimeBucket {
        // Check if we need a new bucket
        let needs_new_bucket = self.time_buckets.is_empty() || 
            self.time_buckets.back().unwrap().end_time <= now;
        
        if needs_new_bucket {
            let start_time = now;
            let end_time = now + self.bucket_duration;
            
            let bucket = TimeBucket {
                start_time,
                end_time,
                producer_metrics: HashMap::new(),
                provider_metrics: HashMap::new(),
                total_metrics: BucketMetrics::default(),
            };
            
            self.time_buckets.push_back(bucket);
            
            // Cleanup old buckets
            if self.time_buckets.len() > self.max_buckets {
                self.time_buckets.pop_front();
            }
        }
        
        self.time_buckets.back_mut().unwrap()
    }
    
    /// Remove buckets older than our tracking window
    fn cleanup_old_buckets(&mut self) {
        let cutoff_time = Instant::now() - Duration::from_secs(300); // 5 minutes
        self.time_buckets.retain(|bucket| bucket.start_time > cutoff_time);
    }
    
    /// Calculate duration covered by current buckets
    fn calculate_duration_minutes(&self) -> f64 {
        if self.time_buckets.is_empty() {
            return 1.0; // Avoid division by zero
        }
        
        let oldest = self.time_buckets.front().unwrap().start_time;
        let newest = self.time_buckets.back().unwrap().end_time;
        let duration = newest.duration_since(oldest);
        
        duration.as_secs_f64() / 60.0
    }
    
    /// Calculate overall metrics across all buckets
    fn calculate_overall_metrics(&self, duration_minutes: f64) -> PerformanceMetrics {
        let mut total = BucketMetrics::default();
        
        for bucket in &self.time_buckets {
            total.add(&bucket.total_metrics);
        }
        
        self.metrics_from_bucket_data(&total, duration_minutes)
    }
    
    /// Calculate per-producer metrics
    fn calculate_producer_metrics(&self, duration_minutes: f64) -> HashMap<shared::ProcessId, PerformanceMetrics> {
        let mut producer_totals: HashMap<shared::ProcessId, BucketMetrics> = HashMap::new();
        
        for bucket in &self.time_buckets {
            for (producer_id, metrics) in &bucket.producer_metrics {
                producer_totals.entry(producer_id.clone())
                    .and_modify(|total| total.add(metrics))
                    .or_insert(metrics.clone());
            }
        }
        
        producer_totals.into_iter()
            .map(|(id, metrics)| (id, self.metrics_from_bucket_data(&metrics, duration_minutes)))
            .collect()
    }
    
    /// Calculate per-provider metrics
    fn calculate_provider_metrics(&self, duration_minutes: f64) -> HashMap<ProviderId, PerformanceMetrics> {
        let mut provider_totals: HashMap<ProviderId, BucketMetrics> = HashMap::new();
        
        for bucket in &self.time_buckets {
            for (provider_id, metrics) in &bucket.provider_metrics {
                provider_totals.entry(*provider_id)
                    .and_modify(|total| total.add(metrics))
                    .or_insert(metrics.clone());
            }
        }
        
        provider_totals.into_iter()
            .map(|(id, metrics)| (id, self.metrics_from_bucket_data(&metrics, duration_minutes)))
            .collect()
    }
    
    /// Convert bucket data to performance metrics
    fn metrics_from_bucket_data(&self, data: &BucketMetrics, duration_minutes: f64) -> PerformanceMetrics {
        PerformanceMetrics {
            uam: data.unique_attributes as f64 / duration_minutes,
            tokens_per_minute: data.tokens_used.total() as f64 / duration_minutes,
            cost_per_minute: data.cost_usd / duration_minutes,
            unique_per_dollar: if data.cost_usd > 0.0 {
                data.unique_attributes as f64 / data.cost_usd
            } else { 0.0 },
            unique_per_1k_tokens: if data.tokens_used.total() > 0 {
                (data.unique_attributes as f64 / data.tokens_used.total() as f64) * 1000.0
            } else { 0.0 },
            uniqueness_ratio: if data.total_attributes > 0 {
                data.unique_attributes as f64 / data.total_attributes as f64
            } else { 0.0 },
            request_rate: data.request_count as f64 / duration_minutes,
        }
    }
    
    /// Calculate efficiency comparison metrics
    fn calculate_efficiency_metrics(&self) -> EfficiencyMetrics {
        let mut best_cost_efficiency: Option<(ProviderId, f64)> = None;
        let mut best_token_efficiency: Option<(ProviderId, f64)> = None;
        let mut best_uam: Option<(ProviderId, f64)> = None;
        let mut total_efficiency_score = 0.0;
        
        for (provider_id, metrics) in &self.current_stats.by_provider {
            // Cost efficiency
            if best_cost_efficiency.is_none() || 
               metrics.unique_per_dollar > best_cost_efficiency.unwrap().1 {
                best_cost_efficiency = Some((*provider_id, metrics.unique_per_dollar));
            }
            
            // Token efficiency
            if best_token_efficiency.is_none() || 
               metrics.unique_per_1k_tokens > best_token_efficiency.unwrap().1 {
                best_token_efficiency = Some((*provider_id, metrics.unique_per_1k_tokens));
            }
            
            // UAM
            if best_uam.is_none() || metrics.uam > best_uam.unwrap().1 {
                best_uam = Some((*provider_id, metrics.uam));
            }
            
            // Overall efficiency score (weighted combination)
            total_efficiency_score += metrics.unique_per_dollar * 0.4 + 
                                     metrics.unique_per_1k_tokens * 0.3 + 
                                     metrics.uam * 0.3;
        }
        
        EfficiencyMetrics {
            best_cost_efficiency,
            best_token_efficiency,
            best_uam,
            overall_efficiency_score: total_efficiency_score,
        }
    }
    
    /// Calculate trend metrics
    fn calculate_trend_metrics(&self) -> TrendMetrics {
        if self.time_buckets.len() < 4 {
            return TrendMetrics::default(); // Not enough data for trends
        }
        
        // Compare first half vs second half of buckets
        let mid = self.time_buckets.len() / 2;
        let buckets_vec: Vec<&TimeBucket> = self.time_buckets.iter().collect();
        let (first_half, second_half) = buckets_vec.split_at(mid);
        
        let first_total = self.sum_bucket_metrics(first_half);
        let second_total = self.sum_bucket_metrics(second_half);
        
        let first_duration = first_half.len() as f64 * 0.5; // 30s buckets = 0.5 min
        let second_duration = second_half.len() as f64 * 0.5;
        
        let first_metrics = self.metrics_from_bucket_data(&first_total, first_duration);
        let second_metrics = self.metrics_from_bucket_data(&second_total, second_duration);
        
        TrendMetrics {
            uam_trend: self.calculate_trend_direction(first_metrics.uam, second_metrics.uam),
            cost_trend: self.calculate_trend_direction(second_metrics.cost_per_minute, first_metrics.cost_per_minute), // Inverted: lower cost is better
            efficiency_trend: self.calculate_trend_direction(first_metrics.unique_per_dollar, second_metrics.unique_per_dollar),
            stability_score: self.calculate_stability_score(&first_metrics, &second_metrics),
        }
    }
    
    /// Calculate cost for a token usage
    fn calculate_cost(&self, provider_id: &ProviderId, tokens: &TokenUsage) -> f64 {
        if let Some(cost_model) = self.provider_costs.get(provider_id) {
            let input_cost = (tokens.input_tokens as f64 / 1000.0) * cost_model.input_cost_per_1k;
            let output_cost = (tokens.output_tokens as f64 / 1000.0) * cost_model.output_cost_per_1k;
            input_cost + output_cost
        } else {
            0.0 // Unknown provider
        }
    }
    
    /// Default cost models for providers
    fn default_cost_models() -> HashMap<ProviderId, CostModel> {
        let mut models = HashMap::new();
        
        models.insert(
            ProviderId::OpenAI,
            CostModel {
                input_cost_per_1k: 0.03,   // GPT-4 pricing
                output_cost_per_1k: 0.06,
                model_name: "gpt-4".to_string(),
            }
        );
        
        models.insert(
            ProviderId::Anthropic,
            CostModel {
                input_cost_per_1k: 0.015,  // Claude 3 pricing
                output_cost_per_1k: 0.075,
                model_name: "claude-3-opus".to_string(),
            }
        );
        
        models.insert(
            ProviderId::Gemini,
            CostModel {
                input_cost_per_1k: 0.00125, // Gemini Pro pricing
                output_cost_per_1k: 0.00375,
                model_name: "gemini-pro".to_string(),
            }
        );
        
        models.insert(
            ProviderId::Random,
            CostModel {
                input_cost_per_1k: 0.001,   // Test pricing - low cost
                output_cost_per_1k: 0.001,  // Test pricing - low cost
                model_name: "random-test".to_string(),
            }
        );
        
        models
    }
    
    // Helper methods for trend calculation
    fn sum_bucket_metrics(&self, buckets: &[&TimeBucket]) -> BucketMetrics {
        let mut total = BucketMetrics::default();
        for bucket in buckets {
            total.add(&bucket.total_metrics);
        }
        total
    }
    
    fn calculate_trend_direction(&self, old_value: f64, new_value: f64) -> TrendDirection {
        let change_percent = if old_value > 0.0 {
            (new_value - old_value) / old_value
        } else {
            0.0
        };
        
        if change_percent > 0.05 {
            TrendDirection::Improving
        } else if change_percent < -0.05 {
            TrendDirection::Declining
        } else {
            TrendDirection::Stable
        }
    }
    
    fn calculate_stability_score(&self, first: &PerformanceMetrics, second: &PerformanceMetrics) -> f64 {
        // Calculate coefficient of variation for key metrics
        let uam_variance = (first.uam - second.uam).abs() / ((first.uam + second.uam) / 2.0 + 0.001);
        let cost_variance = (first.cost_per_minute - second.cost_per_minute).abs() / ((first.cost_per_minute + second.cost_per_minute) / 2.0 + 0.001);
        
        // Lower variance = higher stability (inverted and clamped to 0-1)
        let avg_variance = (uam_variance + cost_variance) / 2.0;
        (1.0 - avg_variance).max(0.0).min(1.0)
    }
}

impl BucketMetrics {
    /// Add another bucket's metrics to this one
    fn add(&mut self, other: &BucketMetrics) {
        self.unique_attributes += other.unique_attributes;
        self.total_attributes += other.total_attributes;
        self.tokens_used.input_tokens += other.tokens_used.input_tokens;
        self.tokens_used.output_tokens += other.tokens_used.output_tokens;
        self.cost_usd += other.cost_usd;
        self.request_count += other.request_count;
    }
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            overall: PerformanceMetrics::default(),
            by_producer: HashMap::new(),
            by_provider: HashMap::new(),
            efficiency: EfficiencyMetrics::default(),
            trends: TrendMetrics::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_performance_tracking() {
        let mut tracker = PerformanceTracker::new();
        
        // Record some contributions
        tracker.record_contribution(
            shared::ProcessId::Producer(1),
            ProviderId::OpenAI,
            10, // unique
            15, // total
            TokenUsage { input_tokens: 100, output_tokens: 200 }
        );
        
        tracker.recalculate_stats();
        let stats = tracker.get_current_stats();
        
        assert!(stats.overall.uam > 0.0);
        assert!(stats.overall.cost_per_minute > 0.0);
        assert!(stats.overall.tokens_per_minute > 0.0);
        assert_eq!(stats.overall.uniqueness_ratio, 10.0 / 15.0);
    }
    
    #[test]
    fn test_cost_calculation() {
        let tracker = PerformanceTracker::new();
        let tokens = TokenUsage { input_tokens: 1000, output_tokens: 500 };
        
        let cost = tracker.calculate_cost(&ProviderId::OpenAI, &tokens);
        assert!(cost > 0.0);
        
        // Should be: (1000/1000 * 0.03) + (500/1000 * 0.06) = 0.03 + 0.03 = 0.06
        assert!((cost - 0.06).abs() < 0.001);
    }
    
    #[test]
    fn test_reset_functionality() {
        let mut tracker = PerformanceTracker::new();
        
        // Add some data
        tracker.record_contribution(
            shared::ProcessId::Producer(1),
            ProviderId::Anthropic,
            5, 10,
            TokenUsage { input_tokens: 50, output_tokens: 100 }
        );
        
        tracker.recalculate_stats();
        assert!(tracker.get_current_stats().overall.uam > 0.0);
        
        // Reset should clear everything
        tracker.reset();
        assert_eq!(tracker.get_current_stats().overall.uam, 0.0);
        assert!(tracker.time_buckets.is_empty());
    }
}