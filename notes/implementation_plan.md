# Implementation Plan - Concurrent LLM Orchestration System

This document provides a phased implementation plan based on our complete system design, coding standards, and architectural decisions.

## Project Structure Visualization

```
igentai/                            # Workspace root
├── Cargo.toml                      # Workspace manifest
├── target/                         # Build artifacts
├── output.txt                      # Generated attributes (runtime)
├── metrics.json                    # System metrics (runtime)
├── state.json                      # System state (runtime)
├── notes/                          # Design documentation
│   ├── design_decisions.md         # Architectural decisions log
│   ├── coding_standards.md         # Development standards
│   ├── system_specification.md     # High-level architecture spec
│   ├── orchestrator_design.md      # Orchestrator component design
│   ├── producer_design.md          # Producer component design
│   ├── webserver_design.md         # Web server component design
│   └── project_structure.md        # Multi-crate organization guide
├── shared/                         # Common types and utilities
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Shared library exports
│       ├── main.rs                 # Common types and traits
│       ├── messages.rs             # Inter-component messages
│       ├── types.rs                # Shared data structures
│       ├── metrics.rs              # Metrics definitions
│       ├── errors.rs               # Common error types
│       └── tests/
│           ├── mod.rs              # Test organization
│           ├── main.rs             # Core shared type tests
│           ├── message_tests.rs    # Message serialization tests
│           └── fixtures.rs         # Test data and mocks
├── orchestrator/                   # Master process crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Orchestrator library exports
│       ├── main.rs                 # CLI entry + OrchestratorMain struct
│       ├── state.rs                # Core orchestrator state management
│       ├── statistics.rs           # Statistics engine implementation
│       ├── optimization.rs         # Performance optimization algorithms
│       ├── process_manager.rs      # Child process lifecycle management
│       ├── file_manager.rs         # File I/O and state persistence
│       └── tests/
│           ├── mod.rs              # Test organization
│           ├── main.rs             # Core orchestrator tests
│           ├── state_tests.rs      # State management tests
│           ├── statistics_tests.rs # Statistics engine tests
│           ├── optimization_tests.rs # Optimization algorithm tests
│           ├── process_management_tests.rs # Process lifecycle tests
│           ├── integration_tests.rs # Cross-component integration
│           ├── fixtures.rs         # Test data and mock objects
│           └── helpers.rs          # Test utility functions
├── producer/                       # Producer process crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Producer library exports
│       ├── main.rs                 # CLI entry + ProducerMain struct
│       ├── state.rs                # Producer state management
│       ├── llm_client.rs           # LLM API integration hub
│       ├── openai.rs               # OpenAI client implementation
│       ├── anthropic.rs            # Anthropic client implementation
│       ├── provider_manager.rs     # Intelligent provider selection
│       ├── prompt_engineer.rs      # Prompt building and optimization
│       ├── response_parser.rs      # LLM response parsing and filtering
│       └── tests/
│           ├── mod.rs              # Test organization
│           ├── main.rs             # Core producer tests
│           ├── state_tests.rs      # State management tests
│           ├── provider_selection_tests.rs # Provider selection tests
│           ├── openai_client_tests.rs # OpenAI integration tests
│           ├── anthropic_client_tests.rs # Anthropic integration tests
│           ├── prompt_engineering_tests.rs # Prompt generation tests
│           ├── response_parsing_tests.rs # Response parsing tests
│           ├── communication_tests.rs # TCP communication tests
│           ├── integration_tests.rs # End-to-end producer tests
│           ├── fixtures.rs         # Mock data and responses
│           └── helpers.rs          # Test utility functions
└── webserver/                      # Web interface crate
    ├── Cargo.toml
    ├── static/                     # Frontend assets
    │   ├── index.html              # Dashboard HTML
    │   ├── app.css                 # Dashboard styles
    │   └── app.js                  # Dashboard JavaScript
    └── src/
        ├── lib.rs                  # Web server library exports
        ├── main.rs                 # CLI entry + WebServerMain struct
        ├── server.rs               # Core HTTP server implementation
        ├── handlers.rs             # HTTP endpoint handlers
        ├── websocket.rs            # WebSocket connection management
        ├── static_files.rs         # Static asset serving
        └── tests/
            ├── mod.rs              # Test organization
            ├── main.rs             # Core web server tests
            ├── server_tests.rs     # HTTP server tests
            ├── handler_tests.rs    # Endpoint handler tests
            ├── websocket_tests.rs  # WebSocket communication tests
            ├── static_serving_tests.rs # Static file serving tests
            ├── integration_tests.rs # Browser simulation tests
            ├── fixtures.rs         # Mock HTTP clients and data
            └── helpers.rs          # Test utility functions
```

## Implementation Phases

### Phase 0: Foundation Setup (Day 1, Hours 0-1)
**Goal**: Establish workspace structure and shared types

**Tasks**:
1. Set up Rust workspace with 4 crates
2. Configure Cargo.toml files with dependencies
3. Implement shared types and message protocols
4. Basic error handling and common utilities
5. Initial test framework setup

**Dependencies**: None
**Output**: Compilable workspace with shared types

### Phase 1: Core Orchestrator (Day 1, Hours 1-3)
**Goal**: Master process with basic producer management

**Tasks**:
1. Command-line interface with clap (`-n` and `-p` parameters)
2. Basic OrchestratorState with HashSet uniqueness checking
3. Producer process spawning and TCP connection handling
4. Message serialization/deserialization with bincode
5. File I/O for output.txt and basic state persistence
6. Web server child process management

**Dependencies**: Shared crate
**Output**: Working orchestrator that can spawn and communicate with child processes

### Phase 2: Basic Producer (Day 1, Hours 2-4)
**Goal**: Autonomous producer with LLM API integration

**Tasks**:
1. ProducerState with direct LLM client methods
2. OpenAI API integration with error handling
3. Anthropic API integration with error handling
4. Basic generation loop and TCP communication with orchestrator
5. Response parsing and candidate extraction
6. Provider health monitoring

**Dependencies**: Shared crate, API keys
**Output**: Working producer that generates and sends attributes

### Phase 3: Basic Web Server (Day 1, Hours 3-5)
**Goal**: Web interface with static serving and WebSocket

**Tasks**:
1. Axum HTTP server with static file serving
2. WebSocket connection handling for browser clients
3. Basic API endpoints for start/stop operations
4. Dashboard HTML/CSS/JS integration
5. Real-time metrics display framework

**Dependencies**: Shared crate, static files
**Output**: Functional web dashboard with real-time updates

### Phase 4: System Integration (Day 1, Hours 4-6)
**Goal**: End-to-end working system

**Tasks**:
1. Complete message flow implementation
2. Statistics engine in orchestrator
3. Real-time metrics calculation and broadcasting
4. Error handling and recovery mechanisms
5. File persistence and state recovery
6. System testing and debugging

**Dependencies**: All components
**Output**: Fully functional LLM orchestration system

### Phase 5: Optimization Features (Day 1, Hours 6-8)
**Goal**: Dynamic routing and performance optimization

**Tasks**:
1. Per-producer weight optimization
2. Provider performance analysis algorithms
3. Prompt partitioning strategies
4. Dynamic configuration updates
5. Advanced statistics and trend analysis
6. Performance monitoring dashboard enhancements

**Dependencies**: Working base system
**Output**: System with intelligent optimization and bonus features

## Comprehensive Testing Specifications

### Shared Crate Tests (`shared/tests/`)

#### **Core Type Tests** (`main.rs`)
```rust
// Message serialization round-trip tests
#[test]
fn test_producer_message_serialization() {
    let original = ProducerMessage::StartTopic {
        topic: "test topic".to_string(),
        prompt: "test prompt".to_string(),
        provider_weights: HashMap::from([("openai".to_string(), 0.6)]),
        generation_config: GenerationConfig::default(),
    };
    
    let serialized = bincode::serialize(&original).unwrap();
    let deserialized: ProducerMessage = bincode::deserialize(&serialized).unwrap();
    
    assert_eq!(original, deserialized);
}

#[test]
fn test_websocket_message_json_serialization() {
    let metrics = SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 15.5,
        per_llm_performance: HashMap::new(),
        current_topic: Some("test".to_string()),
        active_producers: 5,
        uptime_seconds: 300,
        last_updated: 1234567890,
    };
    
    let original = WebSocketMessage::SystemMetrics(metrics);
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: WebSocketMessage = serde_json::from_str(&json).unwrap();
    
    assert_eq!(original, deserialized);
}
```

#### **Error Type Tests** (`error_tests.rs`)
```rust
#[test]
fn test_error_chain_propagation() {
    let orchestrator_error = OrchestratorError::ProducerSpawnFailed("test_id".to_string());
    let error_string = format!("{}", orchestrator_error);
    assert!(error_string.contains("test_id"));
}

#[test]
fn test_producer_error_conversion() {
    let api_error = ProducerError::ApiError("OpenAI rate limit".to_string());
    let converted: Box<dyn std::error::Error> = Box::new(api_error);
    assert!(converted.to_string().contains("rate limit"));
}
```

### Orchestrator Tests (`orchestrator/tests/`)

#### **Core Orchestrator Tests** (`main.rs`)
```rust
use super::fixtures::*;
use crate::orchestrator::{OrchestratorState, OrchestratorConfig};

#[tokio::test]
async fn test_orchestrator_initialization() {
    let config = create_test_config();
    let orchestrator = OrchestratorState::new(config).await.unwrap();
    
    assert_eq!(orchestrator.producer_count, 3);
    assert_eq!(orchestrator.webserver_port, 8080);
    assert!(orchestrator.unique_attributes.is_empty());
}

#[tokio::test]
async fn test_start_stop_producers() {
    let mut orchestrator = create_test_orchestrator().await;
    
    // Test starting producers
    orchestrator.start_producers(3, "test topic".to_string()).await.unwrap();
    assert_eq!(orchestrator.producer_processes.len(), 3);
    assert_eq!(orchestrator.current_topic, Some("test topic".to_string()));
    
    // Test stopping producers
    orchestrator.stop_producers().await.unwrap();
    assert!(orchestrator.producer_processes.is_empty());
}

#[tokio::test]
async fn test_uniqueness_checking() {
    let mut orchestrator = create_test_orchestrator().await;
    let producer_id = ProducerId::new();
    
    // First batch - all should be unique
    let candidates1 = vec!["item1".to_string(), "item2".to_string(), "item3".to_string()];
    let unique1 = orchestrator.process_attribute_batch(producer_id.clone(), candidates1, mock_generation_metadata()).await.unwrap();
    assert_eq!(unique1.len(), 3);
    assert_eq!(orchestrator.total_count, 3);
    
    // Second batch with duplicates
    let candidates2 = vec!["item2".to_string(), "item4".to_string(), "item1".to_string()];
    let unique2 = orchestrator.process_attribute_batch(producer_id, candidates2, mock_generation_metadata()).await.unwrap();
    assert_eq!(unique2.len(), 1); // Only "item4" should be unique
    assert_eq!(unique2[0], "item4");
    assert_eq!(orchestrator.total_count, 4);
}
```

#### **Statistics Engine Tests** (`statistics_tests.rs`)
```rust
#[test]
fn test_statistics_calculation() {
    let mut stats_engine = StatisticsEngine::new();
    let events = vec![
        create_producer_request_stat("producer1", "openai", 5, 3, 1000),
        create_producer_request_stat("producer2", "anthropic", 4, 4, 800),
    ];
    
    stats_engine.update_statistics(events).unwrap();
    
    let metrics = stats_engine.get_current_metrics();
    assert_eq!(metrics.total_unique_entries, 7);
    assert!(metrics.entries_per_minute > 0.0);
    assert_eq!(metrics.per_llm_performance.len(), 2);
}

#[test]
fn test_performance_trend_analysis() {
    let mut stats_engine = StatisticsEngine::new();
    
    // Add historical data points
    for i in 0..10 {
        let events = vec![create_producer_request_stat("producer1", "openai", 5, 3, 1000 + i * 100)];
        stats_engine.update_statistics(events).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    let trends = stats_engine.analyze_performance_trends().unwrap();
    assert!(trends.provider_success_trends.contains_key("openai"));
    assert!(trends.overall_generation_rate_trend.current_value > 0.0);
}
```

#### **Optimization Algorithm Tests** (`optimization_tests.rs`)
```rust
#[tokio::test]
async fn test_provider_weight_optimization() {
    let mut orchestrator = create_test_orchestrator().await;
    
    // Set up provider performance data
    orchestrator.add_mock_performance_data("openai", 0.8, 1200.0); // Good performance
    orchestrator.add_mock_performance_data("anthropic", 0.4, 3000.0); // Poor performance
    
    let optimized_weights = orchestrator.calculate_optimal_provider_weights().await.unwrap();
    
    // OpenAI should get higher weight due to better performance
    assert!(optimized_weights.get("openai").unwrap() > optimized_weights.get("anthropic").unwrap());
}

#[test]
fn test_prompt_partitioning_strategy() {
    let orchestrator = create_test_orchestrator().await;
    let strategy = orchestrator.determine_partition_strategy("Paris attractions");
    
    match strategy {
        PartitionStrategy::CategoryBased(categories) => {
            assert!(categories.contains(&"museums".to_string()));
            assert!(categories.contains(&"restaurants".to_string()));
        }
        _ => panic!("Expected category-based partitioning for Paris attractions"),
    }
    
    let specialized_prompt = orchestrator.generate_partition_specific_prompt(
        "Paris attractions", 
        0, 
        &strategy
    );
    assert!(specialized_prompt.contains("museums"));
}
```

### Producer Tests (`producer/tests/`)

#### **Core Producer Tests** (`main.rs`)
```rust
use super::fixtures::*;
use crate::producer::{ProducerState, ProducerConfig};

#[tokio::test]
async fn test_producer_initialization() {
    let config = create_test_producer_config();
    let producer = ProducerState::new(ProducerId::new(), test_socket_addr(), config).unwrap();
    
    assert!(producer.is_provider_configured("mock"));
    assert!(!producer.is_provider_configured("nonexistent"));
}

#[tokio::test]
async fn test_generation_cycle() {
    let mut producer = create_mock_producer().await;
    
    // Set up for generation
    producer.current_topic = Some("test topic".to_string());
    producer.prompt = "Generate items about {topic}".to_string();
    producer.provider_weights.insert("mock".to_string(), 1.0);
    
    // Run one generation cycle
    producer.generation_cycle().await.unwrap();
    
    // Verify generation attempt was recorded
    assert_eq!(producer.generation_history.len(), 1);
    assert_eq!(producer.generation_history[0].provider_used, "mock");
}

#[tokio::test]
async fn test_message_handling() {
    let mut producer = create_mock_producer().await;
    
    // Test StartTopic message
    let start_message = ProducerMessage::StartTopic {
        topic: "test topic".to_string(),
        prompt: "test prompt".to_string(),
        provider_weights: HashMap::from([("openai".to_string(), 0.7), ("anthropic".to_string(), 0.3)]),
        generation_config: GenerationConfig::default(),
    };
    
    producer.handle_orchestrator_message(start_message).await.unwrap();
    
    assert_eq!(producer.current_topic, Some("test topic".to_string()));
    assert_eq!(producer.prompt, "test prompt");
    assert_eq!(producer.provider_weights.len(), 2);
    assert!(producer.is_running.load(Ordering::Relaxed));
}
```

#### **Provider Selection Tests** (`provider_selection_tests.rs`)
```rust
#[test]
fn test_weighted_provider_selection() {
    let mut producer = create_mock_producer().await;
    producer.provider_weights = HashMap::from([
        ("openai".to_string(), 0.7),
        ("anthropic".to_string(), 0.3),
        ("mock".to_string(), 0.0), // Should never be selected
    ]);
    
    // Run selection multiple times to test distribution
    let mut selections = HashMap::new();
    for _ in 0..1000 {
        let provider = producer.select_provider_intelligently().await.unwrap();
        *selections.entry(provider).or_insert(0) += 1;
    }
    
    // Verify distribution approximately matches weights
    let openai_ratio = selections.get("openai").unwrap_or(&0) as f32 / 1000.0;
    let anthropic_ratio = selections.get("anthropic").unwrap_or(&0) as f32 / 1000.0;
    
    assert!((openai_ratio - 0.7).abs() < 0.1); // Within 10%
    assert!((anthropic_ratio - 0.3).abs() < 0.1);
    assert_eq!(selections.get("mock"), None); // Should never be selected
}

#[test]
fn test_unhealthy_provider_exclusion() {
    let mut producer = create_mock_producer().await;
    
    // Mark OpenAI as unhealthy
    producer.provider_health.insert("openai".to_string(), ProviderHealth {
        consecutive_failures: 5,
        current_status: ProviderStatus::Unhealthy,
        last_success: None,
        last_failure: Some(Instant::now()),
        average_response_time: Duration::from_millis(5000),
    });
    
    producer.provider_weights = HashMap::from([
        ("openai".to_string(), 0.8),
        ("mock".to_string(), 0.2),
    ]);
    
    // Should always select mock since OpenAI is unhealthy
    for _ in 0..10 {
        let selected = producer.select_provider_intelligently().await.unwrap();
        assert_eq!(selected, "mock");
    }
}
```

#### **LLM Client Tests** (`openai_client_tests.rs`, `anthropic_client_tests.rs`)
```rust
// OpenAI Client Tests
#[tokio::test]
async fn test_openai_successful_generation() {
    let producer = create_producer_with_mock_openai().await;
    
    // Mock successful response
    let response = producer.generate_openai("test prompt", &GenerationConfig::default()).await.unwrap();
    
    assert!(!response.content.is_empty());
    assert!(response.tokens_used > 0);
    assert!(response.response_time > Duration::ZERO);
}

#[tokio::test]
async fn test_openai_api_error_handling() {
    let producer = create_producer_with_failing_openai().await;
    
    let result = producer.generate_openai("test prompt", &GenerationConfig::default()).await;
    
    match result {
        Err(ProducerError::ApiError(msg)) => assert!(msg.contains("OpenAI")),
        _ => panic!("Expected OpenAI API error"),
    }
}

#[tokio::test]
async fn test_openai_health_check() {
    let producer = create_producer_with_mock_openai().await;
    
    let health_result = producer.health_check_openai().await;
    assert!(health_result.is_ok());
}
```

#### **Response Parsing Tests** (`response_parsing_tests.rs`)
```rust
#[test]
fn test_llm_response_parsing() {
    let producer = create_mock_producer().await;
    
    let raw_response = "1. First item\n2. Second item\n- Third item\n\nFourth item\n\n";
    let parsed = producer.parse_llm_response(raw_response).unwrap();
    
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed[0], "First item");
    assert_eq!(parsed[1], "Second item");
    assert_eq!(parsed[2], "Third item");
    assert_eq!(parsed[3], "Fourth item");
}

#[test]
fn test_parsing_with_various_formats() {
    let producer = create_mock_producer().await;
    
    let test_cases = vec![
        ("• Item A\n• Item B", vec!["Item A", "Item B"]),
        ("- Item X\n  - Item Y\n", vec!["Item X", "Item Y"]),
        ("1) First\n2) Second\n", vec!["First", "Second"]),
        ("Plain item\nAnother item\n", vec!["Plain item", "Another item"]),
    ];
    
    for (input, expected) in test_cases {
        let parsed = producer.parse_llm_response(input).unwrap();
        assert_eq!(parsed, expected);
    }
}
```

### Web Server Tests (`webserver/tests/`)

#### **Core Web Server Tests** (`main.rs`)
```rust
use super::fixtures::*;
use axum_test::TestServer;

#[tokio::test]
async fn test_web_server_initialization() {
    let webserver = create_test_webserver().await;
    assert!(webserver.static_file_cache.contains_key("index.html"));
    assert!(webserver.static_file_cache.contains_key("app.css"));
    assert!(webserver.static_file_cache.contains_key("app.js"));
}

#[tokio::test]
async fn test_static_file_serving() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();
    
    // Test index page
    let response = server.get("/").await;
    response.assert_status_ok();
    response.assert_text_contains("LLM Orchestration System");
    
    // Test CSS file
    let css_response = server.get("/static/app.css").await;
    css_response.assert_status_ok();
    css_response.assert_header("content-type", "text/css");
    
    // Test JS file
    let js_response = server.get("/static/app.js").await;
    js_response.assert_status_ok();
    js_response.assert_header("content-type", "application/javascript");
}
```

#### **API Endpoint Tests** (`handler_tests.rs`)
```rust
#[tokio::test]
async fn test_start_topic_endpoint() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();
    
    let request_body = serde_json::json!({
        "topic": "test topic",
        "producer_count": 3,
        "prompt": "custom prompt"
    });
    
    let response = server.post("/api/start-topic")
        .json(&request_body)
        .await;
    
    response.assert_status_ok();
    
    let response_json: serde_json::Value = response.json();
    assert_eq!(response_json["status"], "success");
}

#[tokio::test]
async fn test_stop_generation_endpoint() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();
    
    let response = server.post("/api/stop").await;
    
    response.assert_status_ok();
    
    let response_json: serde_json::Value = response.json();
    assert_eq!(response_json["status"], "success");
    assert!(response_json["message"].as_str().unwrap().contains("stopped"));
}

#[tokio::test]
async fn test_status_endpoint() {
    let app = create_test_app().await;
    let server = TestServer::new(app).unwrap();
    
    let response = server.get("/api/status").await;
    response.assert_status_ok();
    
    let status_json: serde_json::Value = response.json();
    assert!(status_json["web_server"].is_object());
    assert!(status_json["orchestrator"].is_object());
}
```

#### **WebSocket Tests** (`websocket_tests.rs`)
```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::test]
async fn test_websocket_connection() {
    let webserver = start_test_webserver().await;
    let (ws_stream, _) = connect_async("ws://localhost:8080/ws").await.unwrap();
    
    // Connection should be established
    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    
    // Send test message
    let test_message = BrowserMessage::RequestDashboard;
    let json_msg = serde_json::to_string(&test_message).unwrap();
    ws_tx.send(Message::Text(json_msg)).await.unwrap();
    
    // Should receive dashboard data response
    if let Some(Ok(Message::Text(response))) = ws_rx.next().await {
        let parsed: BrowserMessage = serde_json::from_str(&response).unwrap();
        match parsed {
            BrowserMessage::DashboardData { .. } => {}, // Expected
            _ => panic!("Expected DashboardData response"),
        }
    }
}

#[tokio::test]
async fn test_multiple_client_broadcasting() {
    let webserver = start_test_webserver().await;
    
    // Connect multiple clients
    let clients = join_all((0..3).map(|_| async {
        connect_async("ws://localhost:8080/ws").await.unwrap()
    })).await;
    
    // Broadcast a metrics update
    let metrics = create_test_metrics();
    webserver.broadcast_metrics_update(metrics.clone()).await;
    
    // All clients should receive the update
    for (mut ws_stream, _) in clients {
        if let Some(Ok(Message::Text(msg))) = ws_stream.next().await {
            let parsed: BrowserMessage = serde_json::from_str(&msg).unwrap();
            match parsed {
                BrowserMessage::MetricsUpdate(received_metrics) => {
                    assert_eq!(received_metrics.total_unique_entries, metrics.total_unique_entries);
                }
                _ => panic!("Expected metrics update"),
            }
        }
    }
}
```

### Integration Tests (`tests/` in workspace root)

#### **End-to-End System Tests** (`end_to_end.rs`)
```rust
#[tokio::test]
async fn test_complete_topic_generation_workflow() {
    // Start orchestrator with mock configuration
    let mut orchestrator = OrchestratorState::new(create_integration_test_config()).await.unwrap();
    
    // Start web server
    let webserver = orchestrator.start_webserver().await.unwrap();
    
    // Start producers
    orchestrator.start_producers(2, "test topic".to_string()).await.unwrap();
    
    // Simulate producer generations
    let producer_id = ProducerId::new();
    let candidates = vec!["unique1".to_string(), "unique2".to_string()];
    let generation_metadata = create_mock_generation_metadata();
    
    let unique_items = orchestrator.process_attribute_batch(
        producer_id, 
        candidates, 
        generation_metadata
    ).await.unwrap();
    
    // Verify uniqueness processing
    assert_eq!(unique_items.len(), 2);
    assert_eq!(orchestrator.total_count, 2);
    
    // Verify file output
    let output_content = tokio::fs::read_to_string("output.txt").await.unwrap();
    assert!(output_content.contains("unique1"));
    assert!(output_content.contains("unique2"));
    
    // Stop system
    orchestrator.stop_producers().await.unwrap();
}

#[tokio::test]
async fn test_producer_communication_flow() {
    let orchestrator_addr = "127.0.0.1:8083".parse().unwrap();
    let orchestrator = start_test_orchestrator(orchestrator_addr).await;
    
    // Start a real producer process
    let producer_handle = tokio::spawn(async move {
        let config = ProducerConfig {
            openai_api_key: None,
            anthropic_api_key: None,
            openai_model: Some("mock".to_string()),
            anthropic_model: Some("mock".to_string()),
        };
        let producer = ProducerState::new_mock(ProducerId::new(), orchestrator_addr);
        producer.run_generation_loop().await
    });
    
    // Send start topic message
    let start_message = ProducerMessage::StartTopic {
        topic: "integration test".to_string(),
        prompt: "Generate test items".to_string(),
        provider_weights: HashMap::from([("mock".to_string(), 1.0)]),
        generation_config: GenerationConfig::default(),
    };
    
    // Verify communication works
    tokio::time::timeout(Duration::from_secs(5), async {
        // Producer should receive message and start generating
        // Orchestrator should receive attribute batches
        // System should maintain state correctly
    }).await.unwrap();
}

#[tokio::test]
async fn test_system_recovery_from_files() {
    // Create initial state files
    tokio::fs::write("output.txt", "item1\nitem2\nitem3\n").await.unwrap();
    
    let state_data = serde_json::json!({
        "current_topic": "recovered topic",
        "total_unique_count": 3,
        "active_producer_count": 0,
        "provider_weights": {"openai": 0.6, "anthropic": 0.4},
        "last_updated": 1234567890
    });
    tokio::fs::write("state.json", serde_json::to_string_pretty(&state_data).unwrap()).await.unwrap();
    
    // Start orchestrator - should recover state
    let mut orchestrator = OrchestratorState::new(create_test_config()).await.unwrap();
    orchestrator.load_state_from_files().await.unwrap();
    
    // Verify recovery
    assert_eq!(orchestrator.unique_attributes.len(), 3);
    assert_eq!(orchestrator.total_count, 3);
    assert!(orchestrator.unique_attributes.contains("item1"));
    assert!(orchestrator.unique_attributes.contains("item2"));
    assert!(orchestrator.unique_attributes.contains("item3"));
}
```

## Test Fixtures and Helpers

### Common Test Fixtures (`fixtures.rs` in each crate)

```rust
// Orchestrator fixtures
pub fn create_test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        producer_count: 3,
        webserver_port: 8080,
        optimization_interval: Duration::from_secs(10),
        file_save_interval: Duration::from_secs(5),
        tcp_port_range: 8083..8093,
    }
}

pub async fn create_test_orchestrator() -> OrchestratorState {
    OrchestratorState::new(create_test_config()).await.unwrap()
}

pub fn mock_generation_metadata() -> GenerationMetadata {
    GenerationMetadata {
        provider_used: "mock".to_string(),
        prompt_hash: 12345,
        candidates_generated: 5,
        response_time_ms: 1000,
        tokens_used: 50,
        prompt_tokens: 25,
        completion_tokens: 25,
        success: true,
        partition_coherence: Some(0.95),
    }
}

// Producer fixtures  
pub fn create_test_producer_config() -> ProducerConfig {
    ProducerConfig {
        openai_api_key: Some("test_key".to_string()),
        anthropic_api_key: Some("test_key".to_string()),
        openai_model: Some("gpt-4o-mini".to_string()),
        anthropic_model: Some("claude-3-5-sonnet".to_string()),
    }
}

pub async fn create_mock_producer() -> ProducerState {
    ProducerState::new_mock(ProducerId::new(), "127.0.0.1:8083".parse().unwrap())
}

// Web server fixtures
pub async fn create_test_webserver() -> WebServerState {
    WebServerState::new("127.0.0.1:8082".parse().unwrap()).await.unwrap()
}

pub fn create_test_metrics() -> SystemMetrics {
    SystemMetrics {
        total_unique_entries: 100,
        entries_per_minute: 15.5,
        per_llm_performance: HashMap::from([
            ("openai".to_string(), LLMPerformance {
                unique_generated: 60,
                success_rate: 0.85,
                average_response_time_ms: 1200,
                uniqueness_ratio: 0.75,
                efficiency_score: 0.82,
            }),
            ("anthropic".to_string(), LLMPerformance {
                unique_generated: 40,
                success_rate: 0.78,
                average_response_time_ms: 1800,
                uniqueness_ratio: 0.68,
                efficiency_score: 0.71,
            }),
        ]),
        current_topic: Some("test topic".to_string()),
        active_producers: 5,
        uptime_seconds: 300,
        last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    }
}
```

## Implementation Timeline and Dependencies

### Critical Path Analysis

**Day 1 Implementation Schedule**:

| Phase | Hours | Focus | Dependencies | Tests Required |
|-------|-------|--------|--------------|----------------|
| **Phase 0** | 0-1 | Workspace setup, shared types | None | Message serialization |
| **Phase 1** | 1-3 | Orchestrator core | Shared | Process management, uniqueness |
| **Phase 2** | 2-4 | Producer implementation | Shared | Provider selection, API integration |  
| **Phase 3** | 3-5 | Web server + UI | Shared | HTTP endpoints, WebSocket |
| **Phase 4** | 4-6 | Integration + debugging | All | End-to-end workflows |
| **Phase 5** | 6-8 | Optimization features | Working system | Performance algorithms |

### Testing Strategy

**Test-Driven Development Approach**:
1. **Red**: Write failing tests that specify expected behavior
2. **Green**: Implement minimum code to make tests pass  
3. **Refactor**: Improve code while keeping tests green
4. **Repeat**: Move to next component or feature

**Testing Priority**:
1. **Unit Tests**: Core logic in each module (80% coverage target)
2. **Integration Tests**: Cross-component communication (critical path coverage)
3. **End-to-End Tests**: Complete workflows (happy path + error scenarios)

This implementation plan provides a clear roadmap for building the concurrent LLM orchestration system incrementally while maintaining high code quality through comprehensive testing at each phase.