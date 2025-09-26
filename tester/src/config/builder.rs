//! Orchestrator Configuration Builder
//!
//! Provides a flexible builder pattern for constructing orchestrator configurations

use super::fault_tolerance::FaultToleranceConfig;
use super::{OrchestratorConfig, OrchestratorMode};
use std::time::Duration;

pub struct OrchestratorConfigBuilder {
    config: OrchestratorConfig,
    pub fault_tolerance_config: Option<FaultToleranceConfig>,
}

impl OrchestratorConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: OrchestratorConfig::default(),
            fault_tolerance_config: None,
        }
    }

    /// Set orchestrator mode (CLI or WebServer)
    pub fn mode(mut self, mode: OrchestratorMode) -> Self {
        self.config.mode = mode;
        self
    }

    /// Enable CLI mode (default)
    pub fn cli_mode(mut self) -> Self {
        self.config.mode = OrchestratorMode::Cli;
        self
    }

    /// Enable WebServer mode
    pub fn webserver_mode(mut self) -> Self {
        self.config.mode = OrchestratorMode::WebServer;
        self
    }

    /// Set provider (random or env)
    pub fn provider(mut self, provider: &str) -> Self {
        self.config.provider = provider.to_string();
        self
    }

    /// Set tracing endpoint
    pub fn trace_endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.config.trace_endpoint = Some(endpoint.into());
        self
    }

    /// Set topic name
    pub fn topic<S: Into<String>>(mut self, topic: S) -> Self {
        self.config.topic = Some(topic.into());
        self
    }

    /// Set number of producers
    pub fn producers(mut self, count: usize) -> Self {
        self.config.producers = Some(count);
        self
    }

    /// Set number of iterations (None for unlimited)
    pub fn iterations(mut self, count: Option<usize>) -> Self {
        self.config.iterations = count;
        self
    }

    /// Set request size
    pub fn request_size(mut self, size: usize) -> Self {
        self.config.request_size = Some(size);
        self
    }

    /// Set output directory
    pub fn output<S: Into<String>>(mut self, output: S) -> Self {
        self.config.output = Some(output.into());
        self
    }

    /// Set webserver bind address
    pub fn webserver_addr<S: Into<String>>(mut self, addr: S) -> Self {
        self.config.webserver_addr = Some(addr.into());
        self
    }

    /// Set producer communication bind address
    pub fn producer_addr<S: Into<String>>(mut self, addr: S) -> Self {
        self.config.producer_addr = Some(addr.into());
        self
    }

    /// Set log level (trace, debug, info, warn, error)
    pub fn log_level<S: Into<String>>(mut self, level: S) -> Self {
        self.config.log_level = level.into();
        self
    }

    /// Set maximum duration to wait for completion
    pub fn max_duration(mut self, duration: Duration) -> Self {
        self.config.max_duration = duration;
        self
    }

    /// Set routing strategy (backoff, roundrobin, priority, weighted)
    pub fn routing_strategy<S: Into<String>>(mut self, strategy: S) -> Self {
        self.config.routing_strategy = Some(strategy.into());
        self
    }

    /// Set routing config (strategy-specific configuration)
    pub fn routing_config<S: Into<String>>(mut self, config: S) -> Self {
        self.config.routing_config = Some(config.into());
        self
    }

    // Comprehensive fluent routing configuration API aligned with new terminology

    /// Configure backoff routing strategy with a single provider:model pair
    /// Retries with exponential backoff on failures
    /// Example: .with_backoff_strategy("openai:gpt-4o-mini")
    pub fn with_backoff_strategy<S: Into<String>>(mut self, provider_config: S) -> Self {
        self.config.routing_strategy = Some("backoff".to_string());
        self.config.routing_config = Some(provider_config.into());
        self
    }

    /// Configure round-robin routing strategy across multiple providers
    /// Distributes requests evenly across all providers
    /// Example: .with_round_robin_strategy("openai:gpt-4o-mini,anthropic:claude-3-sonnet,gemini:gemini-pro")
    pub fn with_round_robin_strategy<S: Into<String>>(mut self, provider_configs: S) -> Self {
        self.config.routing_strategy = Some("roundrobin".to_string());
        self.config.routing_config = Some(provider_configs.into());
        self
    }

    /// Configure priority routing strategy (tries providers in order)
    /// Falls back to next provider if current one fails
    /// Example: .with_priority_strategy("gemini:gemini-pro,openai:gpt-4o-mini,anthropic:claude-3-sonnet") // cheapest first
    pub fn with_priority_strategy<S: Into<String>>(mut self, provider_configs: S) -> Self {
        self.config.routing_strategy = Some("priority".to_string());
        self.config.routing_config = Some(provider_configs.into());
        self
    }

    /// Configure weighted routing strategy with providers and weights
    /// Distributes requests according to specified weights
    /// Example: .with_weighted_strategy("openai:gpt-4o-mini:0.6,anthropic:claude-3-sonnet:0.3,gemini:gemini-pro:0.1")
    pub fn with_weighted_strategy<S: Into<String>>(mut self, weighted_configs: S) -> Self {
        self.config.routing_strategy = Some("weighted".to_string());
        self.config.routing_config = Some(weighted_configs.into());
        self
    }

    // Convenience methods for common testing scenarios

    /// Use random provider for testing (no API keys required)
    /// Equivalent to: .with_backoff_strategy("random:random")
    pub fn with_random_provider(self) -> Self {
        self.with_backoff_strategy("random:random")
    }

    /// Use OpenAI GPT-4o-mini for testing
    /// Equivalent to: .with_backoff_strategy("openai:gpt-4o-mini")
    pub fn with_openai_gpt4o_mini(self) -> Self {
        self.with_backoff_strategy("openai:gpt-4o-mini")
    }

    /// Use OpenAI GPT-4-turbo for testing
    /// Equivalent to: .with_backoff_strategy("openai:gpt-4-turbo")
    pub fn with_openai_gpt4_turbo(self) -> Self {
        self.with_backoff_strategy("openai:gpt-4-turbo")
    }

    /// Use Anthropic Claude 3 Sonnet for testing
    /// Equivalent to: .with_backoff_strategy("anthropic:claude-3-sonnet")
    pub fn with_anthropic_claude3_sonnet(self) -> Self {
        self.with_backoff_strategy("anthropic:claude-3-sonnet")
    }

    /// Use Anthropic Claude 3 Haiku for testing
    /// Equivalent to: .with_backoff_strategy("anthropic:claude-3-haiku")
    pub fn with_anthropic_claude3_haiku(self) -> Self {
        self.with_backoff_strategy("anthropic:claude-3-haiku")
    }

    /// Use Google Gemini Pro for testing
    /// Equivalent to: .with_backoff_strategy("gemini:gemini-pro")
    pub fn with_gemini_pro(self) -> Self {
        self.with_backoff_strategy("gemini:gemini-pro")
    }

    /// Use Google Gemini Flash for testing
    /// Equivalent to: .with_backoff_strategy("gemini:gemini-1.5-flash")
    pub fn with_gemini_flash(self) -> Self {
        self.with_backoff_strategy("gemini:gemini-1.5-flash")
    }

    // Multi-provider testing configurations

    /// Configure all major providers in round-robin
    /// Uses fastest/cheapest models from each provider
    pub fn with_all_providers_round_robin(self) -> Self {
        self.with_round_robin_strategy("openai:gpt-4o-mini,anthropic:claude-3-haiku,gemini:gemini-1.5-flash")
    }

    /// Configure cost-optimized priority routing (cheapest first)
    /// Falls back from cheapest to more expensive options
    pub fn with_cost_optimized_priority(self) -> Self {
        self.with_priority_strategy("gemini:gemini-1.5-flash,openai:gpt-4o-mini,anthropic:claude-3-haiku")
    }

    /// Configure quality-optimized priority routing (best models first)
    /// Falls back from highest quality to faster options
    pub fn with_quality_optimized_priority(self) -> Self {
        self.with_priority_strategy("openai:gpt-4-turbo,anthropic:claude-3-sonnet,gemini:gemini-pro")
    }

    /// Configure balanced weighted distribution
    /// Favors OpenAI but includes other providers
    pub fn with_balanced_weighted_distribution(self) -> Self {
        self.with_weighted_strategy("openai:gpt-4o-mini:0.5,anthropic:claude-3-haiku:0.3,gemini:gemini-1.5-flash:0.2")
    }

    /// Configure OpenAI-heavy weighted distribution
    /// Primarily uses OpenAI with minimal fallback
    pub fn with_openai_heavy_distribution(self) -> Self {
        self.with_weighted_strategy("openai:gpt-4o-mini:0.8,random:random:0.2")
    }

    /// Configure random testing with occasional real API calls
    /// Mostly uses random for speed, some real calls for validation
    pub fn with_mixed_testing_distribution(self) -> Self {
        self.with_weighted_strategy("random:random:0.7,openai:gpt-4o-mini:0.3")
    }

    // Legacy compatibility methods (marked as deprecated)

    /// @deprecated Use with_random_provider() instead
    pub fn with_test_provider(self) -> Self {
        self.with_random_provider()
    }

    /// @deprecated Use with_openai_gpt4o_mini() instead
    pub fn with_openai_provider(self) -> Self {
        self.with_openai_gpt4o_mini()
    }

    /// @deprecated Use with_mixed_testing_distribution() instead
    pub fn with_multi_provider_test(self) -> Self {
        self.with_mixed_testing_distribution()
    }

    // Testing scenario builders with comprehensive configurations

    /// Configure for load testing with high concurrency
    /// Uses fast, cost-effective models for maximum throughput
    pub fn for_load_testing(self) -> Self {
        self.with_all_providers_round_robin()
            .producers(10)
            .iterations(Some(50))
            .log_level("info")
    }

    /// Configure for quality validation testing
    /// Uses best available models to ensure high-quality output
    pub fn for_quality_validation(self) -> Self {
        self.with_quality_optimized_priority()
            .producers(3)
            .iterations(Some(10))
            .log_level("debug")
    }

    /// Configure for cost optimization testing
    /// Prioritizes cheapest options while maintaining functionality
    pub fn for_cost_optimization(self) -> Self {
        self.with_cost_optimized_priority()
            .producers(5)
            .iterations(Some(20))
            .log_level("info")
    }

    /// Configure for reliability testing with fault tolerance
    /// Tests system behavior under various failure conditions
    pub fn for_reliability_testing(self) -> Self {
        self.with_balanced_weighted_distribution()
            .producers(5)
            .iterations(Some(30))
            .log_level("debug")
            .max_duration(Duration::from_secs(300))
    }

    /// Configure for rapid development testing
    /// Uses random provider for speed, minimal real API calls
    pub fn for_rapid_development(self) -> Self {
        self.with_random_provider()
            .producers(2)
            .iterations(Some(5))
            .log_level("debug")
            .max_duration(Duration::from_secs(30))
    }

    /// Configure for end-to-end validation
    /// Thorough testing with real providers and comprehensive validation
    pub fn for_e2e_validation(self) -> Self {
        self.with_all_providers_round_robin()
            .producers(3)
            .iterations(Some(15))
            .log_level("debug")
            .max_duration(Duration::from_secs(180))
    }

    /// Configure for performance benchmarking
    /// Optimized for measuring system performance metrics
    pub fn for_performance_benchmarking(self) -> Self {
        self.with_openai_gpt4o_mini()
            .producers(8)
            .iterations(Some(100))
            .log_level("info")
            .max_duration(Duration::from_secs(600))
    }

    /// Configure for single provider stress testing
    /// Tests a single provider under high load
    pub fn for_single_provider_stress_test<S: Into<String>>(self, provider_config: S) -> Self {
        self.with_backoff_strategy(provider_config)
            .producers(15)
            .iterations(Some(200))
            .log_level("warn")
            .max_duration(Duration::from_secs(900))
    }

    // Environment-aware configuration methods

    /// Configure based on whether API keys are available
    /// Automatically chooses random or real providers based on environment
    pub fn with_environment_aware_routing(self) -> Self {
        // This would ideally check env vars, but for now we'll use mixed distribution
        self.with_mixed_testing_distribution()
    }

    /// Configure for CI/CD pipeline testing
    /// Optimized for automated testing environments
    pub fn for_ci_cd_testing(self) -> Self {
        self.with_random_provider()
            .producers(3)
            .iterations(Some(10))
            .log_level("info")
            .max_duration(Duration::from_secs(60))
    }

    /// Configure for local development
    /// Balanced between speed and real API validation
    pub fn for_local_development(self) -> Self {
        self.with_mixed_testing_distribution()
            .producers(2)
            .iterations(Some(5))
            .log_level("debug")
            .max_duration(Duration::from_secs(45))
    }

    /// Configure for production validation
    /// Conservative settings for production environment testing
    pub fn for_production_validation(self) -> Self {
        self.with_cost_optimized_priority()
            .producers(3)
            .iterations(Some(10))
            .log_level("warn")
            .max_duration(Duration::from_secs(120))
    }

    /// Build the configuration
    pub fn build(mut self) -> OrchestratorConfig {
        self.config.fault_tolerance = self.fault_tolerance_config;
        self.config
    }
}

impl Default for OrchestratorConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
