//! Basic optimization strategy implementation
//!
//! This provides a simple, reliable optimization approach that generates
//! consistent prompts and routing strategies without complex adaptation logic.

use super::super::traits::OptimizerStrategy;
use super::super::types::*;
use crate::error::OrchestratorResult;
use async_trait::async_trait;
use shared::{GenerationConfig, RoutingStrategy, ProviderId};
use std::time::Instant;

/// Basic optimization strategy that provides consistent, reliable optimization
/// 
/// This optimizer is completely stateless and functional - it generates the same
/// output for the same input every time.
pub struct BasicOptimizer {
    /// Name for identification
    name: String,
}

impl BasicOptimizer {
    /// Create a new basic optimizer
    pub fn new() -> Self {
        Self {
            name: "Basic".to_string(),
        }
    }
    
    /// Generate a simple, reliable prompt for the given topic
    fn generate_basic_prompt(&self, topic: &str) -> String {
        format!(
            "Generate highly specific, unique nouns and noun phrases for '{topic}'. \
            Each attribute must be a concrete thing, object, component, or feature (e.g., 'steel support beam', 'marble entrance hall', 'control panel'). \
            Focus on specific parts, components, materials, structures, or distinctive elements that exist within or relate to this topic. \
            Avoid adjectives, descriptions, and numbers - only generate actual things/nouns using words only. Think like an expert cataloging specific items. \
            Output one noun/noun phrase per line. \
            IMPORTANT: Do not include any numbers, measurements, or quantities. Provide all results in English only, translating any foreign language terms to English."
        )
    }
    
    /// Create a simple routing strategy based on available providers
    fn create_basic_routing(&self, active_producers: &[ProviderId], routing_options: &RoutingOptions) -> RoutingStrategy {
        // Priority: use topic preference, then system default, then create simple strategy
        if let Some(strategy) = &routing_options.topic_preference {
            return strategy.clone();
        }
        
        if let Some(strategy) = &routing_options.system_default {
            return strategy.clone();
        }
        
        // Create simple round-robin strategy
        if active_producers.len() == 1 {
            // Single provider - use backoff strategy
            let provider_config = shared::types::ProviderConfig::with_default_model(active_producers[0]);
            RoutingStrategy::Backoff { provider: provider_config }
        } else {
            // Multiple providers - use round-robin
            let providers = active_producers.iter()
                .map(|&id| shared::types::ProviderConfig::with_default_model(id))
                .collect();
            RoutingStrategy::RoundRobin { providers }
        }
    }
    
    /// Create basic generation config
    fn create_basic_generation_config(&self, _targets: &OptimizationTargets) -> GenerationConfig {
        GenerationConfig {
            model: "default".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 800,
            temperature: 0.8,
            request_size: 100,
        }
    }
}

#[async_trait]
impl OptimizerStrategy for BasicOptimizer {
    async fn optimize(&self, context: OptimizationContext) -> OrchestratorResult<OptimizationResult> {
        // Note: We can't mutate self in a read-only optimizer, but that's fine for basic optimization
        
        // Generate basic prompt for the topic
        let prompt = self.generate_basic_prompt(&context.topic);
        let prompt_assignments = PromptAssignments::uniform(prompt);
        
        // Create routing strategy
        let routing_strategy = self.create_basic_routing(&context.active_producers, &context.routing_options);
        
        // Create generation config
        let generation_config = self.create_basic_generation_config(&context.targets);
        
        // Basic assessment - always moderate confidence since it's simple and reliable
        let assessment = OptimizationAssessment {
            confidence: 0.7, // Moderate confidence - basic but reliable
            expected_impact: ExpectedImpact {
                uam_change_percent: 5.0, // Modest improvement expected
                cost_change_percent: 0.0, // No cost optimization
                quality_change_percent: 0.0, // No quality optimization  
                time_to_effect_seconds: 30, // Quick to take effect
            },
            rationale: format!(
                "Basic optimization for '{}': Using reliable prompt template with {} routing strategy. \
                Prioritizes consistency and reliability over advanced optimization.",
                context.topic,
                match routing_strategy {
                    RoutingStrategy::RoundRobin { .. } => "round-robin",
                    RoutingStrategy::Backoff { .. } => "backoff",
                    RoutingStrategy::PriorityOrder { .. } => "priority",
                    RoutingStrategy::Weighted { .. } => "weighted",
                }
            ),
            metadata: OptimizationMetadata {
                strategy_name: self.name.clone(),
                techniques_applied: vec![
                    "Static prompt generation".to_string(),
                    "Simple routing selection".to_string(),
                    "Default parameter configuration".to_string(),
                ],
                risk_factors: vec![], // Basic optimizer has minimal risk
                timestamp: Instant::now(),
            },
        };

        Ok(OptimizationResult {
            prompt_assignments,
            routing_strategy,
            generation_config,
            assessment,
        })
    }

    async fn update_performance(&mut self, _feedback: PerformanceFeedback) {
        // Basic optimizer doesn't learn from feedback - completely functional
    }

    async fn reset(&mut self) {
        // Basic optimizer is completely stateless - nothing to reset
    }

    async fn get_state(&self) -> OptimizerState {
        OptimizerState {
            name: self.name.clone(),
            active_strategies: vec!["Static optimization".to_string()],
            performance_history_size: 0, // No state tracking
            last_optimization: None,     // No state tracking
            adaptation_level: AdaptationLevel::None,
        }
    }
}

impl Default for BasicOptimizer {
    fn default() -> Self {
        Self::new()
    }
}