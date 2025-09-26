#!/bin/bash

# Comprehensive test script that starts the system, runs tests, and analyzes the output
# Usage: ./test_and_analyze.sh [topic] [producer_count] [duration_seconds]

set -e  # Exit on any error

# Configuration
TOPIC="${1:-test topic}"
PRODUCER_COUNT="${2:-3}"
DURATION="${3:-20}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_DIR="logs"
DEBUG_LOG="$LOG_DIR/debug_${TIMESTAMP}.log"
ANALYSIS_LOG="$LOG_DIR/analysis_${TIMESTAMP}.log"

echo "🧪 LLM Orchestration System - Test & Analysis"
echo "============================================="
echo ""
echo "📝 Configuration:"
echo "   - Topic: $TOPIC"
echo "   - Producers: $PRODUCER_COUNT"
echo "   - Test Duration: ${DURATION}s"
echo "   - Debug Log: $DEBUG_LOG"
echo "   - Analysis Log: $ANALYSIS_LOG"
echo ""

# Change to project root
cd "$(dirname "$0")/.."

# Create logs directory if it doesn't exist
mkdir -p "$LOG_DIR"

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    pkill -f "orchestrator|producer|webserver" 2>/dev/null || true
}
trap cleanup EXIT

# Start fresh
echo "🧹 Cleaning up existing processes..."
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true
sleep 1

# Note: trace_collector.log is not created unless tracing endpoint is configured
# This script only uses console debug logging, not file-based trace collection

# Start orchestrator with debug output to file
echo "🚀 Starting orchestrator with debug logging..."
cargo run --bin orchestrator -- --log-level debug > "$DEBUG_LOG" 2>&1 &
ORCHESTRATOR_PID=$!

# Wait for system to be ready
echo -n "⏳ Waiting for system to initialize"
MAX_WAIT=30
WAITED=0
while ! curl -s http://localhost:8080 >/dev/null 2>&1; do
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo ""
        echo "❌ Timeout waiting for orchestrator to start"
        echo "Last 20 lines of debug log:"
        tail -n 20 "$DEBUG_LOG"
        exit 1
    fi
    sleep 1
    WAITED=$((WAITED + 1))
    echo -n "."
done
echo " ✅ Ready!"

# Start topic generation
echo ""
echo "🎯 Starting topic generation..."
START_TIME=$(date +%s)
RESPONSE=$(curl -s -X POST http://localhost:8080/api/start \
    -H "Content-Type: application/json" \
    -d "{\"topic\": \"$TOPIC\", \"producer_count\": $PRODUCER_COUNT}")

if [[ $RESPONSE == *"success"* ]]; then
    echo "✅ Topic started successfully: $RESPONSE"
else
    echo "❌ Failed to start topic: $RESPONSE"
    exit 1
fi

# Monitor for specified duration
echo ""
echo "📊 Monitoring system for ${DURATION} seconds..."
echo -n "Progress: "
for i in $(seq 1 $DURATION); do
    sleep 1
    echo -n "."
done
echo " Done!"

END_TIME=$(date +%s)
TEST_DURATION=$((END_TIME - START_TIME))

# Generate analysis report
echo ""
echo "📋 Generating analysis report..."
{
    echo "LLM Orchestration System - Test Analysis Report"
    echo "=============================================="
    echo ""
    echo "Test Configuration:"
    echo "  - Topic: $TOPIC"
    echo "  - Producer Count: $PRODUCER_COUNT"
    echo "  - Test Duration: ${TEST_DURATION}s"
    echo "  - Timestamp: $(date)"
    echo ""
    
    echo "Process Status:"
    echo "---------------"
    ps aux | grep -E "(orchestrator|producer|webserver)" | grep -v grep | while read line; do
        echo "  $line"
    done
    echo ""
    
    echo "Error Summary:"
    echo "--------------"
    ERROR_COUNT=$(grep -c "ERROR\|error\|Failed to send update after" "$DEBUG_LOG" 2>/dev/null || echo "0")
    WARN_COUNT=$(grep -c "WARN\|warn" "$DEBUG_LOG" 2>/dev/null || echo "0")
    echo "  - Total Errors: $ERROR_COUNT"
    echo "  - Total Warnings: $WARN_COUNT"
    echo ""
    
    if [ "$ERROR_COUNT" -gt "0" ]; then
        echo "Error Details (first 10):"
        grep -E "ERROR|error|Failed to send update after" "$DEBUG_LOG" | head -n 10 | while read line; do
            echo "  $line"
        done
        echo ""
    fi
    
    echo "Producer Activity:"
    echo "-----------------"
    SPAWNED=$(grep -c "Spawned producer" "$DEBUG_LOG" 2>/dev/null || echo "0")
    READY=$(grep -c "Ready signal sent" "$DEBUG_LOG" 2>/dev/null || echo "0")
    FAILED=$(grep -c "Producer.*has failed" "$DEBUG_LOG" 2>/dev/null || echo "0")
    RESTARTED=$(grep -c "successfully restarted" "$DEBUG_LOG" 2>/dev/null || echo "0")
    echo "  - Producers Spawned: $SPAWNED"
    echo "  - Ready Signals: $READY"
    echo "  - Failures: $FAILED"
    echo "  - Restarts: $RESTARTED"
    echo ""
    
    echo "Attribute Generation:"
    echo "--------------------"
    BATCHES=$(grep -c "AttributeBatch" "$DEBUG_LOG" 2>/dev/null || echo "0")
    UNIQUE=$(grep "Total Unique Attributes:" "$DEBUG_LOG" | tail -n 1 | grep -o '[0-9]*' || echo "0")
    echo "  - Attribute Batches Sent: $BATCHES"
    echo "  - Total Unique Attributes: $UNIQUE"
    echo ""
    
    echo "Performance Metrics (last reported):"
    echo "-----------------------------------"
    grep -A 10 "Performance Metrics:" "$DEBUG_LOG" | tail -n 11 | while read line; do
        echo "  $line"
    done
    echo ""
    
    echo "Recent Activity (last 20 lines):"
    echo "--------------------------------"
    tail -n 20 "$DEBUG_LOG" | while read line; do
        echo "  $line"
    done
    
} > "$ANALYSIS_LOG"

# Display summary
echo ""
echo "✅ Test completed successfully!"
echo ""
echo "📊 Quick Summary:"
echo "  - Test Duration: ${TEST_DURATION}s"
echo "  - Errors Found: $(grep -c "ERROR\|error" "$DEBUG_LOG" 2>/dev/null || echo "0")"
echo "  - Producers Failed: $(grep -c "Producer.*has failed" "$DEBUG_LOG" 2>/dev/null || echo "0")"
echo "  - Attributes Generated: $(grep "Total Unique Attributes:" "$DEBUG_LOG" | tail -n 1 | grep -o '[0-9]*' || echo "0")"
echo ""
echo "📁 Output Files:"
echo "  - Debug Log: $DEBUG_LOG"
echo "  - Analysis Report: $ANALYSIS_LOG"
echo "  - Trace collection: Not enabled (console logging only)"
echo ""

# Option to view analysis
echo -n "Would you like to view the analysis report? (y/n): "
read -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cat "$ANALYSIS_LOG"
fi