//! Core shared types and identifiers

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

/// Unique identifier for producer processes
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProducerId(Uuid);

impl ProducerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl fmt::Display for ProducerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for client connections
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration for provider-specific requests
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestConfig {
    pub batch_size: u32,        // Attributes to request per generation
    pub temperature: f32,       // LLM temperature
    pub max_tokens: u32,       // Response length limit
    pub context_window: usize, // How many existing attributes to include
    pub model: String,         // Model name to use for requests
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            temperature: 0.7,
            max_tokens: 1000,
            context_window: 100,
            model: "default".to_string(),
        }
    }
}

/// Legacy alias for backward compatibility
pub type GenerationConfig = RequestConfig;

/// Status of producer processes
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProducerStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed(String),
}

/// Status of LLM providers
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProviderStatus {
    Unknown,
    Healthy,
    Degraded,
    Unhealthy,
}

/// Alert levels for system messages
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Success,
}

/// Key-value pair for API keys and configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValuePair {
    pub key: String,
    pub value: String,
}

/// Identifier for downstream LLM providers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderId {
    OpenAI,
    Anthropic,
    Gemini,
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderId::OpenAI => write!(f, "openai"),
            ProviderId::Anthropic => write!(f, "anthropic"),
            ProviderId::Gemini => write!(f, "gemini"),
        }
    }
}

impl ProviderId {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(ProviderId::OpenAI),
            "anthropic" => Some(ProviderId::Anthropic),
            "gemini" => Some(ProviderId::Gemini),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderId::OpenAI => "openai",
            ProviderId::Anthropic => "anthropic",
            ProviderId::Gemini => "gemini",
        }
    }
}

/// Routing strategy for provider selection with embedded provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoutingStrategy {
    /// Select providers in priority order (first provider has highest priority)
    PriorityOrder { 
        providers: Vec<ProviderId> 
    },
    /// Round-robin selection through ordered providers
    RoundRobin { 
        providers: Vec<ProviderId> 
    },
    /// Weighted random selection
    Weighted { 
        weights: HashMap<ProviderId, f32> 
    },
}

/// API failure reasons for LLM provider requests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiFailure {
    /// Authentication failed (invalid API key)
    AuthenticationFailed,
    /// Rate limit exceeded
    RateLimitExceeded,
    /// Request quota exceeded
    QuotaExceeded,
    /// Invalid request format or parameters
    InvalidRequest(String),
    /// Model not found or unavailable
    ModelUnavailable(String),
    /// Network/connection error
    NetworkError(String),
    /// Server error from provider
    ServerError(String),
    /// Request timeout
    Timeout,
    /// Content policy violation
    ContentPolicyViolation,
    /// Service temporarily unavailable
    ServiceUnavailable,
    /// Unknown or unhandled error
    Unknown(String),
}