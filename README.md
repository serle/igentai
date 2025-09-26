# Concurrent LLM Orchestration System

A distributed Rust system that orchestrates multiple LLM providers to explore topics and generate unique attributes. The system coordinates OpenAI, Anthropic, Gemini, and test providers to maximize content generation while eliminating duplicates across all sources.

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

- **Orchestrator**: Central coordinator managing producer processes and eliminating duplicates
- **Producers**: Individual processes calling LLM APIs to generate attributes
- **WebServer**: Real-time web interface for monitoring and control
- **Tester**: End-to-end testing framework using distributed tracing

## Bugs/Unfinished Work

1. The OpenAI API key does not work, so I have not yet been able to test against a real LLM.
2. The websocket stops updating the browser after a while, although the orchestrator continues to send messages. Working on fixing this issue.

## Quick Start

### 1. Build the System

```bash
# Build all components
./scripts/build.sh
```

### 2. Configure API Keys (Optional)

For real LLM providers, set up your API keys:

```bash
# Copy the example environment file
cp example.env .env

# Edit .env and add your API keys (at minimum, OPENAI_API_KEY)
# You can also set keys as environment variables
export OPENAI_API_KEY="your-key-here"
```

### 3. Configure Routing Strategy (Optional)

The system supports multiple routing strategies for load balancing across providers with specific models using a unified configuration format:

```bash
# Single provider with model (default)
ROUTING_STRATEGY=backoff
ROUTING_CONFIG=openai:gpt-4o-mini

# Round-robin across multiple providers with their models
ROUTING_STRATEGY=roundrobin
ROUTING_CONFIG=openai:gpt-4o-mini,anthropic:claude-3-sonnet,gemini:gemini-pro

# Priority order (try cheapest first)
ROUTING_STRATEGY=priority
ROUTING_CONFIG=gemini:gemini-pro,openai:gpt-4o-mini,anthropic:claude-3-sonnet

# Weighted distribution with models and weights
ROUTING_STRATEGY=weighted
ROUTING_CONFIG=openai:gpt-4o-mini:0.5,anthropic:claude-3-sonnet:0.3,gemini:gemini-pro:0.2
```

#### Environment Variables and CLI Arguments

The environment variables align exactly with CLI arguments:

```bash
# Environment variables
ROUTING_STRATEGY=roundrobin
ROUTING_CONFIG=openai:gpt-4o-mini,anthropic:claude-3-sonnet

# Equivalent CLI arguments
--routing-strategy roundrobin --routing-config "openai:gpt-4o-mini,anthropic:claude-3-sonnet"
```

### 4. Run the System

Choose between two modes:

#### Web Mode (Recommended for Interactive Use)

```bash
# Start with web interface at http://localhost:8080
./scripts/start.sh

# With debug logging
./scripts/start.sh --log-level debug

# Open your browser to http://localhost:8080
# Stop with Ctrl+C or:
./scripts/stop.sh
```

#### CLI Mode (For Automated/Batch Processing)

```bash
# Direct command line usage - no web interface
cargo run --bin orchestrator -- --topic "Paris attractions" --producers 3 --routing-strategy backoff --routing-config "openai:gpt-4o-mini"

# With test provider (no API keys needed)  
cargo run --bin orchestrator -- --topic "Paris attractions" --routing-strategy backoff --routing-config "random:random" --iterations 5

# Build first, then use the binary directly
./scripts/build.sh
./target/release/orchestrator --topic "Paris attractions" --producers 3 --routing-strategy backoff --routing-config "openai:gpt-4o-mini"
```

## Operating Modes Explained

### Web Mode

- **Best for**: Interactive exploration, monitoring, real-time control
- **Features**: Live web dashboard, real-time metrics, visual progress tracking
- **Access**: Open http://localhost:8080 in your browser
- **Control**: Start/stop generation through web interface
- **Output**: Files saved to `./output/<topic>/` plus web display

### CLI Mode

- **Best for**: Automated workflows, batch processing, scripting
- **Features**: Direct command-line control, immediate execution, script integration
- **Control**: All configuration through command-line flags
- **Output**: Files saved to `./output/<topic>/` (or custom directory)
- **Completion**: Automatically stops when iterations complete or manually with Ctrl+C

## CLI Options

The following are the most commonly used command-line options. For a complete list of all available options with detailed descriptions and defaults, use `--help`:

```bash
./target/release/orchestrator [OPTIONS]

Key Options:
  --topic <TOPIC>              Topic for generation (enables CLI mode)
  --producers <N>              Number of producer processes (default: 5)
  --iterations <N>             Max iterations per producer (default: unlimited)
  --max-requests <N>           Max requests per producer (overrides iterations)
  --routing-strategy <STRATEGY> Load balancing: backoff, roundrobin, priority, weighted
  --routing-config <CONFIG>     Provider:model configuration for routing (e.g., "openai:gpt-4o-mini")
  --output <DIR>               Output directory (default: ./output/<topic>)
  --log-level <LEVEL>          Logging detail: info, debug, trace (default: info)
  --help                       Display all available options with full descriptions
```

## Usage Examples

### Test the System (No API Keys Required)

```bash
# Quick test with random provider
./target/release/orchestrator --topic "Paris attractions" --routing-strategy backoff --routing-config "random:random" --iterations 3 --producers 2
```

### Real Generation (Requires API Keys)

```bash
# Basic generation with default backoff routing
./target/release/orchestrator --topic "Renewable Energy" --producers 3 --routing-strategy backoff --routing-config "openai:gpt-4o-mini"

# Limited requests for cost control
./target/release/orchestrator --topic "AI Ethics" --producers 5 --routing-strategy backoff --routing-config "openai:gpt-4o-mini" --max-requests 50

# Custom routing strategy with specific models
./target/release/orchestrator --topic "Space Technology" --routing-strategy roundrobin --routing-config "openai:gpt-4o-mini,anthropic:claude-3-sonnet"

# Multiple providers with priority routing and weighted distribution
./target/release/orchestrator --topic "Climate Change" --routing-strategy weighted --routing-config "openai:gpt-4o-mini:0.6,gemini:gemini-pro:0.4" --max-requests 100
```

### Web Mode Usage

```bash
# Start web interface
./scripts/start.sh

# Then open http://localhost:8080 and:
# 1. Enter a topic in the web form
# 2. Set producer count and iterations
# 3. Click "Start" to begin generation
# 4. Watch real-time progress and results
```

## Output

The system creates structured output in the specified directory:

- `output.txt` - Plain text list of unique attributes
- `output.json` - Structured JSON with metadata
- `metadata.json` - Generation statistics and settings

## Testing

The system includes comprehensive testing capabilities:

```bash
# Run end-to-end tests with random provider
cargo run --bin tester

# Test web mode (starts server for manual testing)
cargo run --bin tester -- --scenario webserver

# Keep test environment running for debugging
cargo run --bin tester -- --keep-running
```

The testing framework uses distributed tracing to validate real system behavior across all components, providing more reliable validation than traditional mocking approaches.

## System Requirements

- **Rust**: 1.70+ (included in sandbox)
- **Memory**: 2GB+ recommended for multiple producers
- **Network**: Internet access for LLM API calls
- **API Keys**: At least OpenAI for real generation (optional for testing)

## Troubleshooting

### Common Issues

**"OpenAI API key should start with 'sk-'"**

- Set a valid API key in `.env` file or environment variable
- Or use `--provider random` for testing without API keys

**"Connection refused" or port errors**

- Stop existing processes: `./scripts/stop.sh`
- Check no other services are using port 8080

**Low generation speed**

- Increase producer count: `--producers N`
- Check API key rate limits
- Monitor with `--log-level debug`

Run with `--log-level debug` for detailed diagnostic information.
