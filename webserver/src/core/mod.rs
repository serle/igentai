//! Core business logic modules
//!
//! Pure business logic with no I/O dependencies

pub mod analytics;
pub mod state;

// Re-export commonly used types
pub use analytics::AnalyticsEngine;
pub use state::{TimestampedMetrics, WebServerState};
