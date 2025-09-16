//! Configuration Management
//!
//! This module provides configuration structures and builders for orchestrator setup.

pub mod builder;
pub mod builder_extensions;
pub mod fault_tolerance;
pub mod orchestrator;

// Re-export main types
pub use builder::OrchestratorConfigBuilder;
pub use orchestrator::{OrchestratorConfig, OrchestratorMode};
