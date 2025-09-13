//! Message types for the LLM orchestration system
//!
//! This module organizes all inter-process communication messages by category:
//! - `metrics`: System performance and monitoring data
//! - `producer`: Orchestrator ↔ Producer communication
//! - `webserver`: Orchestrator ↔ WebServer communication  
//! - `process`: Process lifecycle and health monitoring
//! - `config`: Configuration and state management

pub mod metrics;
pub mod producer;
pub mod webserver;
pub mod process;
pub mod config;

// Re-export commonly used types at module level for convenience
pub use metrics::{
    SystemMetrics, LLMPerformance, AttributeUpdate, SystemHealth,
    ProviderRequestMetadata, BloomFilterStats,
};

pub use producer::{ProducerCommand, ProducerUpdate, ProducerMessage};

pub use webserver::{TaskRequest, TaskUpdate};

pub use process::{
    ProcessHandle, ProcessType, ProcessStatus, ProcessHealth, 
    ChannelHealth, ChannelType,
};

pub use config::{
    ProducerConfig, WebServerConfig, SessionState, SessionStatus,
};