#!/bin/bash

# Start script for the LLM Orchestration System
# This script cleans up any existing processes and starts the orchestrator

set -e  # Exit on any error

echo "🧹 Cleaning up existing processes..."

# Kill any existing orchestrator, producer, or webserver processes
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true

# Wait a moment for processes to fully terminate
sleep 1

echo "✅ Cleanup complete"

# Change to the project root directory (script is in scripts/ subdirectory)
cd "$(dirname "$0")/.."

echo "🚀 Starting orchestrator..."
echo "   Web interface will be available at: http://localhost:8080"
echo "   Use --log-level debug for verbose output"
echo "   Press Ctrl+C to stop"
echo ""

# Start the orchestrator with any passed arguments
cargo run --bin orchestrator -- "$@"