//! Producer data structures and configuration types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::{OptimizationMode, ProviderId, TokenUsage};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

/// Producer configuration
#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub orchestrator_addr: SocketAddr,
    pub topic: String,
    pub optimization_mode: OptimizationMode,
    pub api_keys: HashMap<ProviderId, String>,
    pub routing_weights: HashMap<ProviderId, f64>,
    pub max_concurrent_requests: usize,
    pub retry_attempts: usize,
    pub request_timeout_ms: u64,
    pub request_size: usize,
}

impl ProducerConfig {
    pub fn new(orchestrator_addr: SocketAddr, topic: String) -> Self {
        Self {
            orchestrator_addr,
            topic,
            optimization_mode: OptimizationMode::MaximizeEfficiency,
            api_keys: HashMap::new(),
            routing_weights: HashMap::new(),
            max_concurrent_requests: 10,
            retry_attempts: 3,
            request_timeout_ms: 30000,
            request_size: 60, // Default value
        }
    }
}

/// API request to external providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub provider: ProviderId,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub request_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

/// API response from external providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub provider: ProviderId,
    pub request_id: Uuid,
    pub content: String,
    pub tokens_used: TokenUsage,
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Processed attributes extracted from responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedAttribute {
    pub value: String,
    pub source_provider: ProviderId,
    pub extraction_pattern: String,
    pub confidence_score: f64,
    pub timestamp: DateTime<Utc>,
    pub is_unique: bool,
}

/// Producer performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProducerMetrics {
    pub requests_sent: u64,
    pub responses_received: u64,
    pub attributes_extracted: u64,
    pub unique_attributes: u64,
    pub total_tokens_used: u64,
    pub total_cost: f64,
    pub avg_response_time_ms: f64,
    pub success_rate: f64,
    pub uptime_seconds: u64,
    pub last_updated: DateTime<Utc>,
}

impl ProducerMetrics {
    pub fn new() -> Self {
        Self {
            last_updated: Utc::now(),
            ..Default::default()
        }
    }

    /// Calculate attributes per minute
    pub fn attributes_per_minute(&self) -> f64 {
        if self.uptime_seconds == 0 {
            return 0.0;
        }
        self.attributes_extracted as f64 / (self.uptime_seconds as f64 / 60.0)
    }

    /// Calculate cost efficiency (attributes per dollar)
    pub fn cost_efficiency(&self) -> f64 {
        if self.total_cost == 0.0 {
            return 0.0;
        }
        self.unique_attributes as f64 / self.total_cost
    }

    /// Calculate token efficiency (attributes per 1k tokens)
    pub fn token_efficiency(&self) -> f64 {
        if self.total_tokens_used == 0 {
            return 0.0;
        }
        (self.unique_attributes as f64 * 1000.0) / self.total_tokens_used as f64
    }
}

/// Producer internal state
#[derive(Debug)]
pub struct ProducerState {
    pub config: ProducerConfig,
    pub is_running: bool,
    pub should_stop: bool,
    pub start_time: Option<DateTime<Utc>>,
    pub current_prompt: Option<String>,
    pub routing_strategy: Option<shared::types::RoutingStrategy>,
    pub generation_config: Option<shared::types::GenerationConfig>,
    pub metrics: ProducerMetrics,
    /// Seen values from orchestrator for bloom filter synchronization
    pub seen_values_from_orchestrator: Option<Vec<String>>,
    pub last_sync_version: Option<u64>,
}

impl ProducerState {
    pub fn new(config: ProducerConfig) -> Self {
        Self {
            config,
            is_running: false,
            should_stop: false,
            start_time: None,
            current_prompt: None,
            routing_strategy: None,
            generation_config: None,
            metrics: ProducerMetrics::new(),
            seen_values_from_orchestrator: None,
            last_sync_version: None,
        }
    }

    /// Mark producer as started
    pub fn start(&mut self) {
        self.is_running = true;
        self.should_stop = false;
        self.start_time = Some(Utc::now());
    }

    /// Mark producer as stopped
    pub fn stop(&mut self) {
        self.is_running = false;
        self.should_stop = true;
    }

    /// Get current uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        if let Some(start_time) = self.start_time {
            (Utc::now() - start_time).num_seconds() as u64
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_producer_config_creation() {
        let addr: SocketAddr = "127.0.0.1:6001".parse().unwrap();
        let config = ProducerConfig::new(addr, "test topic".to_string());

        assert_eq!(config.orchestrator_addr, addr);
        assert_eq!(config.topic, "test topic");
        assert_eq!(config.max_concurrent_requests, 10);
        assert!(config.api_keys.is_empty());
    }

    #[test]
    fn test_producer_metrics_calculations() {
        let mut metrics = ProducerMetrics::new();
        metrics.attributes_extracted = 120;
        metrics.unique_attributes = 100;
        metrics.total_tokens_used = 5000;
        metrics.total_cost = 2.5;
        metrics.uptime_seconds = 60; // 1 minute

        assert_eq!(metrics.attributes_per_minute(), 120.0);
        assert_eq!(metrics.cost_efficiency(), 40.0); // 100 attributes / $2.5
        assert_eq!(metrics.token_efficiency(), 20.0); // (100 * 1000) / 5000
    }

    #[test]
    fn test_producer_state_lifecycle() {
        let config = ProducerConfig::new("127.0.0.1:6001".parse().unwrap(), "test".to_string());
        let mut state = ProducerState::new(config);

        assert!(!state.is_running);
        assert!(!state.should_stop);

        state.start();
        assert!(state.is_running);
        assert!(!state.should_stop);
        assert!(state.start_time.is_some());

        state.stop();
        assert!(!state.is_running);
        assert!(state.should_stop);
    }
}

// ============================================================================
// Producer Execution Types
// ============================================================================

use crate::core::generator::CommandGenerator;
use shared::ProducerCommand;
use std::time::Duration;
use tokio::sync::mpsc;

/// Unified configuration that handles both test and production modes
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub mode: ExecutionMode,
    pub producer_config: ProducerConfig,
    pub request_interval: Duration,
    pub max_retries: u32,
    pub status_report_interval: Duration,
    pub routing_strategy: RoutingStrategy,
}

#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Standalone { max_iterations: Option<u32> },
    Production { orchestrator_endpoint: String },
}

/// Unified command source that abstracts test vs production command flows
#[derive(Debug)]
pub enum CommandSource {
    /// Commands from orchestrator (production mode)
    Orchestrator(mpsc::Receiver<ProducerCommand>),
    /// Simulated commands for test mode
    Simulator(CommandGenerator),
}

use crate::error::{ProducerError, ProducerResult};
use shared::types::RoutingStrategy;

impl ExecutionConfig {
    /// Parse command line arguments and environment to create unified config
    pub fn from_args_and_env(
        orchestrator_endpoint: Option<String>,
        topic: String,
        request_interval_secs: Option<u64>,
        max_requests: Option<u32>,
        routing_strategy: Option<RoutingStrategy>,
    ) -> ProducerResult<Self> {
        let orchestrator_addr = orchestrator_endpoint
            .as_ref()
            .map(|ep| ep.parse().map_err(|_| ProducerError::config("Invalid endpoint")))
            .transpose()?
            .unwrap_or_else(|| "127.0.0.1:6001".parse().unwrap());

        let producer_config = ProducerConfig::new(orchestrator_addr, topic);

        let mode = match orchestrator_endpoint {
            Some(endpoint) => ExecutionMode::Production {
                orchestrator_endpoint: endpoint,
            },
            None => ExecutionMode::Standalone {
                max_iterations: max_requests,
            },
        };

        Ok(ExecutionConfig {
            mode,
            producer_config,
            request_interval: Duration::from_secs(request_interval_secs.unwrap_or(2)),
            max_retries: 3,
            status_report_interval: Duration::from_secs(2),
            routing_strategy: routing_strategy.unwrap_or_else(|| Self::get_routing_strategy()),
        })
    }

    /// Get routing strategy from environment variables
    /// This replaces the old provider detection logic with explicit environment configuration
    pub fn get_routing_strategy() -> RoutingStrategy {
        RoutingStrategy::from_env().unwrap_or_else(|e| {
            eprintln!("Warning: Failed to load routing strategy from environment: {}. Using default backoff to random.", e);
            RoutingStrategy::Backoff { provider: shared::types::ProviderConfig::with_default_model(shared::ProviderId::Random) }
        })
    }
}

#[cfg(test)]
mod execution_config_tests {
    use super::*;

    #[test]
    fn test_execution_config_creation() {
        // Test production mode
        let config = ExecutionConfig::from_args_and_env(
            Some("127.0.0.1:6001".to_string()),
            "test topic".to_string(),
            Some(5),
            None,
            None,
        )
        .unwrap();

        match config.mode {
            ExecutionMode::Production { .. } => {}
            _ => panic!("Expected production mode"),
        }

        // Test standalone mode
        let config = ExecutionConfig::from_args_and_env(None, "test topic".to_string(), Some(2), Some(100), None).unwrap();

        match config.mode {
            ExecutionMode::Standalone {
                max_iterations: Some(100),
            } => {}
            _ => panic!("Expected standalone mode with max_iterations"),
        }
    }

    #[test]
    fn test_get_routing_strategy() {
        // Test environment-based routing strategy loading
        // In test mode (cfg!(test) = true), should use Random provider
        let strategy = ExecutionConfig::get_routing_strategy();
        
        match strategy {
            RoutingStrategy::Backoff { provider } => {
                // In test mode, should automatically use Random provider
                assert_eq!(provider.provider, ProviderId::Random);
            }
            _ => panic!("Expected backoff strategy with Random provider in test mode"),
        }
    }
}
