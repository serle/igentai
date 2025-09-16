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
