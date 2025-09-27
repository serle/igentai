//! Concrete optimization strategy implementations
//!
//! This module contains different optimization strategy implementations,
//! each following the OptimizerStrategy trait.

pub mod basic;
pub mod adaptive;

pub use basic::BasicOptimizer;
pub use adaptive::AdaptiveOptimizer;