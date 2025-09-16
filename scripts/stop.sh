#!/bin/bash

# Stop script for the LLM Orchestration System
# This script gracefully stops all orchestrator, producer, and webserver processes

echo "🛑 Stopping LLM Orchestration System..."

# Find and list running processes
PROCESSES=$(ps aux | grep -E "(orchestrator|producer|webserver)" | grep -v grep | grep -v stop.sh || true)

if [ -z "$PROCESSES" ]; then
    echo "✅ No running processes found"
    exit 0
fi

echo "Found running processes:"
echo "$PROCESSES"
echo ""

# Kill the processes
echo "Stopping processes..."
pkill -f "orchestrator|producer|webserver" 2>/dev/null || true

# Wait for processes to terminate
sleep 2

# Check if any processes are still running
REMAINING=$(ps aux | grep -E "(orchestrator|producer|webserver)" | grep -v grep | grep -v stop.sh || true)

if [ -z "$REMAINING" ]; then
    echo "✅ All processes stopped successfully"
else
    echo "⚠️  Some processes may still be running:"
    echo "$REMAINING"
    echo ""
    echo "If needed, force kill with: pkill -9 -f \"orchestrator|producer|webserver\""
fi