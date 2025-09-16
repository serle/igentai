//! Producer core business logic

pub mod producer;
pub mod processor;
pub mod metrics;
pub mod prompt;
pub mod utils;
pub mod generator;

pub use producer::Producer;
pub use processor::Processor;
pub use metrics::Metrics;
pub use prompt::PromptHandler;
pub use utils::{
    auto_select_routing_strategy, 
    select_provider, 
    build_api_request, 
    should_retry_request
};
pub use generator::CommandGenerator;