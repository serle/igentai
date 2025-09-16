//! Core business logic modules
//! 
//! Pure business logic with no I/O dependencies

pub mod state;
pub mod analytics;

// Re-export commonly used types
pub use state::{WebServerState, TimestampedMetrics};
pub use analytics::AnalyticsEngine;