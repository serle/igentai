# Producer Service Tests

This directory contains comprehensive unit and integration tests for all producer services.

## Test Structure

- **`provider_router.rs`**: Tests for provider selection, routing strategies, and API interactions
- **`response_processor.rs`**: Tests for response parsing, bloom filtering, and attribute extraction
- **`ipc_communicator.rs`**: Tests for orchestrator communication and message handling
- **`performance_tracker.rs`**: Tests for metrics collection and provider health monitoring
- **`producer_integration.rs`**: Integration tests combining multiple services

## Test Coverage

Tests cover:
- ✅ Happy path scenarios
- ✅ Error conditions and edge cases
- ✅ Mock external dependencies
- ✅ Concurrent operations
- ✅ Resource cleanup
- ✅ Configuration validation

## Running Tests

```bash
# Run all tests
cargo test

# Run producer service tests only
cargo test services::tests

# Run specific test module
cargo test services::tests::provider_router
```