//! Orchestrator library for coordinating distributed attribute generation
//! 
//! This library provides a clean, testable orchestrator implementation that
//! coordinates multiple producer processes to generate unique attributes efficiently
//! while managing costs and optimizing performance.

pub mod error;
pub mod core;
pub mod traits;
pub mod services;
pub mod orchestrator;

// Re-export commonly used types
pub use error::{OrchestratorError, OrchestratorResult};
pub use core::{OrchestratorState, UniquenessTracker, PerformanceTracker, Optimizer};
pub use traits::{ApiKeySource, Communicator, FileSystem, ProcessManager};
pub use orchestrator::Orchestrator;