//! WebServer library for serving the orchestrator dashboard
//!
//! This library provides a clean, testable WebServer implementation that
//! communicates with the orchestrator and serves a rich dashboard interface
//! to browser clients via WebSockets and REST APIs.

pub mod core;
pub mod error;
pub mod services;
pub mod traits;
pub mod types;
pub mod web;

// Re-export commonly used types
pub use core::{AnalyticsEngine, WebServerState};
pub use error::{WebServerError, WebServerResult};
pub use traits::{OrchestratorClient, StaticFileServer, WebSocketManager};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::debug;

use shared::{OrchestratorUpdate, ProcessId, process_debug};
use crate::types::ClientMessage;
// Handler wrapper functions for AppState
use axum::Json;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Html, Response};
use serde_json::Value;

/// Combined application state for Axum router - using Arc for cloning
struct AppState<O, W, S>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    orchestrator_client: Arc<tokio::sync::Mutex<O>>,
    websocket_manager: Arc<W>,
    static_server: Arc<S>,
}

// Manual Clone implementation for AppState
impl<O, W, S> Clone for AppState<O, W, S>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            orchestrator_client: self.orchestrator_client.clone(),
            websocket_manager: self.websocket_manager.clone(),
            static_server: self.static_server.clone(),
        }
    }
}

/// Main WebServer that coordinates the entire system
pub struct WebServer<O, W, S>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    /// Core state management
    state: Arc<Mutex<WebServerState>>,

    /// Analytics engine for insights
    #[allow(dead_code)]
    analytics: AnalyticsEngine,

    /// Injected services
    orchestrator_client: Arc<Mutex<O>>,
    websocket_manager: Arc<W>,
    static_server: Arc<S>,

    /// Shutdown signal
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl<O, W, S> WebServer<O, W, S>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    /// Create new WebServer with injected dependencies
    pub fn new(
        state: WebServerState,
        analytics: AnalyticsEngine,
        orchestrator_client: O,
        websocket_manager: W,
        static_server: S,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            state: Arc::new(Mutex::new(state)),
            analytics,
            orchestrator_client: Arc::new(Mutex::new(orchestrator_client)),
            websocket_manager: Arc::new(websocket_manager),
            static_server: Arc::new(static_server),
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Get shutdown sender for external shutdown requests
    pub fn get_shutdown_sender(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Main WebServer run loop
    pub async fn run(&mut self, http_addr: SocketAddr, standalone_mode: bool) -> WebServerResult<()> {
        shared::process_info!(
            shared::ProcessId::current(),
            "🚀 Starting WebServer initialization (standalone_mode: {})",
            standalone_mode
        );

        let mut orchestrator_updates = if standalone_mode {
            shared::process_info!(
                shared::ProcessId::current(),
                "🔧 Standalone mode - skipping orchestrator connection"
            );
            // Create a dummy receiver that will never receive messages but keeps sender alive
            let (tx, rx) = mpsc::channel(1);
            // Keep the sender alive by storing it
            std::mem::forget(tx);
            rx
        } else {
            // Initialize orchestrator connection
            match {
                let mut client = self.orchestrator_client.lock().await;
                client.initialize().await
            } {
                Ok(_) => {
                    // Get orchestrator update receiver
                    let mut client = self.orchestrator_client.lock().await;
                    match client.get_updates().await {
                        Ok(updates) => {
                            // Update state with connection status
                            {
                                let mut state = self.state.lock().await;
                                state.set_orchestrator_connected(true);
                            }
                            shared::process_info!(
                                shared::ProcessId::current(),
                                "✅ Connected to orchestrator successfully"
                            );
                            updates
                        }
                        Err(e) => {
                            shared::process_error!(
                                shared::ProcessId::current(),
                                "⚠️ Failed to get orchestrator updates, continuing in offline mode: {}",
                                e
                            );
                            let (tx, rx) = mpsc::channel(1);
                            std::mem::forget(tx);
                            rx
                        }
                    }
                }
                Err(e) => {
                    shared::process_error!(
                        shared::ProcessId::current(),
                        "⚠️ Failed to connect to orchestrator, continuing in offline mode: {}",
                        e
                    );
                    let (tx, rx) = mpsc::channel(1);
                    std::mem::forget(tx);
                    rx
                }
            }
        };

        // Start HTTP server
        let app = self.create_axum_app();
        let listener = tokio::net::TcpListener::bind(&http_addr)
            .await
            .map_err(|e| WebServerError::http(format!("Failed to bind to {}: {}", http_addr, e)))?;

        shared::process_info!(
            shared::ProcessId::current(),
            "🌐 WebServer HTTP listening on {}", 
            http_addr
        );

        // Start background tasks - DISABLED FOR DEBUGGING
        // let analytics_task = self.start_analytics_task();
        // let health_check_task = self.start_health_check_task();

        // Spawn the HTTP server task
        let mut server_handle = tokio::spawn(async move {
            shared::process_info!(shared::ProcessId::current(), "🚀 Starting axum HTTP server...");
            shared::process_info!(shared::ProcessId::current(), "About to call axum::serve...");

            match axum::serve(listener, app.into_make_service()).await {
                Ok(_) => {
                    shared::process_info!(shared::ProcessId::current(), "✅ Axum server completed normally");
                }
                Err(e) => {
                    shared::process_error!(shared::ProcessId::current(), "❌ Axum server error: {}", e);
                }
            }

            shared::process_info!(shared::ProcessId::current(), "🏁 Server task finishing");
        });

        // Main event loop
        if standalone_mode {
            // In standalone mode, just wait for server to complete or shutdown
            loop {
                tokio::select! {
                    // Handle shutdown signal
                    Some(_) = self.shutdown_rx.recv() => {
                        shared::process_info!(shared::ProcessId::current(), "🛑 Shutting down WebServer...");
                        break;
                    },

                    // Handle server completion
                    result = &mut server_handle => {
                        match result {
                            Ok(()) => shared::process_info!(shared::ProcessId::current(), "HTTP server completed successfully"),
                            Err(e) => shared::process_error!(shared::ProcessId::current(), "HTTP server task error: {}", e),
                        }
                        break;
                    }
                }
            }
        } else {
            // Normal mode with orchestrator updates
            loop {
                tokio::select! {
                    // Handle orchestrator updates
                    Some(update) = orchestrator_updates.recv() => {
                        debug!("📨 Received orchestrator update in webserver main loop");
                        if let Err(e) = self.handle_orchestrator_update(update).await {
                            shared::process_error!(shared::ProcessId::current(), "❌ Error handling orchestrator update: {}", e);
                        }
                    },

                    // Handle shutdown signal
                    Some(_) = self.shutdown_rx.recv() => {
                        shared::process_info!(shared::ProcessId::current(), "🛑 Shutting down WebServer...");
                        break;
                    },

                    // Handle server completion
                    result = &mut server_handle => {
                        match result {
                            Ok(()) => shared::process_info!(shared::ProcessId::current(), "HTTP server completed successfully"),
                            Err(e) => shared::process_error!(shared::ProcessId::current(), "HTTP server task error: {}", e),
                        }
                        break;
                    }
                }
            }
        }

        // Cleanup
        server_handle.abort();
        // analytics_task.abort();
        // health_check_task.abort();

        shared::process_info!(shared::ProcessId::current(), "✅ WebServer shutdown complete");
        Ok(())
    }

    /// Handle orchestrator update and broadcast to clients
    async fn handle_orchestrator_update(&self, update: OrchestratorUpdate) -> WebServerResult<()> {
        let update_type = match &update {
            OrchestratorUpdate::StatisticsUpdate { .. } => "StatisticsUpdate",
            OrchestratorUpdate::NewAttributes { .. } => "NewAttributes", 
            OrchestratorUpdate::GenerationComplete { .. } => "GenerationComplete",
            OrchestratorUpdate::ErrorNotification(_) => "ErrorNotification",
            _ => "Other",
        };
        
        process_debug!(ProcessId::current(), "📨 Processing orchestrator update: {}", update_type);
        process_debug!(ProcessId::current(), "📨 Full orchestrator update: {:?}", update);

        // Check connected clients
        let client_count = self.websocket_manager.client_count().await;
        shared::process_info!(shared::ProcessId::current(), "📊 Processing update with {} WebSocket clients connected", client_count);

        // Process update through state
        let client_messages = {
            let mut state = self.state.lock().await;
            state.process_orchestrator_update(update)
        };

        process_debug!(ProcessId::current(), "📤 Broadcasting {} client messages to {} clients", client_messages.len(), client_count);

        // Broadcast messages to connected clients
        for message in client_messages {
            match &message {
                ClientMessage::StatisticsUpdate { metrics, .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting StatisticsUpdate: UAM={:.2}, cost/min=${:.4}", 
                          metrics.uam, metrics.cost_per_minute);
                }
                ClientMessage::AttributeUpdate { attributes, .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting AttributeUpdate: {} attributes", attributes.len());
                }
                ClientMessage::GenerationComplete { topic, .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting GenerationComplete: topic={}", topic);
                }
                ClientMessage::Alert { level, title, .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting Alert: level={:?}, title={}", level, title);
                }
                ClientMessage::StatusUpdate { .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting StatusUpdate");
                }
                ClientMessage::DashboardUpdate { .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting DashboardUpdate");
                }
                ClientMessage::ConnectionAck { .. } => {
                    process_debug!(ProcessId::current(), "🔄 Broadcasting ConnectionAck");
                }
            }
            debug!("Broadcasting message: {:?}", message);
            self.websocket_manager.broadcast(message).await?;
        }

        Ok(())
    }

    /// Create Axum application with routes
    fn create_axum_app(&self) -> axum::Router {
        use axum::{
            Router,
            routing::{get, post},
        };

        // Create combined state for the router
        let app_state = AppState {
            orchestrator_client: self.orchestrator_client.clone(),
            websocket_manager: self.websocket_manager.clone(),
            static_server: self.static_server.clone(),
        };

        Router::new()
            .route("/", get(serve_index_wrapper))
            .route("/ws", get(websocket_handler_wrapper))
            .route("/api/dashboard", get(get_dashboard_wrapper))
            .route("/api/status", get(get_status_wrapper))
            .route("/api/start", post(start_generation_wrapper))
            .route("/api/stop", post(stop_generation_wrapper))
            .route("/static/*path", get(serve_static_wrapper))
            .route("/test", get(|| async { "WebServer is running!" }))
            .with_state(app_state)
    }
}

// Handler wrapper functions for Axum routes

async fn websocket_handler_wrapper<O, W, S>(
    ws: WebSocketUpgrade,
    State(app_state): State<AppState<O, W, S>>,
) -> Response
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::websocket::websocket_handler(ws, State(app_state.websocket_manager)).await
}

async fn get_dashboard_wrapper<O, W, S>(State(app_state): State<AppState<O, W, S>>) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::api::get_dashboard(State(app_state.websocket_manager)).await
}

async fn get_status_wrapper<O, W, S>(State(app_state): State<AppState<O, W, S>>) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::api::get_status(State(app_state.websocket_manager)).await
}

async fn start_generation_wrapper<O, W, S>(
    State(app_state): State<AppState<O, W, S>>,
    Json(request): Json<crate::web::handlers::api::StartRequest>,
) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::api::start_generation(State(app_state.orchestrator_client), Json(request)).await
}

async fn stop_generation_wrapper<O, W, S>(State(app_state): State<AppState<O, W, S>>) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::api::stop_generation(State(app_state.orchestrator_client)).await
}

async fn serve_static_wrapper<O, W, S>(
    Path(path): Path<String>,
    State(app_state): State<AppState<O, W, S>>,
) -> Result<Response, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::static_files::serve_static(Path(path), State(app_state.static_server)).await
}

async fn serve_index_wrapper<O, W, S>(State(app_state): State<AppState<O, W, S>>) -> Result<Html<String>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
    W: WebSocketManager + Send + Sync + 'static,
    S: StaticFileServer + Send + Sync + 'static,
{
    crate::web::handlers::static_files::serve_index(State(app_state.static_server)).await
}
