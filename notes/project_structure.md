# Project Structure - Concurrent LLM Orchestration System

This document describes the reorganized project structure using Rust workspaces with separate crates for each component.

## Workspace Organization

```
igentai/                            # Workspace root
├── Cargo.toml                      # Workspace manifest
├── orchestrator/                   # Orchestrator crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                  # Library exports
│       ├── main.rs                 # CLI entry point
│       ├── state.rs                # Core state management
│       ├── statistics.rs           # Statistics engine
│       ├── optimization.rs         # Optimization logic
│       ├── process_manager.rs      # Child process management
│       └── tests/
│           ├── mod.rs
│           └── main.rs
├── producer/                       # Producer crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── main.rs                 # Producer process entry
│       ├── llm_client.rs           # LLM client abstraction
│       ├── openai.rs               # OpenAI implementation
│       ├── anthropic.rs            # Anthropic implementation
│       ├── provider_manager.rs     # Provider selection logic
│       └── tests/
│           ├── mod.rs
│           └── main.rs
├── webserver/                      # Web server crate
│   ├── Cargo.toml
│   ├── static/                     # Static web assets
│   │   ├── index.html
│   │   ├── app.js
│   │   └── app.css
│   └── src/
│       ├── lib.rs
│       ├── main.rs                 # Web server entry
│       ├── handlers.rs             # HTTP handlers
│       ├── websocket.rs            # WebSocket management
│       └── tests/
│           ├── mod.rs
│           └── main.rs
├── shared/                         # Shared types and utilities
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── messages.rs             # Common message types
│       ├── types.rs                # Shared data structures
│       ├── metrics.rs              # Metric definitions
│       └── errors.rs               # Common error types
├── target/                         # Build artifacts
├── output.txt                      # Generated attributes output
├── metrics.json                    # System metrics
└── state.json                      # System state persistence
```

## Workspace Cargo.toml

```toml
[workspace]
members = [
    "orchestrator",
    "producer",
    "webserver",
    "shared"
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["iGent AI"]

[workspace.dependencies]
# Common dependencies
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"
thiserror = "1.0"
anyhow = "1.0"

# Async trait
async-trait = "0.1"

# Logging
tracing = "0.4"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Shared internal crate
shared = { path = "shared" }
```

## Individual Crate Configurations

### Orchestrator Cargo.toml
```toml
[package]
name = "orchestrator"
version.workspace = true
edition.workspace = true

[[bin]]
name = "orchestrator"
path = "src/main.rs"

[dependencies]
shared.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
bincode.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true

# CLI parsing
clap = { version = "4.0", features = ["derive"] }

# WebSocket
tokio-tungstenite = "0.21"

# Process management
nix = { version = "0.27", features = ["process"] }
```

### Producer Cargo.toml
```toml
[package]
name = "producer"
version.workspace = true
edition.workspace = true

[[bin]]
name = "producer"
path = "src/main.rs"

[dependencies]
shared.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
bincode.workspace = true
async-trait.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true

# LLM clients
openai-api-rust = "0.1"
anthropic-rust = "0.1"
reqwest = { version = "0.11", features = ["json"] }

# Random selection
rand = "0.8"

# Bloom filter (Phase 2)
bloom = { version = "0.3", optional = true }

[features]
bloom-filter = ["bloom"]
```

### Webserver Cargo.toml
```toml
[package]
name = "webserver"
version.workspace = true
edition.workspace = true

[[bin]]
name = "webserver"
path = "src/main.rs"

[dependencies]
shared.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# WebSocket
tokio-tungstenite = "0.21"

# Static file embedding
rust-embed = "8.0"
```

### Shared Cargo.toml
```toml
[package]
name = "shared"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

# UUID for IDs
uuid = { version = "1.0", features = ["v4", "serde"] }
```

## Building and Running

### Development Build
```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p orchestrator
```

### Running the System
```bash
# Start orchestrator (which spawns other processes)
cargo run -p orchestrator -- -n 5 -p 8080

# Or with release optimizations
cargo run --release -p orchestrator -- -n 5 -p 8080
```

### Running Individual Components (for testing)
```bash
# Run orchestrator
cargo run -p orchestrator

# Run a producer manually
cargo run -p producer -- --orchestrator-addr localhost:8083 --producer-id test-producer

# Run web server manually
cargo run -p webserver -- --port 8080 --orchestrator-addr localhost:8082
```

## Testing

### Unit Tests
```bash
# Test all crates
cargo test

# Test specific crate
cargo test -p orchestrator
```

### Integration Tests
```bash
# Run workspace-level integration tests
cargo test --workspace
```

## Module Organization

Following the coding standards, each crate follows this structure:
- `mod.rs` files contain only module inclusion and re-exports
- `main.rs` files contain the main module struct/trait/impl
- Supporting files contain additional implementation details
- `tests/` directories contain module-level tests

## Benefits of This Structure

1. **Clear Separation**: Each component is a distinct crate with its own dependencies
2. **Parallel Compilation**: Rust can compile crates in parallel
3. **Selective Dependencies**: Each crate only includes what it needs
4. **Shared Types**: Common types and messages defined once in `shared`
5. **Independent Testing**: Each crate can be tested in isolation
6. **Deployment Flexibility**: Can deploy components separately if needed