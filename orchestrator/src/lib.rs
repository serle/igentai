//! Orchestrator library
//!
//! This library provides the core orchestration functionality for the LLM system.
//! It exposes traits and implementations needed for testing and integration.

pub mod traits;
pub mod error;
pub mod state;
pub mod services;
pub mod orchestrator_impl;

// Re-export main types for easy access
pub use traits::*;
pub use error::*;
pub use state::*;
pub use orchestrator_impl::Orchestrator;