//! Producer library for generating and processing unique attributes
//!
//! This library provides a clean, testable Producer implementation that
//! communicates with external API providers and processes responses to
//! extract unique attributes using regex patterns and bloom filter deduplication.

pub mod core;
pub mod error;
pub mod services;
pub mod traits;
pub mod types;

// Re-export commonly used types
pub use core::{Metrics, Processor, Producer};
pub use error::{ProducerError, ProducerResult};
pub use services::{RealApiClient, RealCommunicator};
pub use traits::{ApiClient, Communicator};
pub use types::{
    ApiRequest, ApiResponse, CommandSource, ExecutionConfig, ExecutionMode, ProcessedAttribute, ProducerConfig,
    ProducerMetrics,
};
