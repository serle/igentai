#!/bin/bash

# Build script for the LLM Orchestration System
# Builds all components in release mode for production deployment

set -e  # Exit on any error

echo "🔨 Building LLM Orchestration System (Release Mode)"
echo "=================================================="

# Change to the project root directory (script is in scripts/ subdirectory)
cd "$(dirname "$0")/.."

echo "📦 Building all workspace members..."
echo ""

# Build all binaries in release mode
echo "Building orchestrator..."
cargo build --release --bin orchestrator

echo "Building producer..."
cargo build --release --bin producer

echo "Building webserver..."
cargo build --release --bin webserver

echo "Building tester..."
cargo build --release --bin tester

echo ""
echo "✅ Build completed successfully!"
echo ""
echo "Binaries available at:"
echo "  ./target/release/orchestrator"
echo "  ./target/release/producer"
echo "  ./target/release/webserver"
echo "  ./target/release/tester"
echo ""
echo "To start the system:"
echo "  ./scripts/start.sh"