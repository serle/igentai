//! Testing Framework
//! 
//! This module provides the core testing interfaces and assertion capabilities.

pub mod topic;
pub mod assertions;
pub mod output;

// Re-export main types
pub use topic::Topic;
pub use assertions::{TracingAssertions, AssertionResult};
pub use output::{OutputLoader, OutputData, OutputMetadata, OutputComparison};