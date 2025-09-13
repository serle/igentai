//! Orchestrator main entry point

use clap::Parser;
use std::time::Duration;

mod error;
mod state;
mod traits;
mod services;
mod orchestrator_impl;

use error::OrchestratorResult;
use orchestrator_impl::Orchestrator;
use services::{RealApiKeySource, RealFileSystem, RealProcessManager, RealMessageTransport};

#[derive(Parser)]
#[command(name = "orchestrator")]
struct Args {
    #[arg(short = 'n', long, default_value = "5")]
    producers: u32,
    
    #[arg(short = 'p', long, default_value = "8080")]
    port: u16,
}

/// Type alias for production orchestrator 
pub type ProductionOrchestrator = Orchestrator<RealApiKeySource, RealFileSystem, RealProcessManager, RealMessageTransport>;

impl ProductionOrchestrator {
    /// Create production orchestrator with real dependencies
    pub fn new_production(producer_count: u32, webserver_port: u16) -> Self {
        Self::new(
            producer_count,
            webserver_port,
            RealApiKeySource,
            RealFileSystem::new(),
            RealProcessManager::new(),
            RealMessageTransport::new(),
        )
    }
}

/// Type alias for test orchestrator
#[cfg(test)]
pub type TestOrchestrator = Orchestrator<
    traits::MockApiKeySource, 
    traits::MockFileSystem, 
    traits::MockProcessManager, 
    traits::MockMessageTransport
>;

#[cfg(test)]
impl TestOrchestrator {
    /// Create test orchestrator with all mock dependencies
    pub fn new_test(producer_count: u32, webserver_port: u16) -> Self {
        Self::new(
            producer_count,
            webserver_port,
            traits::MockApiKeySource::new(),
            traits::MockFileSystem::new(),
            traits::MockProcessManager::new(),
            traits::MockMessageTransport::new(),
        )
    }
}

#[tokio::main]
async fn main() -> OrchestratorResult<()> {
    let args = Args::parse();
    
    // Create production orchestrator with real dependencies
    let mut orchestrator = ProductionOrchestrator::new_production(args.producers, args.port);
    
    // Demo workflow with IPC coordination
    orchestrator.start_producers("coordinated_demo".to_string()).await?;
    orchestrator.start_file_sync().await?;
    
    // Run orchestration cycles
    for i in 0..5 {
        println!("Orchestrator cycle {} with process monitoring", i + 1);
        orchestrator.next().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    orchestrator.stop_file_sync().await?;
    orchestrator.stop_producers().await?;
    
    println!("Coordinated demo completed");
    Ok(())
}