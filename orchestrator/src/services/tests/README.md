# Service Tests

This directory contains comprehensive tests for all orchestrator services. Each service has its own test file with dedicated test utilities.

## Test Structure

```
src/services/tests/
├── mod.rs                    # Common test utilities and module declarations
├── api_keys.rs              # RealApiKeySource tests (2 tests)
├── communication.rs         # RealMessageTransport tests (16 tests)
├── file_system.rs          # RealFileSystem tests (10 tests)
└── process_management.rs   # RealProcessManager tests (11 tests)
```

## Test Coverage Summary

### API Keys Service (api_keys.rs)
- **2 tests**: Environment variable loading and .env file parsing
- **Focus**: API key validation and multi-provider support
- **Key Scenarios**: Required vs optional keys, environment precedence

### Communication Service (communication.rs)  
- **16 tests**: Channel management and message coordination
- **Focus**: Producer/webserver channel availability scenarios
- **Key Scenarios**: Channel establishment, reestablishment, backpressure handling, concurrent processing

### File System Service (file_system.rs)
- **10 tests**: File management and synchronization
- **Focus**: Topic folder creation, attribute queuing, batch synchronization
- **Key Scenarios**: Folder overwrite, name sanitization, concurrent writes, complete workflow

### Process Management Service (process_management.rs)
- **11 tests**: Process lifecycle management
- **Focus**: Producer/webserver spawning, monitoring, restart scenarios
- **Key Scenarios**: Process health monitoring, failure recovery, concurrent management

## Running Service Tests

```bash
# Run all service tests
cargo test services::tests

# Run tests for specific service
cargo test services::tests::communication
cargo test services::tests::process_management
cargo test services::tests::api_keys
cargo test services::tests::file_system

# Run specific test
cargo test services::tests::communication::test_establish_producer_channels
```

## Test Utilities

The `common` module in `mod.rs` provides shared utilities:

- `TEST_TIMEOUT`: Standard timeout for async operations
- `with_timeout()`: Helper for timeout-based operations
- `test_producer_id()`: Generate test producer IDs

## Benefits of This Structure

1. **Localized Testing**: Tests are co-located with service implementations
2. **Service-Specific Fixtures**: Each service can have its own test utilities
3. **Clear Separation**: Service tests are separate from integration/unit tests
4. **Modular Organization**: Easy to find and maintain service-specific tests
5. **Independent Test Suites**: Each service test suite can run independently

## Integration with Main Test Suite

Service tests are part of the main library test suite and run with `cargo test`. They complement:

- `tests/integration.rs`: Full system integration tests
- `tests/unit.rs`: Individual component unit tests
- `tests/common/`: Shared integration test utilities

Total: **39 service-specific tests** ensuring comprehensive coverage of all critical orchestrator services.