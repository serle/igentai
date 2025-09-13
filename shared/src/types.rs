//! Core shared types and identifiers

use serde::{Deserialize, Serialize};
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

/// Configuration for LLM generation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub batch_size: u32,        // Attributes to request per generation
    pub temperature: f32,       // LLM temperature
    pub max_tokens: u32,       // Response length limit
    pub context_window: usize, // How many existing attributes to include
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            batch_size: 10,
            temperature: 0.7,
            max_tokens: 1000,
            context_window: 100,
        }
    }
}

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