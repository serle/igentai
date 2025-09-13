//! Shared types for LLM orchestration system
//!
//! Contains only truly shared types for inter-process communication.
//! Component-internal types (like Browser ↔ WebServer messages) are kept
//! in their respective components.

pub mod types;
pub mod errors;
pub mod messages;

pub use types::*;
pub use errors::*;

// Re-export only inter-process communication messages
pub use messages::{
    // Producer ↔ Orchestrator communication
    ProducerMessage, GenerationMetadata,
    
    // WebServer ↔ Orchestrator communication  
    TaskRequest, TaskUpdate,
    
    // Shared data structures
    SystemMetrics, LLMPerformance, AttributeUpdate, SystemHealth,
    
    // Process coordination types
    ProcessHandle, ProcessType, ProcessStatus, ProcessHealth, ChannelHealth,
    
    // Configuration types
    ProducerConfig, WebServerConfig, SessionState, SessionStatus,
};
