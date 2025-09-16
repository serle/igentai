//! Service implementations
//! 
//! Real implementations of all service traits for production use

pub mod orchestrator_client;
pub mod websocket_manager;
pub mod static_server;

// Re-export service implementations
pub use orchestrator_client::RealOrchestratorClient;
pub use websocket_manager::RealWebSocketManager;
pub use static_server::RealStaticFileServer;