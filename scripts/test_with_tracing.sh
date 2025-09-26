#!/bin/bash

# Test script that demonstrates proper trace collection with endpoints
# This script shows how to enable trace collection when actually needed

set -e  # Exit on any error

echo "🧪 LLM Orchestration System - With Trace Collection"
echo "=================================================="
echo ""

# Configuration
TOPIC="${1:-test topic}"
PRODUCER_COUNT="${2:-3}"
LOG_DIR="logs"
LOG_FILE="$LOG_DIR/test_debug_$(date +%Y%m%d_%H%M%S).log"
TRACE_LOG="$LOG_DIR/trace_collector.log"

# Trace collection endpoint (port 8081)
TRACE_ENDPOINT="http://127.0.0.1:8081"

echo "📝 Configuration:"
echo "   - Topic: $TOPIC"
echo "   - Producers: $PRODUCER_COUNT"  
echo "   - Log file: $LOG_FILE"
echo "   - Trace endpoint: $TRACE_ENDPOINT"
echo "   - Trace log: $TRACE_LOG"
echo ""

# Change to project root
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

# Create logs directory if it doesn't exist
mkdir -p "$LOG_DIR"

echo "🧹 Cleaning up existing processes..."
pkill -f "orchestrator|producer|webserver|tester" 2>/dev/null || true
sleep 1

# Start a simple HTTP server to collect traces (mock collector)
echo "🗃️  Starting trace collector on port 8081..."
(while true; do echo -e "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK" | nc -l 8081; done) &
TRACE_COLLECTOR_PID=$!

# Create trace log file for the collector to write to
echo "📊 Trace Collection Started - $(date)" > "$TRACE_LOG"

sleep 2

echo "🚀 Starting orchestrator with debug logging and trace endpoint..."
cargo run --bin orchestrator -- --log-level debug --trace-ep "$TRACE_ENDPOINT" > "$LOG_FILE" 2>&1 &
ORCHESTRATOR_PID=$!

echo "   - Orchestrator PID: $ORCHESTRATOR_PID"
echo "   - Trace Collector PID: $TRACE_COLLECTOR_PID"
echo "   - Waiting for system to initialize..."

# Wait for orchestrator to be ready
MAX_WAIT=30
WAITED=0
while ! curl -s http://localhost:8080 >/dev/null 2>&1; do
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo "❌ Timeout waiting for orchestrator to start"
        echo "📋 Recent log output:"
        tail -n 20 "$LOG_FILE" 2>/dev/null || echo "No log output available"
        
        # Cleanup
        kill $ORCHESTRATOR_PID 2>/dev/null || true
        kill $TRACE_COLLECTOR_PID 2>/dev/null || true
        exit 1
    fi
    echo -n "."
    sleep 1
    WAITED=$((WAITED + 1))
done
echo " Ready!"

echo ""
echo "🎯 Starting topic generation..."
RESPONSE=$(curl -s -X POST \
  "http://localhost:8080/api/start" \
  -H "Content-Type: application/json" \
  -d "{\"topic\": \"$TOPIC\", \"producer_count\": $PRODUCER_COUNT, \"iterations\": 3}")

echo "📡 API Response: $RESPONSE"

echo ""
echo "⏱️  Letting producers run for 15 seconds..."
sleep 15

echo ""
echo "🛑 Stopping system..."
curl -s -X POST "http://localhost:8080/api/stop" >/dev/null

# Wait for graceful shutdown
sleep 2

# Stop processes
if ps -p $ORCHESTRATOR_PID > /dev/null 2>&1; then
    kill $ORCHESTRATOR_PID
    echo "✅ Orchestrator stopped"
fi

if ps -p $TRACE_COLLECTOR_PID > /dev/null 2>&1; then  
    kill $TRACE_COLLECTOR_PID
    echo "✅ Trace collector stopped"
fi

echo ""
echo "📁 Debug log saved to: $LOG_FILE"
echo "📁 Trace log saved to: $TRACE_LOG"
echo "📊 Trace collection was enabled with endpoint: $TRACE_ENDPOINT"
echo ""
echo "💡 This script shows how to properly enable trace collection when needed"
echo "💡 The trace_collector.log file is only created when traces are actually collected"
echo ""