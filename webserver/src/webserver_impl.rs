//! Main webserver implementation
//!
//! This module contains the main WebServer struct that orchestrates all services
//! using dependency injection, similar to the orchestrator pattern.

use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    routing::{get, post},
    Router, 
    extract::{State, WebSocketUpgrade, ws::WebSocket},
    response::{IntoResponse, Json},
    http::StatusCode,
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use serde_json::json;
use shared::SystemMetrics;
use crate::error::{WebServerError, WebServerResult};
use crate::state::WebServerState;
use crate::traits::{
    FileManager, ClientBroadcaster, IpcCommunicator, 
    ClientRegistry, MetricsAggregator
};
use crate::types::{BrowserMessage, ClientId};

/// Main webserver struct with dependency injection
#[derive(Clone)]
pub struct WebServer<S, W, O, C, M>
where
    S: FileManager,
    W: ClientBroadcaster,
    O: IpcCommunicator,
    C: ClientRegistry,
    M: MetricsAggregator,
{
    state: Arc<WebServerState>,
    file_manager: S,
    client_broadcaster: W,
    ipc_communicator: O,
    client_registry: C,
    metrics_aggregator: M,
}

impl<S, W, O, C, M> WebServer<S, W, O, C, M>
where
    S: FileManager + Clone + Send + Sync + 'static,
    W: ClientBroadcaster + Clone + Send + Sync + 'static,
    O: IpcCommunicator + Clone + Send + Sync + 'static,
    C: ClientRegistry + Clone + Send + Sync + 'static,
    M: MetricsAggregator + Clone + Send + Sync + 'static,
{
    /// Create a new webserver with dependency injection
    pub fn new(
        bind_address: SocketAddr,
        orchestrator_address: SocketAddr,
        file_manager: S,
        client_broadcaster: W,
        ipc_communicator: O,
        client_registry: C,
        metrics_aggregator: M,
    ) -> Self {
        let state = Arc::new(WebServerState::new(bind_address, orchestrator_address));
        
        Self {
            state,
            file_manager,
            client_broadcaster,
            ipc_communicator,
            client_registry,
            metrics_aggregator,
        }
    }

    /// Build the Axum router with all routes
    pub fn build_router(&self) -> Router {
        Router::new()
            // Static file routes
            .route("/", get(serve_index))
            .route("/static/*path", get(serve_static))
            
            // WebSocket route  
            .route("/ws", get(websocket_handler))
            
            // API routes
            .route("/api/start-topic", post(start_topic_handler))
            .route("/api/stop", post(stop_generation_handler))
            .route("/api/status", get(status_handler))
            .route("/api/metrics", get(metrics_handler))
            
            // Health check
            .route("/health", get(health_check))
            
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive()) // Allow CORS for development
                    .into_inner(),
            )
            .with_state(self.clone())
    }

    /// Start the webserver
    pub async fn run(&self) -> WebServerResult<()> {
        let router = self.build_router();
        
        // Start orchestrator connection manager
        let orchestrator_task = {
            let client = self.ipc_communicator.clone();
            let _state = self.state.clone();
            let orchestrator_addr = self.state.orchestrator_address;
            tokio::spawn(async move {
                if let Err(e) = client.connect(orchestrator_addr).await {
                    eprintln!("Failed to connect to orchestrator: {}", e);
                }
                if let Err(e) = client.start_connection_loop().await {
                    eprintln!("Orchestrator connection loop failed: {}", e);
                }
            })
        };

        // Start the HTTP server
        let listener = tokio::net::TcpListener::bind(self.state.bind_address).await
            .map_err(|e| WebServerError::ServerStartup(format!("Failed to bind to {}: {}", self.state.bind_address, e)))?;
        
        println!("🌐 Web server listening on http://{}", self.state.bind_address);
        println!("📊 Dashboard available at http://{}/", self.state.bind_address);
        
        let server_task = tokio::spawn({
            async move {
                if let Err(e) = axum::serve(listener, router).await {
                    eprintln!("Server error: {}", e);
                }
            }
        });

        // Wait for either task to complete or for shutdown signal
        tokio::select! {
            _ = orchestrator_task => {
                println!("Orchestrator connection task completed");
            },
            _ = server_task => {
                println!("HTTP server task completed");
            },
            _ = tokio::signal::ctrl_c() => {
                println!("Received shutdown signal");
                self.state.set_running(false);
            }
        }

        Ok(())
    }

    /// Get server state for external access
    pub fn state(&self) -> &Arc<WebServerState> {
        &self.state
    }
}


// HTTP Handlers

/// Serve the main index page
async fn serve_index<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> impl IntoResponse
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    match webserver.file_manager.serve_file("index.html").await {
        Ok(response) => response,
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load index page").into_response()
        }
    }
}

/// Serve static files
async fn serve_static<S, W, O, C, M>(
    axum::extract::Path(path): axum::extract::Path<String>,
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> impl IntoResponse
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    match webserver.file_manager.serve_file(&path).await {
        Ok(response) => response,
        Err(_) => {
            (StatusCode::NOT_FOUND, "File not found").into_response()
        }
    }
}

/// Handle WebSocket connections
async fn websocket_handler<S, W, O, C, M>(
    ws: WebSocketUpgrade,
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> impl IntoResponse
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    ws.on_upgrade(move |socket| handle_websocket(socket, webserver))
}

/// Handle individual WebSocket connection
async fn handle_websocket<S, W, O, C, M>(
    socket: WebSocket,
    webserver: WebServer<S, W, O, C, M>,
)
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    let client_id = ClientId::new();
    println!("🔗 Setting up WebSocket connection for client: {}", client_id.0);
    
    // Create client connection for registry
    let (tx, _rx) = tokio::sync::mpsc::channel(100);
    let connection = crate::types::ClientConnection {
        id: client_id.clone(),
        websocket_tx: tx,
        connected_at: std::time::Instant::now(),
        last_ping: std::time::Instant::now(),
        user_agent: None,
    };
    
    // Register client with client registry
    if let Err(e) = webserver.client_registry.add_client(connection).await {
        eprintln!("Failed to register client {}: {}", client_id.0, e);
        return;
    }
    
    // Handle the WebSocket connection with the broadcaster
    let result = webserver.client_broadcaster.handle_connection(socket, client_id.clone()).await;
    
    // When connection ends, remove from registry
    if let Err(e) = webserver.client_registry.remove_client(&client_id).await {
        eprintln!("Failed to unregister client {}: {}", client_id.0, e);
    }
    
    if let Err(e) = result {
        eprintln!("WebSocket connection error for {}: {}", client_id.0, e);
    }
    
    println!("🔌 WebSocket connection ended for client: {}", client_id.0);
}

/// Start topic generation
async fn start_topic_handler<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    let topic = request.get("topic")
        .and_then(|t| t.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_string();
        
    let producer_count = request.get("producer_count")
        .and_then(|c| c.as_u64())
        .unwrap_or(5) as u32;
        
    let prompt = request.get("prompt")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());

    let message = BrowserMessage::StartTopic {
        topic: topic.clone(),
        producer_count,
        prompt,
    };

    // Send via IPC to orchestrator
    if let Err(e) = webserver.ipc_communicator.send_message(message.clone()).await {
        eprintln!("Failed to send start topic message to orchestrator: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    // Also broadcast to all connected clients for UI updates
    if let Err(e) = webserver.client_broadcaster.broadcast_to_all(message).await {
        eprintln!("Failed to broadcast start topic message to clients: {}", e);
        // Don't fail the request if client broadcast fails
    }

    println!("✅ Started topic generation: '{}' with {} producers", topic, producer_count);
    
    Ok(Json(json!({
        "status": "success",
        "message": format!("Started generation for topic '{}' with {} producers", topic, producer_count),
        "topic": topic,
        "producer_count": producer_count
    })))
}

/// Stop generation
async fn stop_generation_handler<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    let message = BrowserMessage::StopGeneration;

    // Send via IPC to orchestrator
    if let Err(e) = webserver.ipc_communicator.send_message(message.clone()).await {
        eprintln!("Failed to send stop generation message to orchestrator: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    // Also broadcast to all connected clients
    if let Err(e) = webserver.client_broadcaster.broadcast_to_all(message).await {
        eprintln!("Failed to broadcast stop generation message to clients: {}", e);
        // Don't fail the request if client broadcast fails
    }

    println!("✅ Stopped topic generation");
    
    Ok(Json(json!({
        "status": "success",
        "message": "Generation stopped"
    })))
}

/// Get server status
async fn status_handler<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> Json<serde_json::Value>
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    let system_health = webserver.metrics_aggregator.get_system_health().await
        .unwrap_or_else(|_| crate::types::SystemHealth {
            orchestrator_connected: false,
            active_clients: 0,
            last_update: None,
            server_uptime_seconds: 0,
        });
        
    let active_clients = webserver.client_registry.get_connection_count().await;
    let orchestrator_connected = webserver.ipc_communicator.is_connected().await;

    Json(json!({
        "status": "running",
        "uptime_seconds": webserver.state.get_uptime_seconds(),
        "active_clients": active_clients,
        "orchestrator_connected": orchestrator_connected,
        "last_update": system_health.last_update,
        "server_start_time": webserver.state.server_start_time.elapsed().as_secs()
    }))
}

/// Get current metrics
async fn metrics_handler<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> Result<Json<SystemMetrics>, StatusCode>
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    match webserver.metrics_aggregator.get_current_metrics().await {
        Ok(metrics) => Ok(Json(metrics)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Health check endpoint
async fn health_check<S, W, O, C, M>(
    State(webserver): State<WebServer<S, W, O, C, M>>,
) -> Json<serde_json::Value>
where
    S: FileManager + Clone + 'static,
    W: ClientBroadcaster + Clone + 'static,
    O: IpcCommunicator + Clone + 'static,
    C: ClientRegistry + Clone + 'static,
    M: MetricsAggregator + Clone + 'static,
{
    Json(json!({
        "status": "healthy",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "uptime": webserver.state.get_uptime_seconds(),
        "connections": webserver.state.get_connection_count()
    }))
}