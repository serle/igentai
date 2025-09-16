//! Core business logic modules
//!
//! This module contains pure business logic with no I/O dependencies.
//! All functions are deterministic and easily testable.

pub mod optimizer;
pub mod performance;
pub mod state;
pub mod uniqueness;

pub use optimizer::Optimizer;
pub use performance::PerformanceTracker;
pub use state::OrchestratorState;
pub use uniqueness::UniquenessTracker;
