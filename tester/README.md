# E2E Testing Framework

A clean, Topic-centric end-to-end testing framework for the distributed system.

## Quick Start

```rust
use tester::*;

// Configure orchestrator (CLI mode by default)  
let config = OrchestratorConfig::builder()
    .topic("my_test")
    .producers(2) 
    .iterations(Some(3))
    .build();

// Start orchestrator
let mut constellation = ServiceConstellation::new(trace_endpoint);
constellation.start_orchestrator(config).await?;

// Wait for topic completion and test
if let Some(topic) = Topic::wait_for_topic("my_test", collector, timeout).await {
    assert!(topic.assert_completed().await);
    assert!(topic.assert_min_attributes(10));
    assert!(topic.assert_no_errors().await);
}
```

## Key Components

### 1. **Topic** - Main Testing Interface
The `Topic` struct is the primary interface for E2E testing. It provides:
- Access to all trace events for the topic execution
- Access to generated output data  
- Built-in assertion methods
- Debugging and inspection utilities

### 2. **OrchestratorConfig** - Flexible Configuration
Built using a clean builder pattern:

```rust
// CLI Mode (default) - uses producer_addr default of 6001
let config = OrchestratorConfig::builder()
    .topic("test_topic")
    .producers(3)
    .iterations(Some(5))
    .provider("test") 
    .build();

// CLI Mode with custom producer address
let config = OrchestratorConfig::builder()
    .topic("test_topic")
    .producers(3)
    .producer_addr("127.0.0.1:7001")  // Only producer_addr matters in CLI mode
    .build();

// WebServer Mode (with defaults)
let config = OrchestratorConfig::builder()
    .webserver_mode()
    .build();

// WebServer Mode (with custom addresses)
let config = OrchestratorConfig::builder()
    .webserver_mode()
    .webserver_addr("127.0.0.1:9000")
    .producer_addr("127.0.0.1:9001")
    .build();
```

### 3. **ServiceConstellation** - Simple Process Management
Unified interface for starting orchestrator:

```rust
let mut constellation = ServiceConstellation::new(trace_endpoint);
constellation.start_orchestrator(config).await?;
```

## Package Structure

```
tester/src/
├── lib.rs                 # Main exports & documentation
├── main.rs               # CLI runner
├── config/               # Configuration management
│   ├── mod.rs           #   - Re-exports and module organization
│   ├── orchestrator.rs  #   - OrchestratorConfig & OrchestratorMode
│   └── builder.rs       #   - OrchestratorConfigBuilder
├── runtime/              # Process & service management  
│   ├── mod.rs           #   - Re-exports and module organization
│   ├── constellation.rs #   - ServiceConstellation (process mgmt)
│   └── collector.rs     #   - TracingCollector (event collection)
├── testing/              # Testing framework
│   ├── mod.rs           #   - Re-exports and module organization
│   ├── topic.rs         #   - Topic (main testing interface) ⭐
│   ├── assertions.rs    #   - TracingAssertions (low-level)
│   └── output.rs        #   - OutputLoader & utilities
└── scenarios/            # Test scenarios
    ├── mod.rs           #   - TestScenarios runner
    ├── basic.rs         #   - Traditional assertion examples
    └── examples.rs      #   - Clean Topic API examples ⭐
```

### Core Types (Re-exported at crate root)
- `Topic` - Main testing interface (testing/)
- `OrchestratorConfig` - Configuration struct (config/)
- `OrchestratorConfigBuilder` - Builder for configuration (config/)
- `OrchestratorMode` - CLI or WebServer mode enum (config/)
- `ServiceConstellation` - Process management (runtime/)

### Supporting Types  
- `TracingCollector` - Event collection (runtime/)
- `OutputLoader` - Output file loading (testing/)
- `TracingAssertions` - Low-level assertions (testing/)

## Example Scenarios

### CLI Mode Test
```rust
let config = OrchestratorConfig::builder()
    .topic("cli_test")
    .producers(2)
    .iterations(Some(3))
    .build();
    
constellation.start_orchestrator(config).await?;

if let Some(topic) = Topic::wait_for_topic("cli_test", collector, timeout).await {
    assert!(topic.assert_completed().await);
    assert!(topic.assert_started_with_budget(Some(3)).await);
    assert!(topic.assert_min_attributes(5));
    
    topic.print_summary();
    println!("Sample: {:?}", topic.sample_attributes(3));
}
```

### WebServer Mode Test  
```rust
// Method 1: Keep running for manual testing
let config = OrchestratorConfig::builder()
    .webserver_mode()  // Uses defaults: webserver=8080, producer=6001
    .build();
    
constellation.start_orchestrator(config).await?;
constellation.keep_running().await?;  // Runs indefinitely

// Method 2: Wait for specific topic indefinitely  
if let Some(topic) = Topic::wait_for_topic_indefinitely("my_topic", collector).await {
    assert!(topic.assert_completed().await);
}

// Method 3: Wait for topic with timeout (hybrid approach)
if let Some(topic) = Topic::wait_for_topic("my_topic", collector, Duration::from_secs(60)).await {
    assert!(topic.assert_completed().await);
}
```

## Benefits

1. **Clean API**: `Topic::wait_for_topic()` is the main entry point
2. **Self-contained**: Each Topic has all trace events and output data
3. **Simple configuration**: Builder pattern with sensible defaults
4. **Unified orchestrator startup**: Single method handles CLI/WebServer modes
5. **Built-in debugging**: Summary, trace printing, sample data access
6. **Type-safe**: Strong typing with clean re-exports

## Running Tests

```bash
# Run default scenario (CLI mode)
cargo run --bin tester

# Run specific scenario
cargo run --bin tester -- --scenario basic
cargo run --bin tester -- --scenario examples  

# Run WebServer mode (keeps running for manual HTTP testing)
cargo run --bin tester -- --scenario webserver

# Keep services running for debugging
cargo run --bin tester -- --keep-running

# Verbose output
cargo run --bin tester -- --verbose
```

## WebServer Mode Manual Testing

When running the `webserver` scenario, the tester will:

1. **Start orchestrator in WebServer mode** (defaults: http://127.0.0.1:8080)
2. **Keep running indefinitely** - waiting for HTTP requests
3. **Monitor trace events** in real-time
4. **Allow manual testing** via HTTP requests

### Making HTTP Requests

```bash
# Example HTTP request to trigger topic generation
curl -X POST http://127.0.0.1:8080/topics \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "manual_test",
    "producers": 2,
    "iterations": 3,
    "provider": "test"
  }'
```

The tester will capture all trace events and you can see real-time progress in the logs.