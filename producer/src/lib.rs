//! Producer library for LLM orchestration system
//!
//! This library provides services for generating unique attributes using multiple LLM providers
//! with intelligent routing and communication with the orchestrator.

pub mod error;
pub mod types;
pub mod traits;
pub mod state;
pub mod producer_impl;
pub mod services;

// Re-export main types
pub use error::{ProducerError, ProducerResult};
pub use types::*;
pub use traits::*;
pub use types::ProducerState;
pub use producer_impl::Producer;
pub use services::*;