#!/bin/bash

# Test script that starts the system with debug logging and automatically starts a topic
# This script captures all debug output and makes it easy to diagnose issues

set -e  # Exit on any error

echo "🧪 LLM Orchestration System - Debug Test Script"
echo "=============================================="
echo ""

# Configuration
TOPIC="${1:-test topic}"
PRODUCER_COUNT="${2:-3}"
LOG_DIR="logs"
LOG_FILE="$LOG_DIR/test_debug_$(date +%Y%m%d_%H%M%S).log"

echo "📝 Configuration:"
echo "   - Topic: $TOPIC"
echo "   - Producers: $PRODUCER_COUNT"
echo "   - Log file: $LOG_FILE"
echo ""

# Change to project root
cd "$(dirname "$0")/.."

# Create logs directory if it doesn't exist
mkdir -p "$LOG_DIR"

echo "🧹 Cleaning up existing processes..."
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true
sleep 1

# Note: trace_collector.log is not created unless tracing endpoint is configured
# This script only uses console debug logging, not file-based trace collection

echo "🚀 Starting orchestrator with debug logging..."
cargo run --bin orchestrator -- --log-level debug > "$LOG_FILE" 2>&1 &
ORCHESTRATOR_PID=$!

echo "   - Orchestrator PID: $ORCHESTRATOR_PID"
echo "   - Waiting for system to initialize..."

# Wait for orchestrator to be ready
MAX_WAIT=30
WAITED=0
while ! curl -s http://localhost:8080 >/dev/null 2>&1; do
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo "❌ Timeout waiting for orchestrator to start"
        echo "📋 Last 50 lines of log:"
        tail -n 50 "$LOG_FILE"
        exit 1
    fi
    sleep 1
    WAITED=$((WAITED + 1))
    echo -n "."
done
echo " Ready!"

echo ""
echo "🎯 Starting topic generation..."
RESPONSE=$(curl -s -X POST http://localhost:8080/api/start \
    -H "Content-Type: application/json" \
    -d "{\"topic\": \"$TOPIC\", \"producer_count\": $PRODUCER_COUNT}")

echo "📡 API Response: $RESPONSE"

# Let it run for a bit to collect some data
echo ""
echo "⏱️  Letting producers run for 15 seconds..."
sleep 15

echo ""
echo "📊 System Status:"
echo "=================="

# Check processes
echo ""
echo "🔍 Running processes:"
ps aux | grep -E "(orchestrator|producer|webserver)" | grep -v grep | awk '{print "   -", $11, $12, $13, $14, "(PID:", $2")"}'

# Check for errors in logs
echo ""
echo "⚠️  Checking for errors in logs..."
ERROR_COUNT=$(grep -c "ERROR\|error\|Failed\|failed" "$LOG_FILE" 2>/dev/null || echo "0")
WARN_COUNT=$(grep -c "WARN\|warn\|Warning" "$LOG_FILE" 2>/dev/null || echo "0")

echo "   - Errors found: $ERROR_COUNT"
echo "   - Warnings found: $WARN_COUNT"

if [ "$ERROR_COUNT" -gt "0" ]; then
    echo ""
    echo "❌ Recent errors (last 10):"
    grep -E "ERROR|error|Failed|failed" "$LOG_FILE" | tail -n 10 | sed 's/^/   /'
fi

# Show recent debug output
echo ""
echo "📋 Recent debug output (last 30 lines):"
echo "========================================"
tail -n 30 "$LOG_FILE"

# Show producer activity
echo ""
echo "🏭 Producer Activity Summary:"
echo "============================="
grep -E "producer.*spawned|producer.*ready|producer.*failed|AttributeBatch" "$LOG_FILE" | tail -n 20 | sed 's/^/   /'

# Prompt for continuation
echo ""
echo "✅ Test completed. System is running."
echo ""
echo "Options:"
echo "  - Press Enter to stop the system and exit"
echo "  - Press Ctrl+C to keep system running and exit"
echo ""
read -p "Your choice: " -n 1 -r

if [[ ! $REPLY =~ ^[Cc]$ ]]; then
    echo ""
    echo "🛑 Stopping system..."
    pkill -f "orchestrator|producer|webserver" 2>/dev/null || true
    echo "✅ System stopped"
fi

echo ""
echo "📁 Debug log saved to: $LOG_FILE"
echo "📁 Trace collection: Not enabled (console logging only)"
echo ""