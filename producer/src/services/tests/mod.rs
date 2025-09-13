//! Comprehensive tests for producer services
//!
//! These tests verify the functionality of all producer service components,
//! including mocking external dependencies and error conditions.

pub mod provider_router;
pub mod response_processor;
pub mod ipc_communicator;
pub mod performance_tracker;

// Re-export test utilities
pub use crate::traits::*;