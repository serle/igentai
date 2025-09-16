# Concurrent LLM Orchestration System

A distributed Rust system that orchestrates multiple LLM producers to exhaustively explore topics and generate unique attributes. The system uses dependency injection for testability and comprehensive end-to-end testing via distributed tracing.

## Architecture Overview

```
                    ┌─────────┐
                    │  LLM 1  │
                    └────┬────┘
                         │
┌─────────┐         ┌────▼────┐         ┌─────────┐
│  LLM 2  ├────────►│         │◄────────┤ Web UI  │
└─────────┘         │Orchestr-│         └─────────┘
                    │   ator  │
┌─────────┐         │         │         ┌──────────┐
│  LLM N  ├────────►│         ├────────►│Attribute │
└─────────┘         └─────────┘         │  File    │
                                        └──────────┘
```

The system consists of:
- **Orchestrator**: Central coordinator managing producer processes and deduplicating results
- **Producers**: Individual processes calling LLM APIs (OpenAI, Anthropic, Gemini) to generate attributes
- **WebServer**: Real-time web interface for metrics and control
- **Tester**: End-to-end testing framework using distributed tracing for validation

## Quick Start

### Build and Setup

```bash
# Build all components for production
./scripts/build.sh

# Setup API keys (copy example.env to .env and configure)
cp example.env .env
# Edit .env with your OPENAI_API_KEY (minimum required)
```

### Basic Usage

```bash
# Start the orchestrator with web interface
./scripts/start.sh

# Start with debug logging
./scripts/start.sh --log-level debug

# Open your web browser to http://localhost:8080

# Stop the orchestrator
./scripts/stop.sh
```

## CLI Mode

The orchestrator's CLI mode provides direct topic generation without requiring the web interface. This is the primary mode for batch processing and automated workflows.

### Key Features

- **Direct Topic Processing**: Specify topic and immediately start generation
- **Configurable Producers**: Set number of concurrent LLM processes
- **Multiple Providers**: Support for OpenAI, Anthropic, Gemini, and test providers
- **Automatic Deduplication**: Maintains uniqueness across all producers using Bloom filters
- **File Output**: Results saved to `./output/<topic>/attributes.txt`

### CLI Options

```bash
./target/release/orchestrator [OPTIONS]

Key CLI Mode Options:
  --topic <TOPIC>              Topic for generation (enables CLI mode)
  --producers <N>              Number of producer processes (default: 5)
  --iterations <N>             Max iterations per producer (unlimited if not set)
  --provider <PROVIDER>        "env" for real APIs, "test" for mock data
  --request-size <SIZE>        Items per API call (default: 60)
  --output <DIR>               Output directory (default: ./output/<topic>)
  --log-level <LEVEL>          Log level: trace, debug, info, warn, error (default: info)
  --trace-ep <URL>             Optional tracing endpoint URL
```

### Examples

```bash
# Basic generation
./target/release/orchestrator --topic "Paris attractions" --producers 3

# With iteration limit
./target/release/orchestrator --topic "Programming languages" --producers 5 --iterations 50

# Limited run with custom output
./target/release/orchestrator \
  --topic "Machine learning concepts" \
  --producers 3 \
  --iterations 100 \
  --output ./results/ml_concepts

# Test mode (no API keys needed)
./target/release/orchestrator --topic "Test topic" --providers 2 --provider test

# Debug logging for troubleshooting
./target/release/orchestrator --topic "Debug test" --providers 2 --provider test --log-level debug
```

## Testing Strategy

The system employs a comprehensive testing approach built around dependency injection and end-to-end validation using the `tester` binary.

### Dependency Injection Architecture

All components use trait-based dependency injection for testability:

```rust
// Orchestrator with injected services
let mut orchestrator = Orchestrator::new(
    api_keys,      // ApiKeySource trait
    communicator,  // Communicator trait  
    file_system,   // FileSystem trait
    process_manager, // ProcessManager trait
);
```

This allows for:
- **Mock implementations** for unit testing
- **Real implementations** for production
- **Test implementations** for end-to-end scenarios

### The Tester Binary

The `tester` binary provides end-to-end validation of the entire distributed system using distributed tracing as the primary testing mechanism.

#### How It Works

1. **Tracing-Based Testing**: Instead of mocking components, the tester starts real processes and uses distributed tracing to validate behavior
2. **Topic-Centric API**: Tests are organized around "topics" - complete end-to-end workflows
3. **Service Constellation Management**: Manages the lifecycle of orchestrator, producers, and webserver processes
4. **Real-World Validation**: Tests actual IPC communication, file I/O, and LLM API interactions

#### Running Tests

```bash
# Default test scenario (CLI mode with test provider)
cargo run --bin tester

# WebServer mode testing (starts server, waits for HTTP requests)
cargo run --bin tester -- --scenario webserver

# Keep services running for manual debugging
cargo run --bin tester -- --keep-running

# Verbose tracing output
cargo run --bin tester -- --verbose
```

#### Test Framework API

The tester provides a clean API for end-to-end testing:

```rust
// Configure test scenario
let config = OrchestratorConfig::builder()
    .topic("test_topic")
    .producers(3)
    .iterations(Some(10))
    .build();

// Start distributed system
let mut constellation = ServiceConstellation::new(trace_endpoint);
constellation.start_orchestrator(config).await?;

// Wait for completion and validate via tracing
if let Some(topic) = Topic::wait_for_topic("test_topic", collector, timeout).await {
    assert!(topic.assert_completed().await);
    assert!(topic.assert_min_attributes(20));
    assert!(topic.assert_no_errors().await);
    
    topic.print_summary();
}
```

#### Tracing-Based Validation

The tester uses distributed tracing spans to validate system behavior:

- **Process Lifecycle**: Validates orchestrator and producer startup/shutdown
- **IPC Communication**: Verifies message passing between components
- **API Interactions**: Confirms LLM API calls and responses
- **File Operations**: Validates output file creation and content
- **Error Handling**: Ensures graceful failure recovery

#### Benefits of This Approach

1. **Real System Testing**: Tests actual production code paths, not mocks
2. **Distributed Validation**: Validates cross-process communication and coordination
3. **Tracing Integration**: Provides detailed observability into test execution
4. **Scalable**: Can test with varying numbers of producers and configurations
5. **Debugging**: Trace data provides rich debugging information when tests fail

### Test Scenarios

The tester includes several built-in scenarios:

- **Basic**: Simple CLI mode validation with test provider
- **Examples**: Comprehensive topic-based testing examples
- **WebServer**: Interactive WebServer mode testing with HTTP API validation

This testing strategy ensures the distributed system works correctly across all components while providing detailed observability into system behavior during test execution.