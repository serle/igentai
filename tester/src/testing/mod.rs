//! Testing Framework
//!
//! This module provides the core testing interfaces and assertion capabilities.

pub mod assertions;
pub mod output;
pub mod topic;

// Re-export main types
pub use assertions::{AssertionResult, TracingAssertions};
pub use output::{OutputComparison, OutputData, OutputLoader, OutputMetadata};
pub use topic::Topic;
