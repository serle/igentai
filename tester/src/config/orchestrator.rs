//! Orchestrator Configuration
//!
//! Configuration structure for orchestrator command-line arguments

use super::fault_tolerance::FaultToleranceConfig;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum OrchestratorMode {
    /// CLI mode - runs for specific iterations then exits
    Cli,
    /// WebServer mode - runs continuously, controlled via HTTP API
    WebServer,
}

impl Default for OrchestratorMode {
    fn default() -> Self {
        OrchestratorMode::Cli
    }
}

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub mode: OrchestratorMode,
    pub provider: String,
    pub trace_endpoint: Option<String>,
    pub topic: Option<String>,
    pub producers: Option<usize>,
    pub iterations: Option<usize>,
    pub request_size: Option<usize>,
    pub output: Option<String>,
    pub webserver_addr: Option<String>,
    pub producer_addr: Option<String>,
    pub log_level: String,
    pub max_duration: Duration,
    pub fault_tolerance: Option<FaultToleranceConfig>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            mode: OrchestratorMode::default(),
            provider: "random".to_string(),
            trace_endpoint: None,
            topic: None,
            producers: None,
            iterations: None,
            request_size: None,
            output: None,
            webserver_addr: None,
            producer_addr: None,
            log_level: "debug".to_string(),        // Default to debug for testing
            max_duration: Duration::from_secs(60), // Default 1 minute timeout
            fault_tolerance: None,
        }
    }
}

impl OrchestratorConfig {
    /// Create a new builder
    pub fn builder() -> crate::config::builder::OrchestratorConfigBuilder {
        crate::config::builder::OrchestratorConfigBuilder::new()
    }

    /// Convert to command-line arguments based on mode
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        args.push("--provider".to_string());
        args.push(self.provider.clone());

        if let Some(ref endpoint) = self.trace_endpoint {
            args.push("--trace-ep".to_string());
            args.push(endpoint.clone());
        }

        if let Some(ref topic) = self.topic {
            args.push("--topic".to_string());
            args.push(topic.clone());
        }

        if let Some(producers) = self.producers {
            args.push("--producers".to_string());
            args.push(producers.to_string());
        }

        if let Some(iterations) = self.iterations {
            args.push("--iterations".to_string());
            args.push(iterations.to_string());
        }

        if let Some(request_size) = self.request_size {
            args.push("--request-size".to_string());
            args.push(request_size.to_string());
        }

        args.push("--log-level".to_string());
        args.push(self.log_level.clone());

        if let Some(ref output) = self.output {
            args.push("--output".to_string());
            args.push(output.clone());
        }

        // Add mode-specific arguments
        match self.mode {
            OrchestratorMode::WebServer => {
                if let Some(ref addr) = self.webserver_addr {
                    args.push("--webserver-addr".to_string());
                    args.push(addr.clone());
                }

                if let Some(ref addr) = self.producer_addr {
                    args.push("--producer-addr".to_string());
                    args.push(addr.clone());
                }
            }
            OrchestratorMode::Cli => {
                // CLI mode uses topic, producers, iterations directly
            }
        }

        args
    }

    /// Get the output directory path
    pub fn get_output_dir(&self) -> String {
        if let Some(ref output) = self.output {
            output.clone()
        } else if let Some(ref topic) = self.topic {
            format!("./output/{}", topic)
        } else {
            "./output/default".to_string()
        }
    }

    /// Get the topic name (required for most operations)
    pub fn get_topic(&self) -> Option<&str> {
        self.topic.as_deref()
    }

    /// Check if this configuration is valid
    pub fn is_valid(&self) -> bool {
        match self.mode {
            OrchestratorMode::Cli => self.topic.is_some(),
            OrchestratorMode::WebServer => {
                true // No required fields for WebServer mode, all have defaults
            }
        }
    }
}
