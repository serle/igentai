//! Orchestrator library for coordinating distributed attribute generation
//!
//! This library provides a clean, testable orchestrator implementation that
//! coordinates multiple producer processes to generate unique attributes efficiently
//! while managing costs and optimizing performance.

pub mod core;
pub mod error;
pub mod optimization;
pub mod orchestrator;
pub mod services;
pub mod traits;

// Re-export commonly used types
pub use core::{OrchestratorState, PerformanceTracker, UniquenessTracker};
pub use error::{OrchestratorError, OrchestratorResult};
pub use optimization::{OptimizerStrategy, OptimizationContext, OptimizationResult};
pub use orchestrator::Orchestrator;
pub use traits::{ApiKeySource, Communicator, FileSystem, ProcessManager};
