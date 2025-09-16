//! Orchestrator library for coordinating distributed attribute generation
//!
//! This library provides a clean, testable orchestrator implementation that
//! coordinates multiple producer processes to generate unique attributes efficiently
//! while managing costs and optimizing performance.

pub mod core;
pub mod error;
pub mod orchestrator;
pub mod services;
pub mod traits;

// Re-export commonly used types
pub use core::{Optimizer, OrchestratorState, PerformanceTracker, UniquenessTracker};
pub use error::{OrchestratorError, OrchestratorResult};
pub use orchestrator::Orchestrator;
pub use traits::{ApiKeySource, Communicator, FileSystem, ProcessManager};
