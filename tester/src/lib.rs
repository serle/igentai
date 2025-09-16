//! E2E Testing Framework
//! 
//! Comprehensive end-to-end testing framework for the distributed system.
//! 
//! ## Main Interface
//! 
//! The primary interface for writing E2E test scenarios is the [`Topic`] struct,
//! which provides access to trace events and output data for a completed topic execution.
//! 
//! ## Quick Start
//! 
//! ```rust
//! use tester::*;
//! 
//! // Configure orchestrator (CLI mode by default)
//! let config = OrchestratorConfig::builder()
//!     .topic("my_test")
//!     .producers(2)
//!     .iterations(Some(3))
//!     .build();
//! 
//! // Start constellation and wait for topic completion
//! let mut constellation = ServiceConstellation::new(trace_endpoint);
//! constellation.start_orchestrator(config).await?;
//! 
//! // Wait for topic and run assertions
//! if let Some(topic) = Topic::wait_for_topic("my_test", collector, timeout).await {
//!     assert!(topic.assert_completed().await);
//!     assert!(topic.assert_min_attributes(10));
//!     assert!(topic.assert_no_errors().await);
//! }
//! ```

// Core modules
pub mod config;
pub mod runtime;
pub mod testing;
pub mod scenarios;

// Main interfaces - re-exported at crate root for convenience
pub use testing::Topic;
pub use config::{OrchestratorConfig, OrchestratorMode, OrchestratorConfigBuilder};
pub use runtime::ServiceConstellation;

// Supporting types
pub use runtime::{TracingCollector, TraceQuery, CollectedEvent};
pub use testing::{TracingAssertions, AssertionResult};
pub use testing::{OutputLoader, OutputData, OutputMetadata, OutputComparison};
pub use scenarios::TestScenarios;

// Re-export web server testing function for convenience
pub use scenarios::web::server as run_webserver_test;