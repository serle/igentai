//! Main entry point for the orchestrator binary
//!
//! This demonstrates how to use the orchestrator with real service implementations
//! and proper dependency injection.

use clap::Parser;
use std::net::SocketAddr;
use tokio::signal;

use orchestrator::{
    services::{RealApiKeySource, RealCommunicator, RealFileSystem, RealProcessManager},
    Orchestrator, OrchestratorResult,
};
use shared::{logging, process_debug, process_info, ProcessId};

/// Orchestrator for managing LLM-based unique attribute generation
#[derive(Parser)]
#[command(name = "orchestrator")]
#[command(about = "Orchestrates multiple producer processes for unique attribute generation")]
pub struct Args {
    /// Tracing endpoint URL (if set, all spawned processes will also trace here)
    #[arg(long)]
    pub trace_ep: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// CLI mode: Topic for generation (when provided, starts producers immediately without webserver)
    #[arg(long)]
    pub topic: Option<String>,

    /// CLI mode: Number of producers to spawn (only used with --topic)
    #[arg(long, default_value = "5")]
    pub producers: u32,

    /// CLI mode: Number of iterations to run (only used with --topic, runs indefinitely if not specified)
    #[arg(long)]
    pub iterations: Option<u32>,

    /// CLI mode: Provider selection (random uses Random provider, env uses environment API keys)
    #[arg(long, default_value = "env")]
    pub provider: String,

    /// CLI mode: Request size (number of items to request per API call)
    #[arg(long, default_value = "60")]
    pub request_size: usize,

    /// Output directory (relative or absolute path, defaults to ./output/<topic>)
    #[arg(long)]
    pub output: Option<String>,

    /// Webserver bind address
    #[arg(long)]
    pub webserver_addr: Option<String>,

    /// Producer communication bind address  
    #[arg(long)]
    pub producer_addr: Option<String>,
}

#[tokio::main]
async fn main() -> OrchestratorResult<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Determine operating mode
    let cli_mode = args.topic.is_some();
    let use_only_random = args.provider == "random";

    // Initialize process ID singleton for orchestrator
    ProcessId::init_orchestrator();

    // Initialize tracing with optional endpoint and log level
    let trace_endpoint = args
        .trace_ep
        .as_ref()
        .map(|url| shared::logging::TracingEndpoint::new(url.clone()));
    shared::logging::init_tracing_with_endpoint_and_level(trace_endpoint, Some(&args.log_level));

    if cli_mode {
        let topic = args.topic.as_ref().unwrap();
        let output_dir = args.output.clone().unwrap_or_else(|| format!("./output/{}", topic));

        process_info!(ProcessId::current(), "🖥️  Starting orchestrator in CLI mode");
        process_debug!(
            ProcessId::current(),
            "Topic: {}, Producers: {}, Iterations: {}",
            topic,
            args.producers,
            args.iterations
                .map(|i| i.to_string())
                .unwrap_or_else(|| "unlimited".to_string())
        );
        process_debug!(
            ProcessId::current(),
            "Provider: {}, Request Size: {}, Output: {}",
            if use_only_random {
                "random (Random)"
            } else {
                "env (API keys)"
            },
            args.request_size,
            output_dir
        );
    } else {
        logging::log_startup(ProcessId::current(), "orchestrator service (web mode)");
        if use_only_random {
            process_debug!(
                ProcessId::current(),
                "🎲 Random provider mode enabled - using dummy API keys"
            );
        }
    }

    // Initialize services
    let api_keys = if use_only_random {
        RealApiKeySource::random_only()
    } else {
        RealApiKeySource::new()
    };
    let communicator = RealCommunicator::new();

    // Configure output directory
    let file_system = if cli_mode {
        let output_dir = args.output.clone().unwrap_or_else(|| "./output".to_string());
        RealFileSystem::with_base_dir(std::path::PathBuf::from(output_dir))
    } else {
        RealFileSystem::new()
    };

    let process_manager = RealProcessManager::new()
        .with_trace_endpoint(args.trace_ep.clone())
        .with_log_level(args.log_level.clone());

    // Create orchestrator with dependency injection
    let mut orchestrator = Orchestrator::new(api_keys, communicator, file_system, process_manager);

    // Configure bind addresses
    let webserver_addr: SocketAddr = args
        .webserver_addr
        .as_ref()
        .unwrap_or(&"127.0.0.1:6000".to_string())
        .parse()
        .map_err(|e| orchestrator::OrchestratorError::config(format!("Invalid webserver address: {}", e)))?;
    let producer_addr: SocketAddr = args
        .producer_addr
        .as_ref()
        .unwrap_or(&"127.0.0.1:6001".to_string())
        .parse()
        .map_err(|e| orchestrator::OrchestratorError::config(format!("Invalid producer address: {}", e)))?;

    // Initialize orchestrator based on mode
    if cli_mode {
        // CLI mode: Initialize without webserver
        orchestrator.initialize_cli_mode(producer_addr).await?;

        // Start generation immediately with provided topic
        let topic = args.topic.unwrap();
        orchestrator
            .start_cli_generation(topic, args.producers, args.iterations, args.request_size)
            .await?;
    } else {
        // WebServer mode: Initialize with webserver
        orchestrator.initialize(webserver_addr, producer_addr).await?;
    }

    // Set up graceful shutdown
    let shutdown_sender = orchestrator.get_shutdown_sender();
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                logging::log_shutdown(ProcessId::current(), "Received Ctrl+C signal");
                let _ = shutdown_sender.send(()).await;
            }
            Err(err) => {
                logging::log_error(ProcessId::current(), "Signal handling", &err);
            }
        }
    });

    // Run main event loop
    orchestrator.run().await?;

    logging::log_success(ProcessId::current(), "Orchestrator stopped gracefully");
    Ok(())
}
