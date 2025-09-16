//! Service implementations
//!
//! Real implementations of all service traits for production use

pub mod orchestrator_client;
pub mod static_server;
pub mod websocket_manager;

// Re-export service implementations
pub use orchestrator_client::RealOrchestratorClient;
pub use static_server::RealStaticFileServer;
pub use websocket_manager::RealWebSocketManager;
