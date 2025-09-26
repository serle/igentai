#!/bin/bash

# Simple test script that outputs debug traces directly to console
# Usage: ./test_debug_console.sh [topic] [producer_count]

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 LLM Orchestration System - Console Debug Test${NC}"
echo "=============================================="
echo ""

# Configuration
TOPIC="${1:-test topic}"
PRODUCER_COUNT="${2:-3}"

echo -e "${YELLOW}📝 Configuration:${NC}"
echo "   - Topic: $TOPIC"
echo "   - Producers: $PRODUCER_COUNT"
echo ""

# Change to project root
cd "$(dirname "$0")/.."

# Create logs directory if it doesn't exist
LOG_DIR="logs"
mkdir -p "$LOG_DIR"

echo -e "${YELLOW}🧹 Cleaning up existing processes...${NC}"
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true
sleep 1

# Clear trace log
echo "" > "$LOG_DIR/trace_collector.log"

echo -e "${GREEN}🚀 Starting orchestrator with debug logging...${NC}"
echo "=============================================="
echo ""

# Start orchestrator in background but capture output
cargo run --bin orchestrator -- --log-level debug 2>&1 &
ORCHESTRATOR_PID=$!

echo "Orchestrator PID: $ORCHESTRATOR_PID"
echo "Waiting for system to initialize..."

# Wait for orchestrator to be ready
MAX_WAIT=30
WAITED=0
while ! curl -s http://localhost:8080 >/dev/null 2>&1; do
    if [ $WAITED -ge $MAX_WAIT ]; then
        echo -e "${RED}❌ Timeout waiting for orchestrator to start${NC}"
        exit 1
    fi
    sleep 1
    WAITED=$((WAITED + 1))
    echo -n "."
done
echo -e " ${GREEN}Ready!${NC}"

echo ""
echo -e "${YELLOW}🎯 Starting topic generation...${NC}"
RESPONSE=$(curl -s -X POST http://localhost:8080/api/start \
    -H "Content-Type: application/json" \
    -d "{\"topic\": \"$TOPIC\", \"producer_count\": $PRODUCER_COUNT}")

echo "API Response: $RESPONSE"
echo ""
echo -e "${BLUE}📊 Live System Output (Press Ctrl+C to stop):${NC}"
echo "=============================================="

# Tail the orchestrator output (it's running in background)
# This will show live debug traces
wait $ORCHESTRATOR_PID