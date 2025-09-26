//! Optimization logic for intelligent prompt generation and routing strategies
//!
//! This module analyzes performance data and generates optimized prompts and
//! routing strategies to maximize unique attributes per minute while respecting
//! cost and token constraints.

use super::performance::PerformanceStats;
use crate::error::OrchestratorResult;
use shared::{GenerationConfig, OptimizationMode, ProviderId, RoutingStrategy};
use std::collections::HashMap;
use std::cmp;
use std::time::Instant;

/// Optimizer for intelligent prompt generation and routing decisions
pub struct Optimizer {
    /// Historical performance data for trend analysis
    optimization_history: Vec<OptimizationResult>,

    /// Current optimization strategy
    current_strategy: OptimizationStrategy,
}

/// Strategy for optimization decisions
#[derive(Debug, Clone)]
pub enum OptimizationStrategy {
    /// Focus on maximizing unique attributes per minute
    MaximizeUAM,

    /// Focus on cost efficiency (unique per dollar)
    MaximizeCostEfficiency,

    /// Balance multiple objectives
    Balanced {
        uam_weight: f64,
        cost_weight: f64,
        quality_weight: f64,
    },

    /// Custom strategy based on specific targets
    TargetDriven { target_uam: f64, max_cost: f64 },
}

/// Result of an optimization analysis
#[derive(Debug, Clone)]
pub struct OptimizationPlan {
    /// Optimized prompt for the topic
    pub optimized_prompt: String,

    /// Recommended routing strategy
    pub routing_strategy: RoutingStrategy,

    /// Generation configuration adjustments
    pub generation_config: GenerationConfig,

    /// Partitioning strategy recommendation
    pub partitioning_strategy: PartitioningStrategy,

    /// Confidence score for this optimization (0.0-1.0)
    pub confidence: f64,

    /// Rationale explaining the optimization decisions
    pub rationale: String,

    /// Expected performance improvements
    pub expected_improvements: ExpectedImprovements,
}

/// Partitioning strategy for distributing work
#[derive(Debug, Clone)]
pub enum PartitioningStrategy {
    /// No partitioning - all producers use same prompt
    None,

    /// Semantic partitioning - divide topic into categories
    Semantic { categories: Vec<String> },

    /// Attribute type partitioning - focus on different types
    AttributeType { types: Vec<String> },

    /// Provider-specific partitioning - leverage strengths
    ProviderSpecific {
        specializations: HashMap<ProviderId, String>,
    },
}

/// Expected improvements from optimization
#[derive(Debug, Clone)]
pub struct ExpectedImprovements {
    pub uam_improvement: f64,     // Expected percentage improvement in UAM
    pub cost_reduction: f64,      // Expected percentage reduction in cost
    pub efficiency_gain: f64,     // Expected improvement in unique/dollar
    pub confidence_interval: f64, // How confident we are in these predictions
}

/// Historical optimization result for learning
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub timestamp: Instant,
    pub topic: String,
    pub strategy_used: OptimizationStrategy,
    pub performance_before: PerformanceSnapshot,
    pub performance_after: Option<PerformanceSnapshot>,
    pub success_score: f64, // How well the optimization worked (0.0-1.0)
}

/// Snapshot of performance metrics at a point in time
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub uam: f64,
    pub cost_per_minute: f64,
    pub unique_per_dollar: f64,
    pub provider_breakdown: HashMap<ProviderId, f64>, // UAM per provider
}

impl Optimizer {
    /// Create new optimizer with default configuration
    pub fn new() -> Self {
        Self {
            optimization_history: Vec::new(),
            current_strategy: OptimizationStrategy::Balanced {
                uam_weight: 0.5,
                cost_weight: 0.3,
                quality_weight: 0.2,
            },
        }
    }

    /// Set optimization strategy
    pub fn set_strategy(&mut self, strategy: OptimizationStrategy) {
        self.current_strategy = strategy;
    }

    /// Generate optimization plan for a topic based on current performance
    pub fn optimize_for_topic(
        &self,
        topic: &str,
        current_performance: &PerformanceStats,
        optimization_mode: &OptimizationMode,
        topic_routing_strategy: Option<&RoutingStrategy>,
        orchestrator_default_routing_strategy: Option<&RoutingStrategy>,
    ) -> OrchestratorResult<OptimizationPlan> {
        // Analyze current performance
        let performance_analysis = self.analyze_performance(current_performance);

        // Generate partitioning strategy
        let partitioning = self.determine_partitioning_strategy(topic, current_performance, &performance_analysis)?;

        // Generate optimized prompt
        let optimized_prompt = self.generate_optimized_prompt(topic, &partitioning, &performance_analysis)?;

        // Determine optimal routing strategy
        let routing_strategy = self.optimize_routing_strategy(current_performance, optimization_mode, topic_routing_strategy, orchestrator_default_routing_strategy)?;

        // Generate configuration adjustments
        let generation_config = self.optimize_generation_config(current_performance, optimization_mode)?;

        // Calculate confidence based on data quality and historical success
        let confidence = self.calculate_confidence(current_performance);

        // Generate expected improvements
        let expected_improvements =
            self.calculate_expected_improvements(current_performance, &routing_strategy, &partitioning);

        // Generate rationale
        let rationale = self.generate_rationale(
            topic,
            &performance_analysis,
            &partitioning,
            &routing_strategy,
            confidence,
        );

        Ok(OptimizationPlan {
            optimized_prompt,
            routing_strategy,
            generation_config,
            partitioning_strategy: partitioning,
            confidence,
            rationale,
            expected_improvements,
        })
    }

    /// Analyze current performance to identify opportunities
    fn analyze_performance(&self, stats: &PerformanceStats) -> PerformanceAnalysis {
        let mut issues = Vec::new();

        // Check overall UAM
        if stats.overall.uam < 5.0 {
            issues.push("Low overall UAM - need to improve generation rate".to_string());
        }

        // Check cost efficiency
        if stats.overall.unique_per_dollar < 10.0 {
            issues.push("Poor cost efficiency - need to optimize provider usage".to_string());
        }

        // Check provider imbalance
        let provider_uams: Vec<f64> = stats.by_provider.values().map(|m| m.uam).collect();
        if let (Some(max_uam), Some(min_uam)) = (
            provider_uams.iter().max_by(|a, b| a.partial_cmp(b).unwrap()),
            provider_uams.iter().min_by(|a, b| a.partial_cmp(b).unwrap()),
        ) {
            if max_uam / min_uam > 2.0 {
                issues.push("Significant provider performance imbalance - rebalancing needed".to_string());
            }
        }

        // Check uniqueness ratio
        if stats.overall.uniqueness_ratio < 0.5 {
            issues.push("Low uniqueness ratio - high duplicate generation".to_string());
        }

        PerformanceAnalysis {
            overall_score: self.calculate_overall_score(stats),
            issues,
            best_provider: self.identify_best_provider(stats),
            worst_provider: self.identify_worst_provider(stats),
        }
    }

    /// Determine the best partitioning strategy
    fn determine_partitioning_strategy(
        &self,
        topic: &str,
        performance: &PerformanceStats,
        analysis: &PerformanceAnalysis,
    ) -> OrchestratorResult<PartitioningStrategy> {
        // If uniqueness ratio is high (>0.8), no partitioning needed
        if performance.overall.uniqueness_ratio > 0.8 {
            return Ok(PartitioningStrategy::None);
        }

        // If we have significant provider performance differences, use provider-specific
        if let (Some(best), Some(worst)) = (&analysis.best_provider, &analysis.worst_provider) {
            let best_uam = performance.by_provider.get(&best.0).map(|m| m.uam).unwrap_or(0.0);
            let worst_uam = performance.by_provider.get(&worst.0).map(|m| m.uam).unwrap_or(0.0);

            if best_uam > worst_uam * 1.5 {
                return Ok(self.create_provider_specific_partitioning(topic));
            }
        }

        // For complex topics, use semantic partitioning
        if self.is_complex_topic(topic) {
            return Ok(self.create_semantic_partitioning(topic));
        }

        // Default to attribute type partitioning
        Ok(self.create_attribute_type_partitioning(topic))
    }

    /// Generate optimized prompt based on analysis
    fn generate_optimized_prompt(
        &self,
        topic: &str,
        partitioning: &PartitioningStrategy,
        analysis: &PerformanceAnalysis,
    ) -> OrchestratorResult<String> {
        let base_prompt = match partitioning {
            PartitioningStrategy::None => {
                format!(
                    "Generate highly unique and specific attributes for '{topic}'. \
                    Focus on originality, avoid common responses, and ensure each attribute \
                    is distinctive and measurable. Provide creative and unexpected perspectives. \
                    IMPORTANT: Provide all results in English only, translating any foreign language terms to English."
                )
            }

            PartitioningStrategy::Semantic { categories } => {
                format!(
                    "Generate unique attributes for '{}' focusing specifically on: {}. \
                    Be creative within your assigned categories, avoid generic responses, \
                    and ensure high originality in your assigned domains. \
                    IMPORTANT: Provide all results in English only, translating any foreign language terms to English.",
                    topic,
                    categories.join(", ")
                )
            }

            PartitioningStrategy::AttributeType { types } => {
                format!(
                    "Generate unique attributes for '{}' of these specific types: {}. \
                    Focus on your assigned attribute types, be highly specific and creative, \
                    ensure each attribute is unique within your specialization. \
                    IMPORTANT: Provide all results in English only, translating any foreign language terms to English.",
                    topic,
                    types.join(", ")
                )
            }

            PartitioningStrategy::ProviderSpecific { specializations: _ } => {
                // This will be customized per provider
                format!(
                    "Generate unique attributes for '{topic}' leveraging your specific capabilities. \
                    Focus on areas where you excel, ensure maximum creativity and uniqueness. \
                    IMPORTANT: Provide all results in English only, translating any foreign language terms to English."
                )
            }
        };

        // Add performance-based modifications
        let enhanced_prompt = if analysis.overall_score < 0.5 {
            format!(
                "{base_prompt}. IMPORTANT: Prioritize uniqueness over quantity - generate fewer but \
                highly distinctive attributes rather than many similar ones."
            )
        } else {
            base_prompt
        };

        Ok(enhanced_prompt)
    }

    /// Optimize routing strategy based on performance with fallback hierarchy
    fn optimize_routing_strategy(
        &self,
        performance: &PerformanceStats,
        optimization_mode: &OptimizationMode,
        topic_routing_strategy: Option<&RoutingStrategy>,
        orchestrator_default_routing_strategy: Option<&RoutingStrategy>,
    ) -> OrchestratorResult<RoutingStrategy> {
        // Routing strategy fallback hierarchy:
        // 1. If topic has a specific routing strategy, use it
        if let Some(topic_strategy) = topic_routing_strategy {
            tracing::debug!("🎯 Optimizer using topic-level routing strategy: {:?}", topic_strategy);
            return Ok(topic_strategy.clone());
        }

        // 2. If orchestrator has a default routing strategy, use it
        if let Some(orchestrator_strategy) = orchestrator_default_routing_strategy {
            tracing::debug!("🎯 Optimizer using orchestrator default routing strategy: {:?}", orchestrator_strategy);
            return Ok(orchestrator_strategy.clone());
        }

        // 3. Fall back to optimizer's performance-based decision
        tracing::debug!("🎯 Optimizer making performance-based routing decision");

        // Special case: if we only have one provider and no performance data yet, use Backoff strategy
        if performance.by_provider.len() == 1 && performance.overall.uam == 0.0 {
            let provider_id = performance
                .by_provider
                .keys()
                .next()
                .copied()
                .unwrap_or(ProviderId::Random);
            let provider_config = shared::types::ProviderConfig::with_default_model(provider_id);
            return Ok(RoutingStrategy::Backoff { provider: provider_config });
        }

        match optimization_mode {
            OptimizationMode::MaximizeUAM { .. } => {
                // Prioritize providers with highest UAM
                let mut providers_by_uam: Vec<_> = performance
                    .by_provider
                    .iter()
                    .map(|(id, metrics)| (*id, metrics.uam))
                    .collect();
                providers_by_uam.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                let providers: Vec<shared::types::ProviderConfig> = providers_by_uam
                    .into_iter()
                    .map(|(id, _)| shared::types::ProviderConfig::with_default_model(id))
                    .collect();
                Ok(RoutingStrategy::PriorityOrder { providers })
            }

            OptimizationMode::MinimizeCost { .. } => {
                // Weight providers by cost efficiency
                let mut weights = HashMap::new();
                let total_efficiency: f64 = performance.by_provider.values().map(|m| m.unique_per_dollar).sum();

                for (provider_id, metrics) in &performance.by_provider {
                    if total_efficiency > 0.0 {
                        let weight = metrics.unique_per_dollar / total_efficiency;
                        let provider_config = shared::types::ProviderConfig::with_default_model(*provider_id);
                        weights.insert(provider_config, weight as f32);
                    }
                }

                Ok(RoutingStrategy::Weighted { weights })
            }

            OptimizationMode::MaximizeEfficiency => {
                // Balance UAM and cost efficiency
                self.create_balanced_routing_strategy(performance)
            }

            OptimizationMode::Weighted {
                uam_weight,
                cost_weight,
                ..
            } => {
                // Custom weighted strategy
                self.create_custom_weighted_strategy(performance, *uam_weight, *cost_weight)
            }
        }
    }

    /// Generate optimized generation configuration
    fn optimize_generation_config(
        &self,
        performance: &PerformanceStats,
        optimization_mode: &OptimizationMode,
    ) -> OrchestratorResult<GenerationConfig> {
        // Base configuration
        let mut config = GenerationConfig {
            model: "default".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 800,
            temperature: 0.8,  // Higher for creativity
            request_size: 100, // Default request size
        };

        // Adjust based on performance
        if performance.overall.uniqueness_ratio < 0.5 {
            // Low uniqueness - increase temperature and reduce max_tokens for more focused responses
            config.temperature = 0.9;
            config.max_tokens = 600;
            config.request_size = 60; // Request fewer items when uniqueness is low
        } else if performance.overall.uniqueness_ratio > 0.8 {
            // High uniqueness - can afford to generate more per request
            config.max_tokens = 1000;
            config.batch_size = 2;
            config.request_size = 150; // Request more items when uniqueness is high
        }

        // Adjust based on optimization mode
        match optimization_mode {
            OptimizationMode::MinimizeCost { .. } => {
                // Reduce token usage
                config.max_tokens = cmp::min(config.max_tokens, 500);
                config.batch_size = 1;
                config.request_size = cmp::min(config.request_size, 80); // Smaller requests for cost efficiency
            }

            OptimizationMode::MaximizeUAM { .. } => {
                // Optimize for generation speed
                config.batch_size = cmp::max(config.batch_size, 2);
                config.request_size = cmp::max(config.request_size, 120); // Larger requests for max UAM
            }

            _ => {} // Use default adjustments
        }

        Ok(config)
    }

    /// Calculate confidence in optimization decisions
    fn calculate_confidence(&self, performance: &PerformanceStats) -> f64 {
        let mut confidence = 0.5; // Base confidence

        // Increase confidence with more provider data
        if performance.by_provider.len() >= 3 {
            confidence += 0.2;
        }

        // Increase confidence with stable trends
        if performance.trends.stability_score > 0.7 {
            confidence += 0.2;
        }

        // Increase confidence based on historical success
        let historical_success = self.calculate_historical_success_rate();
        confidence += historical_success * 0.2;

        // Decrease confidence for extreme values
        if performance.overall.uam < 1.0 || performance.overall.cost_per_minute > 10.0 {
            confidence -= 0.1;
        }

        confidence.max(0.0).min(1.0)
    }

    // Helper methods

    fn calculate_overall_score(&self, stats: &PerformanceStats) -> f64 {
        // Weighted combination of key metrics (0.0-1.0)
        let uam_score = (stats.overall.uam / 20.0).min(1.0); // 20 UAM = perfect score
        let efficiency_score = (stats.overall.unique_per_dollar / 50.0).min(1.0); // 50 unique/$1 = perfect
        let uniqueness_score = stats.overall.uniqueness_ratio;

        (uam_score * 0.4) + (efficiency_score * 0.3) + (uniqueness_score * 0.3)
    }

    fn identify_best_provider(&self, stats: &PerformanceStats) -> Option<(ProviderId, f64)> {
        stats
            .by_provider
            .iter()
            .max_by(|a, b| a.1.uam.partial_cmp(&b.1.uam).unwrap())
            .map(|(id, metrics)| (*id, metrics.uam))
    }

    fn identify_worst_provider(&self, stats: &PerformanceStats) -> Option<(ProviderId, f64)> {
        stats
            .by_provider
            .iter()
            .min_by(|a, b| a.1.uam.partial_cmp(&b.1.uam).unwrap())
            .map(|(id, metrics)| (*id, metrics.uam))
    }

    fn is_complex_topic(&self, topic: &str) -> bool {
        // Simple heuristic for topic complexity
        let word_count = topic.split_whitespace().count();
        let has_conjunctions = topic.contains(" and ") || topic.contains(" or ") || topic.contains(",");

        word_count > 3 || has_conjunctions
    }

    fn create_provider_specific_partitioning(&self, topic: &str) -> PartitioningStrategy {
        let mut specializations = HashMap::new();

        specializations.insert(
            ProviderId::OpenAI,
            format!("technical and analytical aspects of {topic}"),
        );
        specializations.insert(
            ProviderId::Anthropic,
            format!("creative and conceptual aspects of {topic}"),
        );
        specializations.insert(
            ProviderId::Gemini,
            format!("practical and functional aspects of {topic}"),
        );

        PartitioningStrategy::ProviderSpecific { specializations }
    }

    fn create_semantic_partitioning(&self, topic: &str) -> PartitioningStrategy {
        // Extract semantic categories from topic
        let words: Vec<&str> = topic.split_whitespace().collect();
        let categories = match words.len() {
            1..=2 => vec![
                format!("core {}", topic),
                format!("related {}", topic),
                format!("extended {}", topic),
            ],
            _ => vec![
                format!("primary aspects of {}", topic),
                format!("secondary features of {}", topic),
                format!("contextual elements of {}", topic),
            ],
        };

        PartitioningStrategy::Semantic { categories }
    }

    fn create_attribute_type_partitioning(&self, _topic: &str) -> PartitioningStrategy {
        let types = vec![
            "physical properties".to_string(),
            "functional characteristics".to_string(),
            "contextual relationships".to_string(),
        ];

        PartitioningStrategy::AttributeType { types }
    }

    fn create_balanced_routing_strategy(&self, performance: &PerformanceStats) -> OrchestratorResult<RoutingStrategy> {
        // Create weights balancing UAM and cost efficiency
        let mut weights = HashMap::new();

        for (provider_id, metrics) in &performance.by_provider {
            let uam_score = metrics.uam / 20.0; // Normalize to 0-1
            let efficiency_score = metrics.unique_per_dollar / 50.0; // Normalize to 0-1
            let combined_score = (uam_score * 0.6) + (efficiency_score * 0.4);

            let provider_config = shared::types::ProviderConfig::with_default_model(*provider_id);
            weights.insert(provider_config, combined_score as f32);
        }

        // Normalize weights to sum to 1.0
        let total: f32 = weights.values().sum();
        if total > 0.0 {
            for weight in weights.values_mut() {
                *weight /= total;
            }
        }

        Ok(RoutingStrategy::Weighted { weights })
    }

    fn create_custom_weighted_strategy(
        &self,
        performance: &PerformanceStats,
        uam_weight: f64,
        cost_weight: f64,
    ) -> OrchestratorResult<RoutingStrategy> {
        let mut weights = HashMap::new();

        for (provider_id, metrics) in &performance.by_provider {
            let uam_score = metrics.uam / 20.0;
            let cost_score = metrics.unique_per_dollar / 50.0;
            let combined_score = (uam_score * uam_weight) + (cost_score * cost_weight);

            let provider_config = shared::types::ProviderConfig::with_default_model(*provider_id);
            weights.insert(provider_config, combined_score as f32);
        }

        Ok(RoutingStrategy::Weighted { weights })
    }

    fn calculate_expected_improvements(
        &self,
        _current_performance: &PerformanceStats,
        routing_strategy: &RoutingStrategy,
        partitioning: &PartitioningStrategy,
    ) -> ExpectedImprovements {
        // Simplified improvement estimation based on strategy type
        let uam_improvement = match partitioning {
            PartitioningStrategy::None => 0.0,
            PartitioningStrategy::Semantic { .. } => 15.0, // 15% improvement expected
            PartitioningStrategy::AttributeType { .. } => 20.0,
            PartitioningStrategy::ProviderSpecific { .. } => 25.0,
        };

        let cost_reduction = match routing_strategy {
            RoutingStrategy::Weighted { .. } => 10.0, // 10% cost reduction expected
            RoutingStrategy::PriorityOrder { .. } => 5.0,
            RoutingStrategy::RoundRobin { .. } => 0.0,
            RoutingStrategy::Backoff { .. } => 15.0, // Higher efficiency due to retry optimization
        };

        ExpectedImprovements {
            uam_improvement,
            cost_reduction,
            efficiency_gain: (uam_improvement + cost_reduction) / 2.0,
            confidence_interval: 0.8, // 80% confidence
        }
    }

    fn generate_rationale(
        &self,
        topic: &str,
        analysis: &PerformanceAnalysis,
        partitioning: &PartitioningStrategy,
        routing: &RoutingStrategy,
        confidence: f64,
    ) -> String {
        format!(
            "Optimization for '{}': Implemented {} partitioning and {} routing based on \
            performance analysis (score: {:.1}/1.0). Key issues addressed: {}. \
            Expected improvements in UAM and cost efficiency. Confidence: {:.0}%",
            topic,
            match partitioning {
                PartitioningStrategy::None => "no",
                PartitioningStrategy::Semantic { .. } => "semantic",
                PartitioningStrategy::AttributeType { .. } => "attribute-type",
                PartitioningStrategy::ProviderSpecific { .. } => "provider-specific",
            },
            match routing {
                RoutingStrategy::RoundRobin { .. } => "round-robin",
                RoutingStrategy::PriorityOrder { .. } => "priority-based",
                RoutingStrategy::Weighted { .. } => "weighted",
                RoutingStrategy::Backoff { .. } => "backoff",
            },
            analysis.overall_score,
            analysis.issues.first().unwrap_or(&"none identified".to_string()),
            confidence * 100.0
        )
    }

    fn calculate_historical_success_rate(&self) -> f64 {
        if self.optimization_history.is_empty() {
            return 0.5; // No history, assume average
        }

        let total_success: f64 = self
            .optimization_history
            .iter()
            .map(|result| result.success_score)
            .sum();

        total_success / self.optimization_history.len() as f64
    }
}

/// Internal performance analysis structure
struct PerformanceAnalysis {
    overall_score: f64,
    issues: Vec<String>,
    best_provider: Option<(ProviderId, f64)>,
    worst_provider: Option<(ProviderId, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::performance::{PerformanceMetrics, PerformanceStats};

    #[test]
    fn test_optimizer_creation() {
        let optimizer = Optimizer::new();
        assert!(!optimizer.optimization_history.is_empty() == false); // History starts empty
    }

    #[test]
    fn test_performance_analysis() {
        let optimizer = Optimizer::new();
        let stats = create_test_performance_stats();

        let analysis = optimizer.analyze_performance(&stats);
        assert!(analysis.overall_score >= 0.0 && analysis.overall_score <= 1.0);
    }

    fn create_test_performance_stats() -> PerformanceStats {
        let mut by_provider = HashMap::new();
        by_provider.insert(
            ProviderId::OpenAI,
            PerformanceMetrics {
                uam: 10.0,
                cost_per_minute: 0.5,
                unique_per_dollar: 20.0,
                uniqueness_ratio: 0.8,
                ..Default::default()
            },
        );

        PerformanceStats {
            overall: PerformanceMetrics {
                uam: 10.0,
                cost_per_minute: 0.5,
                unique_per_dollar: 20.0,
                uniqueness_ratio: 0.8,
                ..Default::default()
            },
            by_provider,
            ..Default::default()
        }
    }
}
