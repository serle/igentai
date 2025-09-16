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

### 3. Run the System

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
cargo run --bin orchestrator -- --topic "Your Topic" --producers 3

# With test provider (no API keys needed)
cargo run --bin orchestrator -- --topic "Test Topic" --provider random --iterations 5

# Build first, then use the binary directly
./scripts/build.sh
./target/release/orchestrator --topic "Paris attractions" --producers 3
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
  --provider <PROVIDER>        "env" for real APIs, "random" for test data
  --output <DIR>               Output directory (default: ./output/<topic>)
  --log-level <LEVEL>          Logging detail: info, debug, trace (default: info)
  --help                       Display all available options with full descriptions
```

## Usage Examples

### Test the System (No API Keys Required)

```bash
# Quick test with random provider
./target/release/orchestrator --topic "Test Topic" --provider random --iterations 3 --producers 2
```

### Real Generation (Requires API Keys)

```bash
# Basic generation
./target/release/orchestrator --topic "Renewable Energy" --producers 3

# Limited iterations for cost control
./target/release/orchestrator --topic "AI Ethics" --producers 5 --iterations 50

# Custom output location
./target/release/orchestrator --topic "Space Technology" --output ./research/space --iterations 100
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
