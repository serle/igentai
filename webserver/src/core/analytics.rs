//! Analytics engine for performance insights
//!
//! Pure business logic for analyzing metrics and generating insights

use chrono::{Duration, Utc};

use crate::core::state::TimestampedMetrics;
use crate::types::{InsightType, OptimizationInsight, TrendDirection};
use shared::SystemMetrics;

/// Analytics engine for processing performance data
pub struct AnalyticsEngine {
    /// Minimum confidence threshold for insights
    confidence_threshold: f64,

    /// Trend analysis window (minutes)
    trend_window_minutes: i64,

    /// Minimum data points for trend analysis
    min_trend_points: usize,
}

impl AnalyticsEngine {
    /// Create new analytics engine
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.7,
            trend_window_minutes: 30,
            min_trend_points: 5,
        }
    }

    /// Create with custom configuration
    pub fn with_config(confidence_threshold: f64, trend_window_minutes: i64) -> Self {
        Self {
            confidence_threshold,
            trend_window_minutes,
            min_trend_points: 5,
        }
    }

    /// Analyze performance and generate insights
    pub fn analyze_performance(
        &self,
        current_metrics: &SystemMetrics,
        history: &[TimestampedMetrics],
    ) -> Vec<OptimizationInsight> {
        let mut insights = Vec::new();

        // Cost efficiency analysis
        if let Some(insight) = self.analyze_cost_efficiency(current_metrics, history) {
            insights.push(insight);
        }

        // Performance trend analysis
        if let Some(insight) = self.analyze_performance_trends(history) {
            insights.push(insight);
        }

        // Provider health analysis
        insights.extend(self.analyze_provider_health(current_metrics));

        // Routing optimization analysis
        if let Some(insight) = self.analyze_routing_efficiency(current_metrics, history) {
            insights.push(insight);
        }

        // Filter by confidence threshold
        insights
            .into_iter()
            .filter(|insight| insight.confidence >= self.confidence_threshold)
            .collect()
    }

    /// Analyze cost efficiency and suggest improvements
    fn analyze_cost_efficiency(
        &self,
        current: &SystemMetrics,
        history: &[TimestampedMetrics],
    ) -> Option<OptimizationInsight> {
        if current.cost_per_minute <= 0.0 || current.uam <= 0.0 {
            return None;
        }

        let current_efficiency = current.unique_per_dollar;

        // Compare with historical average
        let historical_efficiency = self.calculate_average_efficiency(history);

        if let Some(avg_efficiency) = historical_efficiency {
            let efficiency_change = current_efficiency - avg_efficiency;
            let relative_change = efficiency_change / avg_efficiency;

            if relative_change < -0.15 {
                // Efficiency dropped by 15% or more
                Some(OptimizationInsight {
                    insight_type: InsightType::CostEfficiency,
                    title: "Cost Efficiency Declining".to_string(),
                    description: format!(
                        "Cost efficiency has decreased by {:.1}%. Current: {:.2} attributes per dollar, Average: {:.2}",
                        relative_change.abs() * 100.0,
                        current_efficiency,
                        avg_efficiency
                    ),
                    confidence: 0.85,
                    impact_score: relative_change.abs(),
                    recommendation: Some(
                        "Consider adjusting routing strategy or provider selection to improve cost efficiency"
                            .to_string(),
                    ),
                    timestamp: Utc::now().timestamp() as u64,
                })
            } else if relative_change > 0.15 {
                // Efficiency improved significantly
                Some(OptimizationInsight {
                    insight_type: InsightType::CostEfficiency,
                    title: "Cost Efficiency Improving".to_string(),
                    description: format!(
                        "Cost efficiency has improved by {:.1}%. Current: {:.2} attributes per dollar",
                        relative_change * 100.0,
                        current_efficiency
                    ),
                    confidence: 0.8,
                    impact_score: relative_change,
                    recommendation: None,
                    timestamp: Utc::now().timestamp() as u64,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Analyze performance trends
    fn analyze_performance_trends(&self, history: &[TimestampedMetrics]) -> Option<OptimizationInsight> {
        if history.len() < self.min_trend_points {
            return None;
        }

        // Get recent data within trend window
        let cutoff_time = Utc::now() - Duration::minutes(self.trend_window_minutes);
        let recent_data: Vec<_> = history.iter().filter(|tm| tm.timestamp > cutoff_time).collect();

        if recent_data.len() < self.min_trend_points {
            return None;
        }

        // Calculate UAM trend
        let uam_trend = self.calculate_trend(&recent_data, |tm| tm.metrics.uam);

        if uam_trend.slope.abs() < 0.1 {
            return None; // No significant trend
        }

        let trend_direction = if uam_trend.slope > 0.0 {
            TrendDirection::Up
        } else {
            TrendDirection::Down
        };

        let confidence = (uam_trend.r_squared * 0.9).min(0.95); // Scale R² to confidence

        if confidence < self.confidence_threshold {
            return None;
        }

        Some(OptimizationInsight {
            insight_type: InsightType::PerformanceOptimization,
            title: format!("UAM Trend: {trend_direction:?}"),
            description: format!(
                "Unique attributes per minute is trending {} with slope {:.3} over the last {} minutes",
                match trend_direction {
                    TrendDirection::Up => "upward",
                    TrendDirection::Down => "downward",
                    TrendDirection::Stable => "stable",
                },
                uam_trend.slope,
                self.trend_window_minutes
            ),
            confidence,
            impact_score: uam_trend.slope.abs(),
            recommendation: match trend_direction {
                TrendDirection::Down => {
                    Some("Consider investigating recent changes or scaling up producers".to_string())
                }
                TrendDirection::Up => {
                    Some("Current strategy is working well, consider maintaining current settings".to_string())
                }
                TrendDirection::Stable => None,
            },
            timestamp: Utc::now().timestamp() as u64,
        })
    }

    /// Analyze provider health and performance
    fn analyze_provider_health(&self, current: &SystemMetrics) -> Vec<OptimizationInsight> {
        let mut insights = Vec::new();

        for (provider_id, provider_metrics) in &current.by_provider {
            // Check success rate
            if provider_metrics.success_rate < 0.8 {
                insights.push(OptimizationInsight {
                    insight_type: InsightType::ProviderHealth,
                    title: format!("{provider_id:?} Provider Issues"),
                    description: format!(
                        "{:?} has a low success rate of {:.1}% with average response time of {:.0}ms",
                        provider_id,
                        provider_metrics.success_rate * 100.0,
                        provider_metrics.avg_response_time_ms
                    ),
                    confidence: 0.9,
                    impact_score: 1.0 - provider_metrics.success_rate,
                    recommendation: Some(format!(
                        "Consider reducing load on {provider_id:?} or checking API key/rate limits"
                    )),
                    timestamp: Utc::now().timestamp() as u64,
                });
            }

            // Check response time
            if provider_metrics.avg_response_time_ms > 10000.0 {
                // > 10 seconds
                insights.push(OptimizationInsight {
                    insight_type: InsightType::ProviderHealth,
                    title: format!("{provider_id:?} Slow Response"),
                    description: format!(
                        "{:?} has slow response times averaging {:.0}ms",
                        provider_id, provider_metrics.avg_response_time_ms
                    ),
                    confidence: 0.8,
                    impact_score: (provider_metrics.avg_response_time_ms / 10000.0).min(1.0),
                    recommendation: Some(format!(
                        "Consider reducing concurrent requests to {provider_id:?} or checking network connectivity"
                    )),
                    timestamp: Utc::now().timestamp() as u64,
                });
            }
        }

        insights
    }

    /// Analyze routing strategy effectiveness
    fn analyze_routing_efficiency(
        &self,
        current: &SystemMetrics,
        #[allow(unused_variables)] history: &[TimestampedMetrics],
    ) -> Option<OptimizationInsight> {
        if current.by_provider.len() < 2 {
            return None; // Need multiple providers for routing analysis
        }

        // Calculate provider efficiency variance
        let efficiencies: Vec<f64> = current
            .by_provider
            .values()
            .map(|metrics| metrics.unique_per_dollar)
            .collect();

        let mean_efficiency = efficiencies.iter().sum::<f64>() / efficiencies.len() as f64;
        let variance =
            efficiencies.iter().map(|e| (e - mean_efficiency).powi(2)).sum::<f64>() / efficiencies.len() as f64;

        let coefficient_of_variation = if mean_efficiency > 0.0 {
            variance.sqrt() / mean_efficiency
        } else {
            0.0
        };

        // High variance suggests routing could be optimized
        if coefficient_of_variation > 0.3 {
            Some(OptimizationInsight {
                insight_type: InsightType::RoutingStrategy,
                title: "Routing Optimization Opportunity".to_string(),
                description: format!(
                    "Provider efficiency varies significantly (CV: {:.2}). Consider weighted routing based on efficiency.",
                    coefficient_of_variation
                ),
                confidence: 0.75,
                impact_score: coefficient_of_variation,
                recommendation: Some(
                    "Consider switching to weighted routing strategy to favor more efficient providers".to_string(),
                ),
                timestamp: Utc::now().timestamp() as u64,
            })
        } else {
            None
        }
    }

    /// Calculate average efficiency from history
    fn calculate_average_efficiency(&self, history: &[TimestampedMetrics]) -> Option<f64> {
        if history.is_empty() {
            return None;
        }

        let sum: f64 = history.iter().map(|tm| tm.metrics.unique_per_dollar).sum();

        Some(sum / history.len() as f64)
    }

    /// Calculate linear trend for a metric
    fn calculate_trend<F>(&self, data: &[&TimestampedMetrics], extractor: F) -> TrendResult
    where
        F: Fn(&TimestampedMetrics) -> f64,
    {
        if data.len() < 2 {
            return TrendResult {
                slope: 0.0,
                r_squared: 0.0,
            };
        }

        // Convert timestamps to minutes since first data point
        let first_time = data[0].timestamp;
        let x_values: Vec<f64> = data
            .iter()
            .map(|tm| (tm.timestamp - first_time).num_minutes() as f64)
            .collect();

        let y_values: Vec<f64> = data.iter().map(|tm| extractor(tm)).collect();

        // Simple linear regression
        let n = data.len() as f64;
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = y_values.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(&y_values).map(|(x, y)| x * y).sum();
        let sum_x_squared: f64 = x_values.iter().map(|x| x * x).sum();
        #[allow(unused_variables)]
        let sum_y_squared: f64 = y_values.iter().map(|y| y * y).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x_squared - sum_x * sum_x);

        // Calculate R²
        let y_mean = sum_y / n;
        let ss_tot: f64 = y_values.iter().map(|y| (y - y_mean).powi(2)).sum();
        let ss_res: f64 = x_values
            .iter()
            .zip(&y_values)
            .map(|(x, y)| {
                let predicted = slope * x + (sum_y - slope * sum_x) / n;
                (y - predicted).powi(2)
            })
            .sum();

        let r_squared = if ss_tot > 0.0 { 1.0 - (ss_res / ss_tot) } else { 0.0 };

        TrendResult { slope, r_squared }
    }
}

/// Result of trend analysis
#[derive(Debug, Clone)]
struct TrendResult {
    slope: f64,
    r_squared: f64,
}

impl Default for AnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Test imports moved to individual tests as needed

    #[test]
    fn test_analytics_engine_creation() {
        let engine = AnalyticsEngine::new();
        assert_eq!(engine.confidence_threshold, 0.7);
        assert_eq!(engine.trend_window_minutes, 30);
    }

    #[test]
    fn test_cost_efficiency_analysis() {
        let engine = AnalyticsEngine::new();

        let metrics = SystemMetrics {
            uam: 10.0,
            cost_per_minute: 0.5,
            unique_per_dollar: 20.0,
            tokens_per_minute: 1000.0,
            unique_per_1k_tokens: 10.0,
            by_producer: std::collections::HashMap::new(),
            by_provider: std::collections::HashMap::new(),
            active_producers: 1,
            current_topic: Some("test".to_string()),
            uptime_seconds: 3600,
            last_updated: Utc::now().timestamp() as u64,
        };

        let insights = engine.analyze_performance(&metrics, &[]);
        // Should not generate insights without history
        assert!(insights.is_empty());
    }
}
