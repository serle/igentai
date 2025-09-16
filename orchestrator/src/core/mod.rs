//! Core business logic modules
//! 
//! This module contains pure business logic with no I/O dependencies.
//! All functions are deterministic and easily testable.

pub mod state;
pub mod uniqueness;
pub mod performance;
pub mod optimizer;

pub use state::OrchestratorState;
pub use uniqueness::UniquenessTracker;
pub use performance::PerformanceTracker;
pub use optimizer::Optimizer;