//! Runtime Management
//! 
//! This module handles process management and event collection during test execution.

pub mod constellation;
pub mod collector;
pub mod fault_injector_stub;

// Re-export main types
pub use constellation::ServiceConstellation;
pub use collector::{TracingCollector, TraceQuery, CollectedEvent, CollectorStats};
pub use fault_injector_stub::FaultInjector;