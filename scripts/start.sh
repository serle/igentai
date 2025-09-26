#!/bin/bash

# Start script for the LLM Orchestration System
# This script cleans up any existing processes and starts the orchestrator

set -e  # Exit on any error

# Change to the project root directory (script is in scripts/ subdirectory)
cd "$(dirname "$0")/.."

# Clear any inherited environment variables that might conflict with .env
unset ROUTING_STRATEGY ROUTING_PRIMARY_PROVIDER ROUTING_PROVIDERS ROUTING_WEIGHTS 2>/dev/null || true

# Load environment variables from .env file if it exists
if [ -f ".env" ]; then
    echo "📋 Loading environment from .env file..."
    set -a  # Export all variables
    source .env
    set +a  # Stop exporting
else
    echo "⚠️  Warning: .env file not found - using system environment variables only"
fi

echo "🧹 Cleaning up existing processes..."

# Kill any existing orchestrator, producer, or webserver processes
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true

# Wait a moment for processes to fully terminate
sleep 1

echo "✅ Cleanup complete"

echo "🚀 Starting orchestrator..."
echo "   Web interface will be available at: http://localhost:8080"
echo "   Routing Strategy: ${ROUTING_STRATEGY:-backoff}"
echo "   Primary Provider: ${ROUTING_PRIMARY_PROVIDER:-openai}"
echo "   Use --log-level debug for verbose output"
echo "   Press Ctrl+C to stop"
echo ""

# Check if OpenAI API key is available
if [ -z "$OPENAI_API_KEY" ]; then
    echo "⚠️  Warning: OPENAI_API_KEY environment variable is not set"
    echo "   The system will fall back to other available providers or Random provider"
    echo ""
fi

# Start the orchestrator with env provider (uses OpenAI if API key is available) and any passed arguments
cargo run --bin orchestrator -- --provider env "$@"