//! Core types used throughout the orchestrator system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// Global counter for producer numbering
static PRODUCER_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Global process ID singleton - set once at startup
static PROCESS_ID: OnceLock<ProcessId> = OnceLock::new();

/// Process identifier for any component in the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessId {
    /// Producer process with user-friendly number
    Producer(u32),
    /// Orchestrator process (singleton)
    Orchestrator,
    /// WebServer process (typically singleton)
    WebServer,
}

impl ProcessId {
    /// Initialize the global process ID for a producer with explicit ID
    pub fn init_producer(id: u32) -> &'static ProcessId {
        PROCESS_ID.get_or_init(|| ProcessId::Producer(id))
    }

    /// Initialize the global process ID for orchestrator
    pub fn init_orchestrator() -> &'static ProcessId {
        PROCESS_ID.get_or_init(|| ProcessId::Orchestrator)
    }

    /// Initialize the global process ID for webserver
    pub fn init_webserver() -> &'static ProcessId {
        PROCESS_ID.get_or_init(|| ProcessId::WebServer)
    }

    /// Get the global process ID (must be initialized first)
    pub fn current() -> &'static ProcessId {
        PROCESS_ID.get().expect("ProcessId not initialized - call init_* first")
    }

    /// Reset producer counter (useful for testing)
    pub fn reset_producer_counter() {
        PRODUCER_COUNTER.store(0, Ordering::SeqCst);
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessId::Producer(id) => write!(f, "producer_{id}"),
            ProcessId::Orchestrator => write!(f, "orchestrator"),
            ProcessId::WebServer => write!(f, "webserver"),
        }
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        ProcessId::Producer(1)
    }
}

/// LLM providers available in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    OpenAI,
    Anthropic,
    Gemini,
    Random,
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderId::OpenAI => write!(f, "openai"),
            ProviderId::Anthropic => write!(f, "anthropic"),
            ProviderId::Gemini => write!(f, "gemini"),
            ProviderId::Random => write!(f, "random"),
        }
    }
}

impl std::str::FromStr for ProviderId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(ProviderId::OpenAI),
            "anthropic" => Ok(ProviderId::Anthropic),
            "gemini" | "google" => Ok(ProviderId::Gemini),
            "random" => Ok(ProviderId::Random),
            _ => Err(format!("Unknown provider: {s}")),
        }
    }
}

/// Token usage information for LLM requests
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Metadata about a provider request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub provider_id: ProviderId,
    pub model: String,
    pub response_time_ms: u64,
    pub tokens: TokenUsage,
    pub request_timestamp: u64,
}

/// Alias for compatibility
pub type ProviderRequestMetadata = ProviderMetadata;

/// Status of a producer process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

/// Status of an LLM provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderStatus {
    Available,
    RateLimited,
    Error,
    Offline,
    Healthy,
    Unknown,
}

/// Optimization mode for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationMode {
    /// Maximize unique attributes per minute within budget
    MaximizeUAM { budget_per_minute: f64 },

    /// Minimize cost while maintaining UAM target
    MinimizeCost { target_uam: f64 },

    /// Balance efficiency (unique attributes per dollar)
    MaximizeEfficiency,

    /// Custom weighted optimization
    Weighted {
        uam_weight: f64,
        cost_weight: f64,
        token_weight: f64,
    },
}

/// Generation constraints and targets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConstraints {
    /// Maximum cost per minute (USD)
    pub max_cost_per_minute: f64,

    /// Target unique attributes per minute
    pub target_uam: f64,

    /// Maximum total runtime (seconds)
    pub max_runtime_seconds: Option<u64>,
}

/// Provider configuration with model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProviderConfig {
    pub provider: ProviderId,
    pub model: String,
}

impl ProviderConfig {
    pub fn new(provider: ProviderId, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }
    
    pub fn with_default_model(provider: ProviderId) -> Self {
        let model = match provider {
            ProviderId::OpenAI => "gpt-4o-mini",
            ProviderId::Anthropic => "claude-3-sonnet",
            ProviderId::Gemini => "gemini-pro",
            ProviderId::Random => "random",
        };
        Self::new(provider, model)
    }
}

/// Routing strategy for distributing work to providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Round-robin through providers with their models
    RoundRobin { providers: Vec<ProviderConfig> },

    /// Priority order (try first, then fallback) with models
    PriorityOrder { providers: Vec<ProviderConfig> },

    /// Weighted distribution with models
    Weighted { weights: HashMap<ProviderConfig, f32> },

    /// Single provider with exponential backoff (ideal for test mode)
    Backoff { provider: ProviderConfig },
}

impl RoutingStrategy {
    /// Load routing strategy from environment variables
    /// 
    /// Environment variables:
    /// - ROUTING_STRATEGY: roundrobin|priority|weighted|backoff (default: fallback to backoff/random)
    /// - ROUTING_CONFIG: Provider configuration string (format: "provider:model" or "provider1:model1,provider2:model2")
    pub fn from_env() -> Result<Self, String> {
        use std::env;
        
        // If no routing strategy is set, fallback to backoff with random (test mode)
        let strategy_type = match env::var("ROUTING_STRATEGY") {
            Ok(strategy) => strategy.to_lowercase(),
            Err(_) => return Ok(Self::Backoff { provider: ProviderConfig { provider: ProviderId::Random, model: "random".to_string() } }),
        };
        
        let routing_config = env::var("ROUTING_CONFIG")
            .unwrap_or_else(|_| "random:random".to_string());
        
        match strategy_type.as_str() {
            "backoff" => {
                let provider_config = Self::parse_provider_config(&routing_config)?;
                Ok(Self::Backoff { provider: provider_config })
            }
            "roundrobin" => {
                let providers = Self::parse_provider_config_list(&routing_config)?;
                if providers.is_empty() {
                    return Err("ROUTING_CONFIG must specify providers for roundrobin strategy".to_string());
                }
                Ok(Self::RoundRobin { providers })
            }
            "priority" => {
                let providers = Self::parse_provider_config_list(&routing_config)?;
                if providers.is_empty() {
                    return Err("ROUTING_CONFIG must specify providers for priority strategy".to_string());
                }
                Ok(Self::PriorityOrder { providers })
            }
            "weighted" => {
                let weights = Self::parse_weighted_provider_config(&routing_config)?;
                if weights.is_empty() {
                    return Err("ROUTING_CONFIG must specify weighted providers for weighted strategy".to_string());
                }
                Ok(Self::Weighted { weights })
            }
            _ => Err(format!("Unknown routing strategy '{}'. Valid options: backoff, roundrobin, priority, weighted", strategy_type)),
        }
    }
    
    /// Parse single provider config from format "provider:model"
    fn parse_provider_config(config: &str) -> Result<ProviderConfig, String> {
        let parts: Vec<&str> = config.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid provider config format '{}'. Expected 'provider:model'", config));
        }
        
        let provider: ProviderId = parts[0].trim().parse()
            .map_err(|e| format!("Invalid provider '{}': {}", parts[0], e))?;
        let model = parts[1].trim().to_string();
        
        Ok(ProviderConfig { provider, model })
    }
    
    /// Parse comma-separated provider config list from format "provider1:model1,provider2:model2"
    fn parse_provider_config_list(config: &str) -> Result<Vec<ProviderConfig>, String> {
        config
            .split(',')
            .map(|item| Self::parse_provider_config(item.trim()))
            .collect()
    }
    
    /// Parse weighted provider config from format "provider1:model1:weight1,provider2:model2:weight2"
    fn parse_weighted_provider_config(config: &str) -> Result<HashMap<ProviderConfig, f32>, String> {
        let mut weights = HashMap::new();
        
        for item in config.split(',') {
            let parts: Vec<&str> = item.split(':').collect();
            if parts.len() != 3 {
                return Err(format!("Invalid weighted config format '{}'. Expected 'provider:model:weight'", item));
            }
            
            let provider: ProviderId = parts[0].trim().parse()
                .map_err(|e| format!("Invalid provider '{}': {}", parts[0], e))?;
            let model = parts[1].trim().to_string();
            let weight: f32 = parts[2].trim().parse()
                .map_err(|e| format!("Invalid weight '{}': {}", parts[2], e))?;
            
            if weight < 0.0 || weight > 1.0 {
                return Err(format!("Weight {} must be between 0.0 and 1.0", weight));
            }
            
            let provider_config = ProviderConfig { provider, model };
            weights.insert(provider_config, weight);
        }
        
        // Validate weights sum to approximately 1.0
        let sum: f32 = weights.values().sum();
        if (sum - 1.0).abs() > 0.01 {
            return Err(format!("Weights sum to {:.3}, but should sum to 1.0", sum));
        }
        
        Ok(weights)
    }
    
}

/// Generation configuration for providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub model: String,
    pub batch_size: u32,
    pub context_window: u32,
    pub max_tokens: u32,
    pub temperature: f32,
    pub request_size: usize, // Number of words/items to request
}

/// Performance metrics for a producer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerMetrics {
    pub uam: f64, // Unique attributes per minute
    pub tokens_per_minute: f64,
    pub cost_per_minute: f64,
    pub unique_per_dollar: f64,
    pub unique_per_1k_tokens: f64,
    pub uniqueness_ratio: f64, // unique/total ratio
    pub status: ProcessStatus,
    pub last_activity: u64, // timestamp
}

/// Performance metrics for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    pub uam: f64,
    pub tokens_per_minute: f64,
    pub cost_per_minute: f64,
    pub unique_per_dollar: f64,
    pub unique_per_1k_tokens: f64,
    pub avg_response_time_ms: f64,
    pub success_rate: f64,
    pub status: ProviderStatus,
}

/// System-wide performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Overall system performance
    pub uam: f64,
    pub cost_per_minute: f64,
    pub tokens_per_minute: f64,
    pub unique_per_dollar: f64,
    pub unique_per_1k_tokens: f64,

    /// Breakdown by producer
    pub by_producer: HashMap<String, ProducerMetrics>,

    /// Breakdown by provider
    pub by_provider: HashMap<ProviderId, ProviderMetrics>,

    /// System state
    pub active_producers: u32,
    pub current_topic: Option<String>,
    pub uptime_seconds: u64,
    pub last_updated: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            uam: 0.0,
            cost_per_minute: 0.0,
            tokens_per_minute: 0.0,
            unique_per_dollar: 0.0,
            unique_per_1k_tokens: 0.0,
            by_producer: HashMap::new(),
            by_provider: HashMap::new(),
            active_producers: 0,
            current_topic: None,
            uptime_seconds: 0,
            last_updated: 0,
        }
    }
}

/// Request configuration for provider calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestConfig {
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: 1000,
            temperature: 0.7,
            timeout_seconds: 30,
        }
    }
}

/// API failure types for error handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiFailure {
    RateLimitExceeded,
    InvalidApiKey,
    NetworkTimeout,
    ModelUnavailable,
    InvalidRequest,
    InternalError,
}

impl fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiFailure::RateLimitExceeded => write!(f, "rate limit exceeded"),
            ApiFailure::InvalidApiKey => write!(f, "invalid API key"),
            ApiFailure::NetworkTimeout => write!(f, "network timeout"),
            ApiFailure::ModelUnavailable => write!(f, "model unavailable"),
            ApiFailure::InvalidRequest => write!(f, "invalid request"),
            ApiFailure::InternalError => write!(f, "internal error"),
        }
    }
}

/// Shared error type for cross-package compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SharedError {}
