//! Runtime Management
//!
//! This module handles process management and event collection during test execution.

pub mod cleanup;
pub mod collector;
pub mod constellation;
pub mod fault_injector_stub;

// Re-export main types
pub use cleanup::CleanupManager;
pub use collector::{CollectedEvent, CollectorStats, TraceQuery, TracingCollector};
pub use constellation::ServiceConstellation;
pub use fault_injector_stub::FaultInjector;
