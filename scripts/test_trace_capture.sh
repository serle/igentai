#!/bin/bash
set -e

echo "🧪 Testing trace capture with tester module"

# Clean up any previous test outputs
rm -rf ./output/trace_test

# Build the project first
echo "🔨 Building project..."
cargo build --workspace

# Run the tester with a simple scenario
echo "🚀 Running tester with trace collection..."
RUST_LOG=debug cargo run -p tester -- --scenarios basic --max-duration 30

echo "✅ Test complete"