//! Optimization module containing traits, types, and implementations
//!
//! This module provides a clean separation between optimization interfaces,
//! data types, and concrete implementations.

pub mod traits;
pub mod types;
pub mod strategies;

pub use traits::OptimizerStrategy;
pub use types::*;