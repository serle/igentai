#!/bin/bash

# Test script for orchestrator system
# Usage: ./test.sh <topic> [workers] [iterations]

# Change to the project root directory (script is in scripts/ subdirectory)
cd "$(dirname "$0")/.."

# Clear any inherited environment variables that might conflict with .env
unset ROUTING_STRATEGY ROUTING_CONFIG 2>/dev/null || true

# Load environment variables from .env file if it exists
if [ -f ".env" ]; then
    echo "📋 Loading environment from .env file..."
    set -a  # Export all variables
    source .env
    set +a  # Stop exporting
else
    echo "⚠️  Warning: .env file not found - using system environment variables only"
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
DEFAULT_WORKERS=3
DEFAULT_ITERATIONS=5

# Parse arguments
TOPIC=${1:-"Paris attractions"}
WORKERS=${2:-$DEFAULT_WORKERS}
ITERATIONS=${3:-$DEFAULT_ITERATIONS}

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Function to kill existing processes
cleanup_processes() {
    print_info "Cleaning up existing processes..."
    
    # Kill orchestrator
    if pgrep -f "orchestrator.*--provider" > /dev/null; then
        print_warning "Killing existing orchestrator process..."
        pkill -f "orchestrator.*--provider"
        sleep 1
    fi
    
    # Kill producers
    if pgrep -f "producer" > /dev/null; then
        print_warning "Killing existing producer processes..."
        pkill -f "producer"
        sleep 1
    fi
    
    # Kill webserver
    if pgrep -f "webserver" > /dev/null; then
        print_warning "Killing existing webserver process..."
        pkill -f "webserver"
        sleep 1
    fi
    
    print_success "Cleanup complete"
}

# Function to wait for webserver to be ready
wait_for_webserver() {
    print_info "Waiting for webserver to be ready..."
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -s http://localhost:8080/test > /dev/null 2>&1; then
            print_success "Webserver is ready!"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
        echo -n "."
    done
    
    print_error "Webserver failed to start within 30 seconds"
    return 1
}

# Function to start generation
start_generation() {
    print_info "Starting generation with:"
    echo "  Topic: $TOPIC"
    echo "  Workers: $WORKERS"
    echo "  Iterations: $ITERATIONS"
    
    local response=$(curl -s -X POST http://localhost:8080/api/start \
        -H "Content-Type: application/json" \
        -d "{\"topic\": \"$TOPIC\", \"producer_count\": $WORKERS, \"iterations\": $ITERATIONS, \"routing_strategy\": \"backoff\", \"routing_config\": \"openai:gpt-4o-mini\"}")
    
    if echo "$response" | grep -q "success"; then
        print_success "Generation started successfully"
        echo "Response: $response"
        return 0
    else
        print_error "Failed to start generation"
        echo "Response: $response"
        return 1
    fi
}

# Function to monitor generation progress
monitor_generation() {
    print_info "Monitoring generation progress (press Ctrl+C to stop)..."
    print_info "You can also view logs in real-time: tail -f $LOG_FILE"
    
    # Trap Ctrl+C to stop generation gracefully
    trap 'cleanup_and_exit' INT
    
    # Keep the script running to see logs
    tail -f /dev/null
}

# Function to cleanup and exit
cleanup_and_exit() {
    echo
    print_info "Shutting down..."
    stop_generation
    
    # Kill orchestrator process
    if [ ! -z "$ORCHESTRATOR_PID" ]; then
        kill $ORCHESTRATOR_PID 2>/dev/null
        wait $ORCHESTRATOR_PID 2>/dev/null
    fi
    
    # Clean up any remaining processes
    cleanup_processes
    
    print_success "Shutdown complete. Logs saved to: $LOG_FILE"
    exit 0
}

# Function to stop generation
stop_generation() {
    print_info "Stopping generation..."
    
    local response=$(curl -s -X POST http://localhost:8080/api/stop)
    
    if echo "$response" | grep -q "success"; then
        print_success "Generation stopped successfully"
    else
        print_warning "Failed to stop generation gracefully"
    fi
}

# Main execution
main() {
    echo "======================================"
    echo "Orchestrator Test Script"
    echo "======================================"
    echo
    
    # Check if cargo is available
    if ! command -v cargo &> /dev/null; then
        print_error "cargo not found. Please install Rust and Cargo."
        exit 1
    fi
    
    # Clean up existing processes
    cleanup_processes
    
    # Build the project
    print_info "Building the project..."
    if ! cargo build 2>&1 | tail -20; then
        print_error "Build failed"
        exit 1
    fi
    print_success "Build complete"
    
    # Start orchestrator
    print_info "Starting orchestrator with backoff routing to OpenAI GPT-4o-mini and debug logging..."
    mkdir -p logs
    LOG_FILE="logs/orchestrator_$(date +%Y%m%d_%H%M%S).log"
    print_info "Logs will be saved to: $LOG_FILE"
    
    cargo run --bin orchestrator -- --routing-strategy backoff --routing-config "openai:gpt-4o-mini" --log-level debug 2>&1 | tee "$LOG_FILE" &
    ORCHESTRATOR_PID=$!
    
    # Wait for webserver to be ready
    if ! wait_for_webserver; then
        print_error "Failed to start system"
        kill $ORCHESTRATOR_PID 2>/dev/null
        exit 1
    fi
    
    # Give it a moment to fully initialize
    sleep 2
    
    # Start generation
    if ! start_generation; then
        print_error "Failed to start generation"
        kill $ORCHESTRATOR_PID 2>/dev/null
        exit 1
    fi
    
    # Open browser if available
    if command -v open &> /dev/null; then
        print_info "Opening dashboard in browser..."
        open http://localhost:8080
    elif command -v xdg-open &> /dev/null; then
        print_info "Opening dashboard in browser..."
        xdg-open http://localhost:8080
    else
        print_info "Dashboard available at: http://localhost:8080"
    fi
    
    echo
    print_success "System is running!"
    echo "======================================"
    echo "Dashboard: http://localhost:8080"
    echo "Topic: $TOPIC"
    echo "Workers: $WORKERS"
    echo "Iterations: $ITERATIONS"
    echo "Example: Eiffel Tower, Louvre Museum, Notre-Dame Cathedral..."
    echo "Log file: $LOG_FILE"
    echo "======================================"
    echo
    echo "What you should see:"
    echo "• Dashboard opens in your browser"
    echo "• Attributes appear in the UI as they're generated"
    echo "• Chart.js bar graph shows batch sizes"
    echo "• Generation will stop after $ITERATIONS iterations"
    echo
    echo "Commands:"
    echo "• Press Ctrl+C to stop early"
    echo "• View logs: tail -f $LOG_FILE"
    echo
    print_info "Monitoring started..."
    echo
    
    # Monitor generation
    monitor_generation
}

# Run main function
main