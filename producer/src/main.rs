//! Producer binary entry point

use std::env;
use std::net::SocketAddr;
use clap::Parser;

use shared::ProducerId;
use producer::{
    Producer, ProducerResult,
    services::{
        RealIpcCommunicator, RealProviderRouter, RealResponseProcessor,
        RealPerformanceTracker
    }
};

#[derive(Parser)]
#[command(name = "producer")]
#[command(about = "LLM Producer for generating unique attributes")]
struct Args {
    /// Orchestrator address
    #[arg(long, default_value = "127.0.0.1:8080")]
    orchestrator_addr: SocketAddr,
    
    /// Producer ID (optional, generates random if not provided)
    #[arg(long)]
    producer_id: Option<String>,
}

#[tokio::main]
async fn main() -> ProducerResult<()> {
    let args = Args::parse();
    
    println!("Starting producer with orchestrator at {}", args.orchestrator_addr);
    
    // Generate or use provided producer ID
    let producer_id = match args.producer_id {
        Some(id) => ProducerId::from_string(&id).map_err(|e| 
            producer::ProducerError::ConfigError { 
                message: format!("Invalid producer ID: {}", e) 
            })?,
        None => ProducerId::new(),
    };
    
    println!("Producer ID: {}", producer_id);
    
    // Get API keys from environment
    let mut api_keys = std::collections::HashMap::new();
    
    // Check for OpenAI API key (primary provider)
    if let Ok(openai_key) = env::var("OPENAI_API_KEY") {
        api_keys.insert("OPENAI_API_KEY".to_string(), openai_key);
        println!("Found OPENAI_API_KEY");
    }
    
    // Check for Anthropic API key
    if let Ok(anthropic_key) = env::var("ANTHROPIC_API_KEY") {
        api_keys.insert("ANTHROPIC_API_KEY".to_string(), anthropic_key);
        println!("Found ANTHROPIC_API_KEY");
    }
    
    // Check for Gemini API keys (Google has multiple key names)
    if let Ok(gemini_key) = env::var("GOOGLE_API_KEY") {
        api_keys.insert("GOOGLE_API_KEY".to_string(), gemini_key);
        println!("Found GOOGLE_API_KEY");
    } else if let Ok(gemini_key) = env::var("GOOGLE_AI_API_KEY") {
        api_keys.insert("GOOGLE_AI_API_KEY".to_string(), gemini_key);
        println!("Found GOOGLE_AI_API_KEY");
    }
    
    // Ensure at least one API key is available
    if api_keys.is_empty() {
        eprintln!("Error: At least one API key must be set (OPENAI_API_KEY, ANTHROPIC_API_KEY, or GOOGLE_API_KEY/GOOGLE_AI_API_KEY)");
        std::process::exit(1);
    }
    
    println!("Found {} API key(s) total", api_keys.len());
    
    // Create service implementations
    let ipc_communicator = RealIpcCommunicator::new();
    let provider_router = RealProviderRouter::new();
    let response_processor = RealResponseProcessor::new();
    let performance_tracker = RealPerformanceTracker::new();
    
    // Create producer with dependency injection
    let producer = Producer::new(
        producer_id,
        args.orchestrator_addr,
        ipc_communicator,
        provider_router,
        response_processor,
        performance_tracker,
    );
    
    // Set API keys
    producer.set_api_keys(api_keys).await?;
    
    // Start producer
    println!("Starting producer main loop...");
    producer.start().await?;
    
    println!("Producer shutting down");
    Ok(())
}