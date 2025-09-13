# Web Server Component Design

The web server provides the user interface for the LLM orchestration system, handling browser connections, serving static content, and facilitating real-time communication between users and the orchestrator.

## Core Responsibilities

1. **Static Content Serving**: Serve HTML, CSS, and JavaScript files for the web interface
2. **WebSocket Management**: Handle bidirectional communication with browsers
3. **Orchestrator Communication**: Maintain WebSocket connection with orchestrator for system updates
4. **Real-time Updates**: Push live metrics and attribute updates to connected browsers
5. **User Interface**: Provide dashboard for system monitoring and control
6. **Request Routing**: Route user requests to appropriate handlers

## Data Structures

### Core State Management
```rust
pub struct WebServerState {
    // Server configuration
    bind_address: SocketAddr,
    static_file_cache: HashMap<String, CachedFile>,
    
    // Browser connection management
    client_connections: Arc<RwLock<HashMap<ClientId, ClientConnection>>>,
    client_broadcast: broadcast::Sender<BrowserMessage>,
    
    // Orchestrator communication
    orchestrator_websocket: Arc<RwLock<Option<WebSocketStream<TcpStream>>>>,
    orchestrator_address: SocketAddr,
    orchestrator_reconnect_interval: Duration,
    
    // System state mirror (read-only view from orchestrator)
    current_metrics: Arc<RwLock<SystemMetrics>>,
    recent_attributes: Arc<RwLock<VecDeque<AttributeUpdate>>>,
    system_health: Arc<RwLock<SystemHealth>>,
    
    // Server state
    is_running: Arc<AtomicBool>,
    connection_count: Arc<AtomicU32>,
    last_orchestrator_update: Arc<RwLock<Instant>>,
}
```

### Message Types
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BrowserMessage {
    // Browser → Web Server
    StartTopic {
        topic: String,
        producer_count: u32,
    },
    StopGeneration,
    RequestDashboard,
    
    // Web Server → Browser
    MetricsUpdate(SystemMetrics),
    AttributeBatch(Vec<AttributeUpdate>),
    SystemAlert {
        level: AlertLevel,
        message: String,
        timestamp: u64,
    },
    DashboardData {
        metrics: SystemMetrics,
        recent_attributes: Vec<AttributeUpdate>,
        system_health: SystemHealth,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WebSocketMessage {
    TopicRequest {
        topic: String,
        producer_count: u32,
        prompt: Option<String>,
    },
    // ... other message types
}

#[derive(Deserialize)]
pub struct StartTopicRequest {
    topic: String,
    producer_count: Option<u32>,
    prompt: Option<String>,
}
```

## Core HTTP Server Implementation

### Axum-based Web Server
```rust
pub struct WebServer {
    state: Arc<WebServerState>,
    router: Router,
}

impl WebServer {
    pub fn new(orchestrator_addr: SocketAddr) -> Self {
        let state = Arc::new(WebServerState::new(orchestrator_addr));
        
        let router = Router::new()
            // Static file routes
            .route("/", get(serve_index))
            .route("/static/*path", get(serve_static))
            
            // WebSocket routes  
            .route("/ws", get(websocket_handler))
            
            // API routes
            .route("/api/start-topic", post(start_topic_handler))
            .route("/api/stop", post(stop_generation_handler))
            .route("/api/status", get(status_handler))
            
            // Health check
            .route("/health", get(health_check))
            
            .with_state(state.clone());
            
        Self { state, router }
    }
    
    pub async fn run(&self, bind_addr: SocketAddr) -> Result<(), WebServerError> {
        // Start orchestrator connection manager
        let orchestrator_task = tokio::spawn({
            let state = self.state.clone();
            async move {
                state.manage_orchestrator_connection().await;
            }
        });
        
        // Start the HTTP server
        let listener = tokio::net::TcpListener::bind(bind_addr).await?;
        println!("Web server listening on {}", bind_addr);
        
        let server_task = tokio::spawn({
            let router = self.router.clone();
            async move {
                axum::serve(listener, router).await.unwrap();
            }
        });
        
        // Wait for either task to complete
        tokio::select! {
            _ = orchestrator_task => {},
            _ = server_task => {},
        }
        
        Ok(())
    }
}
```

## WebSocket Management

### Browser Connection Handling
```rust
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebServerState>>
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

async fn handle_websocket(socket: WebSocket, state: Arc<WebServerState>) {
    let client_id = ClientId::new();
    let (ws_tx, mut ws_rx) = socket.split();
    let (client_tx, mut client_rx) = mpsc::channel::<Message>(100);
    
    // Add client to connection map
    let client_connection = ClientConnection {
        id: client_id.clone(),
        websocket_tx: client_tx.clone(),
        connected_at: Instant::now(),
        last_ping: Instant::now(),
        user_agent: None,
        subscriptions: HashSet::from([SubscriptionType::All]),
    };
    
    state.client_connections.write().await.insert(client_id.clone(), client_connection);
    state.connection_count.fetch_add(1, Ordering::Relaxed);
    
    // Send initial dashboard data
    if let Ok(dashboard_data) = state.get_dashboard_data().await {
        let message = BrowserMessage::DashboardData {
            metrics: dashboard_data.metrics,
            recent_attributes: dashboard_data.recent_attributes,
            system_health: dashboard_data.system_health,
        };
        
        if let Ok(json_msg) = serde_json::to_string(&message) {
            let _ = client_tx.send(Message::Text(json_msg)).await;
        }
    }
    
    // Handle message loop
    tokio::select! {
        _ = handle_outgoing_messages(&mut ws_tx, &mut client_rx) => {},
        _ = handle_incoming_messages(&mut ws_rx, &state, &client_id) => {},
    }
    
    // Cleanup
    state.client_connections.write().await.remove(&client_id);
    state.connection_count.fetch_sub(1, Ordering::Relaxed);
}
```

## Orchestrator Communication

### Connection Management
```rust
impl WebServerState {
    pub async fn manage_orchestrator_connection(&self) {
        while self.is_running.load(Ordering::Relaxed) {
            match self.connect_to_orchestrator().await {
                Ok(websocket) => {
                    println!("Connected to orchestrator");
                    *self.orchestrator_websocket.write().await = Some(websocket);
                    
                    if let Err(e) = self.handle_orchestrator_connection().await {
                        eprintln!("Orchestrator connection error: {}", e);
                    }
                    
                    *self.orchestrator_websocket.write().await = None;
                }
                Err(e) => {
                    eprintln!("Failed to connect to orchestrator: {}", e);
                }
            }
            
            tokio::time::sleep(self.orchestrator_reconnect_interval).await;
        }
    }
    
    async fn handle_orchestrator_message(&self, message: WebSocketMessage) {
        match message {
            WebSocketMessage::SystemMetrics(metrics) => {
                *self.current_metrics.write().await = metrics.clone();
                *self.last_orchestrator_update.write().await = Instant::now();
                self.broadcast_metrics_update(metrics).await;
            }
            
            WebSocketMessage::NewAttributes(attributes) => {
                self.broadcast_new_attributes(attributes).await;
            }
            
            WebSocketMessage::ErrorNotification(error_msg) => {
                let alert = BrowserMessage::SystemAlert {
                    level: AlertLevel::Error,
                    message: error_msg,
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                };
                self.broadcast_to_clients(alert).await;
            }
            
            _ => {
                // Handle other message types as needed
            }
        }
    }
}
```

## HTTP API Endpoints

### Topic Management
```rust
async fn start_topic_handler(
    State(state): State<Arc<WebServerState>>,
    Json(request): Json<StartTopicRequest>
) -> Result<Json<serde_json::Value>, StatusCode> {
    let producer_count = request.producer_count.unwrap_or(5);
    
    let orchestrator_message = WebSocketMessage::TopicRequest {
        topic: request.topic.clone(),
        producer_count,
        prompt: request.prompt,
    };
    
    if let Err(_) = state.send_to_orchestrator(orchestrator_message).await {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    let browser_message = BrowserMessage::SystemAlert {
        level: AlertLevel::Success,
        message: format!("Started generation for topic '{}'", request.topic),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    };
    
    state.broadcast_to_clients(browser_message).await;
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Topic generation started"
    })))
}
```

## Testing Strategy

### Unit Tests
- WebSocket message serialization/deserialization  
- HTTP endpoint functionality
- Static file serving
- Connection management

### Integration Tests
- End-to-end browser communication
- WebSocket connection handling with multiple clients
- Orchestrator disconnection/reconnection handling

This web server design provides a robust, real-time interface for the LLM orchestration system while maintaining clean separation of concerns.