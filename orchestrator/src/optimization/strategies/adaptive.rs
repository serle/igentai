//! Adaptive optimization strategy implementation
//!
//! This implements sophisticated optimization with prompt rotation, UAM decline detection,
//! and performance-based adaptation using interior mutability for state tracking.

use super::super::traits::OptimizerStrategy;
use super::super::types::*;
use crate::error::OrchestratorResult;
use async_trait::async_trait;
use shared::{GenerationConfig, RoutingStrategy, ProviderId};
use std::sync::RwLock;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Adaptive optimization strategy that learns and adapts based on performance
///
/// Uses interior mutability to track performance history and adaptation state
/// while maintaining a clean read-only interface.
pub struct AdaptiveOptimizer {
    /// Name for identification
    name: String,
    
    /// Configuration for adaptation behavior
    config: AdaptationConfig,
    
    /// Mutable state for tracking performance and adaptations (interior mutability)
    state: RwLock<AdaptiveState>,
}

/// Configuration for adaptive behavior
#[derive(Debug, Clone)]
pub struct AdaptationConfig {
    /// UAM decline threshold to trigger adaptation (as a percentage, e.g., 0.15 = 15%)
    pub uam_decline_threshold: f64,
    
    /// Minimum time between adaptations
    pub min_adaptation_interval: Duration,
    
    /// Maximum number of prompt templates to track
    pub max_template_history: usize,
    
    /// Window size for performance trend analysis
    pub performance_window_size: usize,
}

/// Internal mutable state for the adaptive optimizer
#[derive(Debug)]
struct AdaptiveState {
    /// Performance history for trend analysis
    performance_history: VecDeque<PerformanceSnapshot>,
    
    /// Current prompt templates and their performance
    prompt_templates: Vec<PromptTemplate>,
    
    /// Current template assignments per producer
    producer_assignments: HashMap<ProviderId, usize>, // template index
    
    /// Last adaptation timestamp
    last_adaptation: Option<Instant>,
    
    /// Current adaptation level
    current_adaptation_level: AdaptationLevel,
}

/// Performance snapshot for trend analysis
#[derive(Debug, Clone)]
struct PerformanceSnapshot {
    timestamp: Instant,
    overall_uam: f64,
    cost_per_minute: f64,
    uniqueness_ratio: f64,
    provider_performance: HashMap<ProviderId, f64>,
}

/// Template with performance tracking
#[derive(Debug, Clone)]
struct PromptTemplate {
    id: String,
    template: String,
    strategy_type: TemplateStrategy,
    usage_count: u32,
    total_uam: f64,
    last_used: Option<Instant>,
}

/// Different template strategies
#[derive(Debug, Clone)]
enum TemplateStrategy {
    Concrete,
    Creative,
    Technical,
    Functional,
    Structural,
    Contextual,
}

impl AdaptiveOptimizer {
    /// Create a new adaptive optimizer with default configuration
    pub fn new() -> Self {
        Self::with_config(AdaptationConfig::default())
    }
    
    /// Create adaptive optimizer with custom configuration
    pub fn with_config(config: AdaptationConfig) -> Self {
        let templates = Self::create_default_templates();
        
        Self {
            name: "Adaptive".to_string(),
            config,
            state: RwLock::new(AdaptiveState {
                performance_history: VecDeque::new(),
                prompt_templates: templates,
                producer_assignments: HashMap::new(),
                last_adaptation: None,
                current_adaptation_level: AdaptationLevel::None,
            }),
        }
    }
    
    /// Create default set of prompt templates
    fn create_default_templates() -> Vec<PromptTemplate> {
        vec![
            PromptTemplate {
                id: "concrete".to_string(),
                template: "Generate highly specific, concrete physical objects and components for '{topic}'. Focus on: materials, parts, structural elements, tools, equipment. Be extremely detailed and specific. Examples: 'reinforced steel beam', 'marble entrance column', 'bronze door handle'. Avoid abstract concepts.".to_string(),
                strategy_type: TemplateStrategy::Concrete,
                usage_count: 0,
                total_uam: 0.0,
                last_used: None,
            },
            PromptTemplate {
                id: "creative".to_string(),
                template: "Think creatively and generate unique, unconventional aspects of '{topic}'. Explore unexpected angles, hidden elements, rarely considered components. Push boundaries while staying factual. Examples: 'secret passage mechanism', 'weathering pattern', 'acoustic property'. Be innovative and surprising.".to_string(),
                strategy_type: TemplateStrategy::Creative,
                usage_count: 0,
                total_uam: 0.0,
                last_used: None,
            },
            PromptTemplate {
                id: "technical".to_string(),
                template: "Generate technical, engineering, and specialized components of '{topic}'. Focus on: technical specifications, engineering elements, specialized equipment, precise terminology. Examples: 'load-bearing junction', 'thermal insulation layer', 'electrical conduit'. Use expert-level technical knowledge.".to_string(),
                strategy_type: TemplateStrategy::Technical,
                usage_count: 0,
                total_uam: 0.0,
                last_used: None,
            },
        ]
    }
    
    /// Analyze UAM trend from performance history
    fn analyze_uam_trend(&self, state: &AdaptiveState) -> UAMTrendAnalysis {
        if state.performance_history.len() < 3 {
            return UAMTrendAnalysis {
                direction: TrendDirection::Unknown,
                magnitude: 0.0,
                confidence: 0.0,
            };
        }
        
        let recent_uams: Vec<f64> = state.performance_history.iter()
            .rev()
            .take(5)
            .map(|snapshot| snapshot.overall_uam)
            .collect();
        
        if recent_uams.len() < 2 {
            return UAMTrendAnalysis {
                direction: TrendDirection::Unknown,
                magnitude: 0.0,
                confidence: 0.0,
            };
        }
        
        // Simple trend analysis: compare recent average to earlier average
        let recent_avg = recent_uams.iter().take(3).sum::<f64>() / 3.0;
        let earlier_avg = recent_uams.iter().skip(2).sum::<f64>() / (recent_uams.len() - 2) as f64;
        
        let change_percent = (recent_avg - earlier_avg) / earlier_avg;
        
        let direction = if change_percent < -self.config.uam_decline_threshold {
            TrendDirection::Declining
        } else if change_percent > 0.05 {
            TrendDirection::Improving
        } else {
            TrendDirection::Stable
        };
        
        UAMTrendAnalysis {
            direction,
            magnitude: change_percent.abs(),
            confidence: (recent_uams.len() as f64 / 5.0).min(1.0),
        }
    }
}

/// UAM trend analysis result
#[derive(Debug)]
struct UAMTrendAnalysis {
    direction: TrendDirection,
    magnitude: f64,
    confidence: f64,
}

impl Default for AdaptationConfig {
    fn default() -> Self {
        Self {
            uam_decline_threshold: 0.15, // 15% decline
            min_adaptation_interval: Duration::from_secs(300), // 5 minutes
            max_template_history: 10,
            performance_window_size: 20,
        }
    }
}

#[async_trait]
impl OptimizerStrategy for AdaptiveOptimizer {
    async fn optimize(&self, context: OptimizationContext) -> OrchestratorResult<OptimizationResult> {
        let mut state = self.state.write().unwrap();
        
        // Record performance snapshot
        let snapshot = PerformanceSnapshot {
            timestamp: Instant::now(),
            overall_uam: context.performance.overall_uam,
            cost_per_minute: context.performance.cost_per_minute,
            uniqueness_ratio: context.performance.uniqueness_ratio,
            provider_performance: context.performance.by_provider.iter()
                .map(|(id, metrics)| (*id, metrics.uam))
                .collect(),
        };
        
        state.performance_history.push_back(snapshot);
        if state.performance_history.len() > self.config.performance_window_size {
            state.performance_history.pop_front();
        }
        
        // Analyze trend and determine if adaptation is needed
        let trend_analysis = self.analyze_uam_trend(&state);
        let needs_adaptation = matches!(trend_analysis.direction, TrendDirection::Declining) 
            && trend_analysis.confidence > 0.5
            && state.last_adaptation.map_or(true, |last| 
                Instant::now().duration_since(last) > self.config.min_adaptation_interval
            );
        
        // Select prompt strategy based on trend analysis
        let (prompt_assignments, adaptation_level) = if needs_adaptation {
            // Adaptive prompt selection
            state.current_adaptation_level = match trend_analysis.magnitude {
                mag if mag > 0.3 => AdaptationLevel::Aggressive,
                mag if mag > 0.15 => AdaptationLevel::Moderate, 
                _ => AdaptationLevel::Minimal,
            };
            state.last_adaptation = Some(Instant::now());
            
            // Assign different templates to different producers
            let mut assignments = HashMap::new();
            for (i, &producer_id) in context.active_producers.iter().enumerate() {
                let template_idx = i % state.prompt_templates.len();
                state.producer_assignments.insert(producer_id, template_idx);
                
                let template = &mut state.prompt_templates[template_idx];
                let prompt = template.template.replace("{topic}", &context.topic);
                template.usage_count += 1;
                template.last_used = Some(Instant::now());
                
                assignments.insert(producer_id, ProducerAssignment {
                    prompt,
                    parameter_overrides: ParameterOverrides::default(),
                    rationale: format!("Adaptive assignment: {} strategy", template.id),
                });
            }
            
            (PromptAssignments::custom(assignments), state.current_adaptation_level.clone())
        } else {
            // Use best performing template for all producers
            let best_template = state.prompt_templates.iter()
                .max_by(|a, b| {
                    let avg_a = if a.usage_count > 0 { a.total_uam / a.usage_count as f64 } else { 0.0 };
                    let avg_b = if b.usage_count > 0 { b.total_uam / b.usage_count as f64 } else { 0.0 };
                    avg_a.partial_cmp(&avg_b).unwrap()
                })
                .unwrap_or(&state.prompt_templates[0]);
            
            let prompt = best_template.template.replace("{topic}", &context.topic);
            (PromptAssignments::uniform(prompt), AdaptationLevel::None)
        };
        
        // Create routing strategy (simplified for now)
        let routing_strategy = context.routing_options.topic_preference
            .or(context.routing_options.system_default)
            .unwrap_or_else(|| {
                if context.active_producers.len() == 1 {
                    let provider_config = shared::types::ProviderConfig::with_default_model(context.active_producers[0]);
                    RoutingStrategy::Backoff { provider: provider_config }
                } else {
                    let providers = context.active_producers.iter()
                        .map(|&id| shared::types::ProviderConfig::with_default_model(id))
                        .collect();
                    RoutingStrategy::RoundRobin { providers }
                }
            });
        
        // Generate configuration
        let generation_config = GenerationConfig {
            model: "default".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 800,
            temperature: match adaptation_level {
                AdaptationLevel::Aggressive => 0.9,
                AdaptationLevel::Moderate => 0.85,
                _ => 0.8,
            },
            request_size: match adaptation_level {
                AdaptationLevel::Aggressive => 80,
                AdaptationLevel::Moderate => 90,
                _ => 100,
            },
        };
        
        let assessment = OptimizationAssessment {
            confidence: 0.8 + (trend_analysis.confidence * 0.2),
            expected_impact: ExpectedImpact {
                uam_change_percent: match adaptation_level {
                    AdaptationLevel::Aggressive => 25.0,
                    AdaptationLevel::Moderate => 15.0,
                    AdaptationLevel::Minimal => 8.0,
                    AdaptationLevel::None => 5.0,
                },
                cost_change_percent: 0.0,
                quality_change_percent: 10.0,
                time_to_effect_seconds: 60,
            },
            rationale: format!(
                "Adaptive optimization for '{}': UAM trend shows {} performance. Applied {} adaptation with {} template assignments.",
                context.topic,
                match trend_analysis.direction {
                    TrendDirection::Declining => "declining",
                    TrendDirection::Improving => "improving", 
                    TrendDirection::Stable => "stable",
                    TrendDirection::Unknown => "unknown",
                },
                match adaptation_level {
                    AdaptationLevel::None => "no",
                    AdaptationLevel::Minimal => "minimal",
                    AdaptationLevel::Moderate => "moderate",
                    AdaptationLevel::Aggressive => "aggressive",
                },
                if prompt_assignments.uses_uniform_prompts() { "uniform" } else { "diversified" }
            ),
            metadata: OptimizationMetadata {
                strategy_name: self.name.clone(),
                techniques_applied: vec![
                    "UAM trend analysis".to_string(),
                    "Adaptive template selection".to_string(),
                    "Performance-based parameter tuning".to_string(),
                ],
                risk_factors: if matches!(adaptation_level, AdaptationLevel::Aggressive) {
                    vec!["Aggressive adaptation may cause instability".to_string()]
                } else {
                    vec![]
                },
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

    async fn update_performance(&mut self, feedback: PerformanceFeedback) {
        let mut state = self.state.write().unwrap();
        
        // Update template performance if we have an assignment for this producer
        if let Some(&template_idx) = state.producer_assignments.get(&feedback.producer_id) {
            if let Some(template) = state.prompt_templates.get_mut(template_idx) {
                template.total_uam += feedback.actual_uam;
            }
        }
    }

    async fn reset(&mut self) {
        let mut state = self.state.write().unwrap();
        state.performance_history.clear();
        state.producer_assignments.clear();
        state.last_adaptation = None;
        state.current_adaptation_level = AdaptationLevel::None;
        
        // Reset template performance but keep the templates
        for template in &mut state.prompt_templates {
            template.usage_count = 0;
            template.total_uam = 0.0;
            template.last_used = None;
        }
    }

    async fn get_state(&self) -> OptimizerState {
        let state = self.state.read().unwrap();
        
        OptimizerState {
            name: self.name.clone(),
            active_strategies: vec![
                "UAM trend analysis".to_string(),
                "Adaptive template rotation".to_string(),
                "Performance feedback learning".to_string(),
            ],
            performance_history_size: state.performance_history.len(),
            last_optimization: state.last_adaptation,
            adaptation_level: state.current_adaptation_level.clone(),
        }
    }
}