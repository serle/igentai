//! E2E Test Runner
//!
//! Simplified end-to-end testing framework that:
//! - Starts orchestrator in CLI or WebServer mode
//! - Sets up tracing collector to capture all trace events
//! - Provides Topic-centric API for clean test assertions
//! - Manages service lifecycle and cleanup

use clap::Parser;
use std::time::Duration;
use tokio::time::timeout;

use tester::{ServiceConstellation, TestScenarios, TracingCollector};

#[derive(Parser)]
#[command(name = "tester")]
#[command(about = "E2E testing framework for the distributed system")]
struct Args {
    /// Test scenario to run
    #[arg(long, default_value = "basic")]
    scenario: String,

    /// Test timeout in seconds
    #[arg(long, default_value = "30")]
    timeout_secs: u64,

    /// Tracing collector port
    #[arg(long, default_value = "9999")]
    collector_port: u16,

    /// Keep services running after test completion (for debugging)
    #[arg(long)]
    keep_running: bool,

    /// Enable verbose tracing output
    #[arg(long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing for the tester itself
    init_tester_tracing(args.verbose);

    tracing::info!("🧪 Starting E2E test framework");
    tracing::info!("Scenario: {}, Timeout: {}s", args.scenario, args.timeout_secs);

    // Start the tracing collector first
    let collector = TracingCollector::new(args.collector_port).await?;
    let trace_endpoint = format!("http://127.0.0.1:{}/traces", args.collector_port);

    tracing::info!("📡 Tracing collector started on port {}", args.collector_port);

    // Create service constellation with tracing endpoint
    let mut constellation = ServiceConstellation::new(trace_endpoint);

    // Create test scenarios manager
    let scenarios = TestScenarios::new(collector.clone());

    let test_result = timeout(
        Duration::from_secs(args.timeout_secs),
        run_test_scenario(&mut constellation, &scenarios, &args.scenario),
    )
    .await;

    match test_result {
        Ok(Ok(())) => {
            tracing::info!("✅ Test scenario '{}' completed successfully", args.scenario);

            if args.keep_running {
                tracing::info!("🔄 Keeping services running (--keep-running flag set)");
                tracing::info!("Press Ctrl+C to stop all services");
                tokio::signal::ctrl_c().await?;
            }
        }
        Ok(Err(e)) => {
            tracing::error!("❌ Test scenario '{}' failed: {}", args.scenario, e);
            return Err(e);
        }
        Err(_) => {
            tracing::error!(
                "⏰ Test scenario '{}' timed out after {}s",
                args.scenario,
                args.timeout_secs
            );
            return Err("Test timeout".into());
        }
    }

    // Shutdown services
    tracing::info!("🛑 Shutting down service constellation");
    constellation.shutdown().await?;

    tracing::info!("🏁 E2E testing completed");
    Ok(())
}

async fn run_test_scenario(
    constellation: &mut ServiceConstellation,
    scenarios: &TestScenarios,
    scenario_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    scenarios.run_scenario(scenario_name, constellation).await
}

fn init_tester_tracing(verbose: bool) {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = if verbose {
        EnvFilter::new("tester=debug,info")
    } else {
        EnvFilter::new("tester=info")
    };

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}
