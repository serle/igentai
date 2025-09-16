//! WebServer child process entry point
//!
//! Started by orchestrator's ProcessManager with specific command line arguments
//! Aligns with ProcessManager::spawn_webserver() expectations

use clap::Parser;
use shared::{ProcessId, logging, process_info};
use std::net::SocketAddr;
use tokio::signal;

use webserver::{
    WebServer, WebServerResult,
    core::{AnalyticsEngine, WebServerState},
    services::{RealOrchestratorClient, RealStaticFileServer, RealWebSocketManager},
};

/// Command line arguments expected from ProcessManager
#[derive(Parser, Debug)]
#[command(name = "webserver")]
#[command(about = "WebServer process spawned by orchestrator")]
struct Args {
    /// Port for HTTP server (browser connections)
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Tracing endpoint URL (if set, traces will be sent here)
    #[arg(long)]
    trace_ep: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Listen port for receiving orchestrator updates (IPC communication, like producers)
    #[arg(long)]
    listen_port: Option<u16>,

    /// Static files directory
    #[arg(long, default_value = "./static")]
    static_dir: String,

    /// Orchestrator address for IPC (if not provided, runs in standalone mode)
    #[arg(long)]
    orchestrator_addr: Option<String>,
}

#[tokio::main]
async fn main() -> WebServerResult<()> {
    // Parse arguments from ProcessManager spawn
    let args = Args::parse();

    // Initialize process ID singleton for webserver
    ProcessId::init_webserver();

    // Initialize tracing with optional endpoint and log level
    let trace_endpoint = args
        .trace_ep
        .as_ref()
        .map(|url| shared::logging::TracingEndpoint::new(url.clone()));
    shared::logging::init_tracing_with_endpoint_and_level(trace_endpoint, Some(&args.log_level));

    // Test that ProcessId is working
    process_info!(ProcessId::current(), "🚀 WebServer ProcessId initialized successfully");

    if let Some(listen_port) = args.listen_port {
        process_info!(
            ProcessId::current(),
            "🌐 WebServer starting on HTTP port {} (IPC listen: {})",
            args.port,
            listen_port
        );
    } else {
        process_info!(
            ProcessId::current(),
            "🌐 WebServer starting on HTTP port {} (standalone mode)",
            args.port
        );
    }

    // Create service addresses matching ProcessManager expectations
    let http_addr: SocketAddr = format!("127.0.0.1:{}", args.port)
        .parse()
        .map_err(|e| webserver::WebServerError::config(format!("Invalid port: {}", e)))?;

    // Determine if running in standalone mode
    let standalone_mode = args.orchestrator_addr.is_none();

    // Initialize services with dependency injection
    let orchestrator_client = if standalone_mode {
        process_info!(
            ProcessId::current(),
            "🔧 Starting in standalone mode (no orchestrator connection)"
        );
        RealOrchestratorClient::new_standalone()
    } else {
        let orchestrator_addr: SocketAddr = args
            .orchestrator_addr
            .unwrap()
            .parse()
            .map_err(|e| webserver::WebServerError::config(format!("Invalid orchestrator address: {}", e)))?;

        if let Some(listen_port) = args.listen_port {
            // IPC mode with orchestrator (standardized like producers)
            let listen_addr: SocketAddr = format!("127.0.0.1:{}", listen_port)
                .parse()
                .map_err(|e| webserver::WebServerError::config(format!("Invalid listen port: {}", e)))?;
            
            process_info!(
                ProcessId::current(),
                "🔗 Starting with orchestrator IPC connection: listen on {}, connect to {}",
                listen_addr,
                orchestrator_addr
            );
            RealOrchestratorClient::new(listen_addr, orchestrator_addr)
        } else {
            return Err(webserver::WebServerError::config(
                "Listen port required when orchestrator address is provided. Use --listen-port.".to_string()
            ));
        }
    };

    let websocket_manager = RealWebSocketManager::new();
    let static_server = RealStaticFileServer::new(args.static_dir);

    // Initialize core business logic
    let state = WebServerState::new();
    let analytics = AnalyticsEngine::new();

    // Create webserver with injected dependencies
    let mut webserver = WebServer::new(state, analytics, orchestrator_client, websocket_manager, static_server);

    // Set up graceful shutdown
    let shutdown_sender = webserver.get_shutdown_sender();
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

    // Start webserver (standalone mode detected automatically)
    webserver.run(http_addr, standalone_mode).await?;

    logging::log_success(ProcessId::current(), "WebServer stopped gracefully");
    Ok(())
}
