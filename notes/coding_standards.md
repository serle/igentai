# Coding Standards - Concurrent LLM Orchestration System

This document establishes the coding standards and practices for the Rust-based concurrent LLM orchestration system.

## Project Overview
- **Language**: Rust (Edition 2024)
- **Architecture**: Multi-process distributed system
- **Testing Philosophy**: Comprehensive module-level testing with hierarchical validation

---

## Module Organization Standards

### File Structure Requirements

#### Module Entry Points
- **`mod.rs` files**: ONLY contain module inclusion and re-exports - no implementation code
- **`main.rs` files**: Contain the main module struct/trait/impl for each module
- **Supporting files**: Additional implementation details in conceptually named files

#### Example Module Structure
```
src/
├── orchestrator/
│   ├── mod.rs             # Module inclusion + re-exports ONLY
│   ├── main.rs            # Main Orchestrator struct/trait/impl
│   ├── manager.rs         # Supporting orchestration logic
│   ├── config.rs          # Configuration handling
│   └── tests/             # Module-specific tests
│       ├── mod.rs         # Test module inclusion + re-exports
│       ├── main.rs        # Main orchestrator tests
│       ├── manager_tests.rs # Specific component tests
│       └── fixtures.rs    # Test data/mocks
```

#### Re-export Strategy
Modules must re-export items hierarchically to hide internal file structure:

```rust
// orchestrator/mod.rs - STRUCTURE EXAMPLE
mod main;
mod manager; 
mod config;

pub use main::{Orchestrator, OrchestratorConfig};
pub use manager::{TaskManager, WorkerPool};
pub use config::{load_config, ConfigError};

#[cfg(test)]  
mod tests;
```

---

## Import and Dependency Standards

### No Embedded Fully Qualified Names (FQNs)
- **REQUIRED**: All types must be imported at the top of each source file
- **PROHIBITED**: Embedded fully qualified names in code

#### ✅ Correct Approach
```rust
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::communication::MessageBus;
use crate::producers::Producer;

pub struct Orchestrator {
    message_bus: Arc<MessageBus>,
    producers: Vec<Producer>,
}
```

#### ❌ Incorrect Approach  
```rust
pub struct Orchestrator {
    message_bus: std::sync::Arc<crate::communication::MessageBus>,
    producers: Vec<crate::producers::Producer>,
}
```

### Import Organization
```rust
// Standard library imports first
use std::sync::Arc;
use std::collections::HashMap;

// External crate imports second  
use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};

// Internal crate imports last
use crate::communication::MessageBus;
use crate::producers::Producer;
```

---

## Testing Standards

### Module-Level Testing Requirements
- **Every module MUST have comprehensive tests that pass**
- **Downstream modules can depend on tested upstream modules with confidence**
- **Tests are localized at the module level in dedicated `tests/` folders**

### Test Organization Structure  
```
module_name/
├── mod.rs                 # Module code organization
├── main.rs                # Main implementation
├── supporting_file.rs     # Supporting code
└── tests/                 # All tests for this module
    ├── mod.rs             # Test module organization with #[cfg(test)]
    ├── main.rs            # Main component tests
    ├── specific_tests.rs  # Focused test files
    ├── fixtures.rs        # Test data and mocks
    └── helpers.rs         # Test utility functions
```

### Test Configuration
- All test modules included with `#[cfg(test)]` attribute
- Test files should use descriptive names ending in `_tests.rs` when focused on specific components
- Test fixtures and helpers clearly separated from actual tests

### Test Coverage Requirements
- **Unit tests**: Core logic and business rules
- **Integration tests**: Module interfaces and communication
- **Error handling**: All error paths must be tested
- **Edge cases**: Boundary conditions and failure scenarios

---

## Naming Conventions

### Method Naming Standards
- **PREFER**: Short, conceptually named method names
- **REQUIREMENT**: Always confirm method names before implementation
- **EXAMPLES**: 
  - `start_producers()` (not `spawn_producer_processes()`)
  - `stop_producers()` (not `terminate_all_producer_processes()`)
  - `parse_response()` (not `parse_llm_api_response_content()`)

### Type Naming
- **Structs**: PascalCase, descriptive but concise
- **Enums**: PascalCase, variants in PascalCase  
- **Traits**: PascalCase, often ending in behavioral suffix
- **Constants**: SCREAMING_SNAKE_CASE

### File Naming
- **Modules**: snake_case directory names
- **Files**: snake_case with conceptual grouping
- **Test files**: `{component}_tests.rs` or descriptive names

---

## Error Handling Standards

### Module-Specific Error Types
- **REQUIRED**: Each module MUST have its own `error.rs` file with custom error types
- **PROHIBITED**: Generic `Box<dyn std::error::Error>` in module APIs
- **IMPLEMENTATION**: Use `thiserror` crate for error derivation

### Error File Organization
```rust
// Each module structure includes:
module_name/
├── mod.rs                 # Module organization
├── main.rs                # Main implementation  
├── error.rs               # Custom error types using thiserror
├── supporting_file.rs     # Supporting code
└── tests/                 # Module tests
    ├── mod.rs             # Test organization
    ├── main.rs            # Core tests
    ├── error_tests.rs     # Error handling tests
    └── fixtures.rs        # Test data
```

### Error Type Pattern
```rust
// error.rs template for each module
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModuleNameError {
    #[error("Configuration error: {message}")]
    ConfigError { message: String },
    
    #[error("Network communication failed: {source}")]
    NetworkError { 
        #[from]
        source: std::io::Error 
    },
    
    #[error("Invalid input: {input}")]
    InvalidInput { input: String },
    
    #[error("Operation timeout after {duration:?}")]
    Timeout { duration: std::time::Duration },
}

pub type ModuleResult<T> = Result<T, ModuleNameError>;
```

### Error Propagation Between Modules
```rust
// Higher-level errors can wrap lower-level errors
#[derive(Error, Debug)]
pub enum OrchestratorError {
    #[error("Producer management failed")]
    ProducerError(#[from] ProducerError),
    
    #[error("WebServer communication failed")]
    WebServerError(#[from] WebServerError),
    
    #[error("Shared component error")]
    SharedError(#[from] SharedError),
}
```

### Error Testing Requirements
- **Error Path Testing**: All error conditions must have corresponding tests
- **Error Message Validation**: Test error messages for clarity and usefulness
- **Error Conversion Testing**: Test error propagation across module boundaries
- **Recovery Testing**: Test error recovery and graceful degradation

### Error Types
- Custom error types for each module using `thiserror` crate
- Implement `std::error::Error` trait for all error types
- Provide meaningful error messages and context

### Error Propagation  
- Use `Result<T, E>` return types consistently
- Use `?` operator for error propagation
- Handle errors at appropriate abstraction levels

---

## Documentation Standards

### Code Documentation
- All public functions, structs, and modules must have doc comments
- Use `///` for public API documentation
- Use `//!` for module-level documentation
- Include examples for complex APIs

### Design Documentation
- All significant design decisions recorded in `notes/design_decisions.md`
- Rationale and alternatives considered must be documented
- Update documentation when decisions change

---

## General Rust Best Practices

### Code Quality
- No compiler warnings allowed in final submission
- Use `cargo clippy` recommendations
- Format code with `cargo fmt`
- Use appropriate visibility modifiers (`pub`, `pub(crate)`, private)

### Performance Considerations  
- Prefer `&str` over `String` for function parameters when possible
- Use `Arc` and `Rc` appropriately for shared ownership
- Minimize allocations in hot paths
- Use async/await patterns consistently

### Safety and Reliability
- Avoid `unwrap()` and `expect()` in production code paths
- Use pattern matching instead of assumptions
- Validate inputs at module boundaries
- Handle all `Result` and `Option` types explicitly

---

## Confirmation Requirements

### Method Names
- **CRITICAL**: All proposed method names must be confirmed before implementation
- Submit method names for approval in the format: "Proposing method: `method_name()` for [purpose]"
- Wait for explicit confirmation before proceeding

### Breaking Changes
- Any changes to public module interfaces require discussion
- Significant architectural changes must be documented in design decisions
- Test impact must be considered for all interface changes

---

This document will be updated as the project evolves and new standards are established.