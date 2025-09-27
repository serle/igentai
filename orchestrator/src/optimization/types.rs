//! Optimization-related types and data structures
//!
//! This module contains all the data types used by optimization strategies,
//! keeping them separate from trait definitions and implementations.

use shared::{GenerationConfig, OptimizationMode, ProviderId, RoutingStrategy};
use std::collections::HashMap;
use std::time::Instant;

/// Core optimization context containing all inputs needed for decision making
#[derive(Debug, Clone)]
pub struct OptimizationContext {
    /// The topic being optimized for
    pub topic: String,
    
    /// Current system performance metrics
    pub performance: PerformanceMetrics,
    
    /// Active producers that need work assignments
    pub active_producers: Vec<ProviderId>,
    
    /// Optimization goals and constraints
    pub targets: OptimizationTargets,
    
    /// Available routing strategies in priority order
    pub routing_options: RoutingOptions,
}

/// Current performance state of the system
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Overall unique attributes per minute
    pub overall_uam: f64,
    
    /// Cost per minute in USD
    pub cost_per_minute: f64,
    
    /// Ratio of unique to total attributes generated
    pub uniqueness_ratio: f64,
    
    /// Performance breakdown by provider
    pub by_provider: HashMap<ProviderId, ProviderMetrics>,
    
    /// Recent performance trend
    pub trend: PerformanceTrend,
}

/// Performance metrics for a specific provider
#[derive(Debug, Clone)]
pub struct ProviderMetrics {
    pub uam: f64,
    pub cost_per_minute: f64,
    pub uniqueness_ratio: f64,
    pub response_time_ms: f64,
    pub success_rate: f64,
}

/// Performance trend analysis
#[derive(Debug, Clone)]
pub struct PerformanceTrend {
    pub direction: TrendDirection,
    pub magnitude: f64,
    pub confidence: f64,
    pub sample_size: usize,
}

/// Direction of performance trend
#[derive(Debug, Clone, PartialEq)]
pub enum TrendDirection {
    Improving,
    Stable,
    Declining,
    Unknown,
}

/// Optimization targets and constraints
#[derive(Debug, Clone)]
pub struct OptimizationTargets {
    pub mode: OptimizationMode,
    pub max_cost_per_minute: f64,
    pub target_uam: f64,
    pub quality_threshold: f64,
}

/// Available routing strategy options
#[derive(Debug, Clone)]
pub struct RoutingOptions {
    /// Topic-specific routing preference (highest priority)
    pub topic_preference: Option<RoutingStrategy>,
    
    /// System default routing strategy (fallback)
    pub system_default: Option<RoutingStrategy>,
}

/// Complete optimization result with all recommendations
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Prompt assignments for producers
    pub prompt_assignments: PromptAssignments,
    
    /// Recommended routing strategy
    pub routing_strategy: RoutingStrategy,
    
    /// Generation configuration
    pub generation_config: GenerationConfig,
    
    /// Quality assessment of this optimization
    pub assessment: OptimizationAssessment,
}

/// Prompt assignments for producers
#[derive(Debug, Clone)]
pub struct PromptAssignments {
    /// Default prompt if all producers use the same one
    pub default_prompt: Option<String>,
    
    /// Per-producer prompt assignments (overrides default)
    pub producer_specific: HashMap<ProviderId, ProducerAssignment>,
}

/// Assignment for a specific producer
#[derive(Debug, Clone)]
pub struct ProducerAssignment {
    /// The prompt template to use
    pub prompt: String,
    
    /// Parameter overrides for this producer
    pub parameter_overrides: ParameterOverrides,
    
    /// Rationale for this specific assignment
    pub rationale: String,
}

/// Parameter overrides for fine-tuning generation
#[derive(Debug, Clone, Default)]
pub struct ParameterOverrides {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub request_size: Option<usize>,
}

/// Assessment of the optimization quality and expected impact
#[derive(Debug, Clone)]
pub struct OptimizationAssessment {
    /// Confidence in this optimization (0.0-1.0)
    pub confidence: f64,
    
    /// Expected performance improvements
    pub expected_impact: ExpectedImpact,
    
    /// Human-readable explanation
    pub rationale: String,
    
    /// Metadata about the optimization approach used
    pub metadata: OptimizationMetadata,
}

/// Expected impact of the optimization
#[derive(Debug, Clone)]
pub struct ExpectedImpact {
    pub uam_change_percent: f64,
    pub cost_change_percent: f64,
    pub quality_change_percent: f64,
    pub time_to_effect_seconds: u64,
}

/// Metadata about the optimization approach
#[derive(Debug, Clone)]
pub struct OptimizationMetadata {
    pub strategy_name: String,
    pub techniques_applied: Vec<String>,
    pub risk_factors: Vec<String>,
    pub timestamp: Instant,
}

/// Performance feedback for continuous learning
#[derive(Debug, Clone)]
pub struct PerformanceFeedback {
    pub producer_id: ProviderId,
    pub actual_uam: f64,
    pub actual_cost: f64,
    pub actual_uniqueness: f64,
    pub timestamp: Instant,
}

/// Current state of an optimizer for monitoring
#[derive(Debug, Clone)]
pub struct OptimizerState {
    pub name: String,
    pub active_strategies: Vec<String>,
    pub performance_history_size: usize,
    pub last_optimization: Option<Instant>,
    pub adaptation_level: AdaptationLevel,
}

/// Level of adaptation currently active
#[derive(Debug, Clone, PartialEq)]
pub enum AdaptationLevel {
    /// No adaptation, using baseline strategies
    None,
    
    /// Minor parameter adjustments
    Minimal,
    
    /// Template rotation and moderate changes
    Moderate,
    
    /// Aggressive restructuring and adaptation
    Aggressive,
}

impl PromptAssignments {
    /// Get the prompt for a specific producer
    pub fn get_prompt_for_producer(&self, producer_id: ProviderId) -> Option<&str> {
        if let Some(assignment) = self.producer_specific.get(&producer_id) {
            Some(&assignment.prompt)
        } else {
            self.default_prompt.as_deref()
        }
    }
    
    /// Check if all producers use the same prompt
    pub fn uses_uniform_prompts(&self) -> bool {
        self.producer_specific.is_empty() && self.default_prompt.is_some()
    }
    
    /// Create simple prompt assignments with a default prompt
    pub fn uniform(prompt: String) -> Self {
        Self {
            default_prompt: Some(prompt),
            producer_specific: HashMap::new(),
        }
    }
    
    /// Create prompt assignments with per-producer customization
    pub fn custom(assignments: HashMap<ProviderId, ProducerAssignment>) -> Self {
        Self {
            default_prompt: None,
            producer_specific: assignments,
        }
    }
}