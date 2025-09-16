//! Configuration Management
//! 
//! This module provides configuration structures and builders for orchestrator setup.

pub mod builder;
pub mod orchestrator;
pub mod fault_tolerance;
pub mod builder_extensions;

// Re-export main types
pub use orchestrator::{OrchestratorConfig, OrchestratorMode};
pub use builder::OrchestratorConfigBuilder;