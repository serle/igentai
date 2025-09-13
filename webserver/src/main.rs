//! Webserver binary
//!
//! This is the main entry point for the webserver application.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use clap::Parser;
use tracing::{info, error};
use tracing_subscriber;

use webserver::{
    webserver_impl::WebServer,
    services::{
        RealFileManager, RealClientBroadcaster, RealIpcCommunicator,
        RealClientRegistry, RealMetricsAggregator,
    },
    state::WebServerState,
};

#[derive(Parser, Debug)]
#[command(name = "webserver")]
#[command(about = "Web dashboard server for the LLM orchestrator")]
struct Args {
    /// Bind address for the web server
    #[arg(short, long, default_value = "127.0.0.1")]
    host: String,

    /// Port for the web server
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// Orchestrator address
    #[arg(long, default_value = "127.0.0.1")]
    orchestrator_host: String,

    /// Orchestrator port
    #[arg(long, default_value_t = 8080)]
    orchestrator_port: u16,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing
    let subscriber = if args.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .finish()
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .finish()
    };
    tracing::subscriber::set_global_default(subscriber)?;

    // Parse addresses
    let bind_addr = SocketAddr::new(
        args.host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        args.port,
    );

    let orchestrator_addr = SocketAddr::new(
        args.orchestrator_host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        args.orchestrator_port,
    );

    info!("🚀 Starting webserver on {}", bind_addr);
    info!("🎯 Will connect to orchestrator at {}", orchestrator_addr);

    // Create state
    let state = std::sync::Arc::new(WebServerState::new(bind_addr, orchestrator_addr));

    // Create service implementations
    let file_manager = RealFileManager::new();
    let client_broadcaster = RealClientBroadcaster::new(state.clone());
    let ipc_communicator = RealIpcCommunicator::new(state.clone());
    let client_registry = RealClientRegistry::new(state.clone());
    let metrics_aggregator = RealMetricsAggregator::new(state.clone());

    // Create webserver with dependency injection
    let webserver = WebServer::new(
        bind_addr,
        orchestrator_addr,
        file_manager,
        client_broadcaster,
        ipc_communicator,
        client_registry,
        metrics_aggregator,
    );

    // Start the server
    if let Err(e) = webserver.run().await {
        error!("❌ Server error: {}", e);
        return Err(e.into());
    }

    info!("👋 Webserver shut down gracefully");
    Ok(())
}