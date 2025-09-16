//! Producer library for generating and processing unique attributes
//! 
//! This library provides a clean, testable Producer implementation that
//! communicates with external API providers and processes responses to
//! extract unique attributes using regex patterns and bloom filter deduplication.

pub mod error;
pub mod types;
pub mod traits;
pub mod core;
pub mod services;

// Re-export commonly used types
pub use error::{ProducerError, ProducerResult};
pub use types::{ProducerConfig, ProducerMetrics, ApiRequest, ApiResponse, ProcessedAttribute, ExecutionConfig, ExecutionMode, CommandSource};
pub use traits::{ApiClient, Communicator};
pub use core::{Producer, Processor, Metrics};
pub use services::{RealApiClient, RealCommunicator};


