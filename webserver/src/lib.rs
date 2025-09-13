//! Webserver library for LLM Orchestration System
//!
//! This library provides a web-based dashboard for monitoring and controlling
//! the LLM orchestration system, with real-time updates and WebSocket communication.

pub mod error;
pub mod services;
pub mod traits;
pub mod types;
pub mod webserver_impl;
pub mod state;

// Re-export main types
pub use error::{WebServerError, WebServerResult};
pub use webserver_impl::WebServer;
pub use state::WebServerState;
pub use types::*;

// Re-export trait definitions
pub use traits::{
    FileManager, ClientBroadcaster, IpcCommunicator, 
    ClientRegistry, MetricsAggregator
};

// Re-export service implementations
pub use services::{
    RealFileManager, RealClientBroadcaster, RealIpcCommunicator,
    RealClientRegistry, RealMetricsAggregator
};

