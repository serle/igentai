//! Producer binary entry point

use std::collections::HashMap;
use std::env;
use clap::Parser;
use shared::{ProviderId, ProcessId, logging, process_info, process_error, process_warn, process_debug};
use producer::{Producer, ProducerConfig, RealApiClient, RealCommunicator};
use producer::types::ExecutionConfig;

#[derive(Parser)]
#[command(name = "producer")]
#[command(about = "Producer service for generating unique attributes")]
struct Args {
    /// Producer ID number (defaults to 0 for testing, set by orchestrator in production)
    #[arg(long, default_value = "0")]
    id: u32,
    
    /// Tracing endpoint URL (if set, traces will be sent here)
    #[arg(long)]
    trace_ep: Option<String>,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
    
    /// Orchestrator address for IPC communication (if not provided, runs in standalone mode)
    #[arg(long)]
    orchestrator_addr: Option<String>,
    
    /// Producer listen port (for receiving commands from orchestrator)
    #[arg(long)]
    listen_port: Option<u16>,
    
    /// CLI mode: Topic for generation (when provided, starts standalone CLI mode)
    #[arg(long)]
    topic: Option<String>,
    
    /// CLI mode: Provider selection (test uses Random provider, env uses environment API keys)
    #[arg(long, default_value = "env")]
    provider: String,
    
    /// CLI mode: Request size (number of items to request per API call)
    #[arg(long, default_value = "60")]
    request_size: usize,
    
    /// Test mode: Model to use (default: gpt-4o-mini)
    #[arg(long, default_value = "gpt-4o-mini")]
    model: String,
    
    /// Test mode: Request delay in seconds between API calls
    #[arg(long, default_value = "2")]
    request_delay: u64,
    
    /// Request timeout in milliseconds
    #[arg(long, default_value = "30000")]
    timeout_ms: u64,
    
    /// Maximum concurrent requests (production mode)
    #[arg(long, default_value = "10")]
    max_concurrent: usize,
    
    /// OpenAI API key (can also be set via OPENAI_API_KEY env var or .env file)
    #[arg(long)]
    openai_key: Option<String>,
    
    /// Anthropic API key (can also be set via ANTHROPIC_API_KEY env var)
    #[arg(long)]
    anthropic_key: Option<String>,
    
    /// Gemini API key (can also be set via GEMINI_API_KEY env var)
    #[arg(long)]
    gemini_key: Option<String>,
    
    /// Random API key (not required - Random provider works without API key)
    #[arg(long)]
    random_key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize process ID with the provided ID
    ProcessId::init_producer(args.id);
    
    // Initialize tracing with optional endpoint and log level
    let trace_endpoint = args.trace_ep.as_ref().map(|url| shared::logging::TracingEndpoint::new(url.clone()));
    shared::logging::init_tracing_with_endpoint_and_level(trace_endpoint, Some(&args.log_level));
    
    // Determine operating mode based on orchestrator address availability (like webserver)
    let standalone_mode = args.orchestrator_addr.is_none();
    let cli_mode = args.topic.is_some() || standalone_mode; // CLI mode includes standalone mode
    let use_test_provider = cli_mode && args.provider == "test";
    
    // Create producer configuration
    let topic = if args.topic.is_some() {
        args.topic.as_ref().unwrap().clone()
    } else if standalone_mode {
        "Standalone test topic".to_string()
    } else {
        "Production generation topic".to_string()
    };
    
    let orchestrator_addr = if standalone_mode {
        "127.0.0.1:0".parse().unwrap() // Dummy address for standalone mode
    } else {
        args.orchestrator_addr.as_ref().unwrap().parse()
            .map_err(|e| format!("Invalid orchestrator address: {}", e))?
    };
    
    let mut config = ProducerConfig::new(orchestrator_addr, topic.clone());
    
    if standalone_mode {
        process_info!(ProcessId::current(), "🔧 Starting producer in standalone mode (no orchestrator connection)");
        process_info!(ProcessId::current(), "Topic: {}, Provider: {}, Request Size: {}", 
            topic,
            if use_test_provider { "test (Random)" } else { "env (API keys)" },
            args.request_size);
    } else if cli_mode {
        process_info!(ProcessId::current(), "🖥️  Starting producer in CLI mode");
        process_info!(ProcessId::current(), "Topic: {}, Provider: {}, Request Size: {}", 
            args.topic.as_ref().unwrap(),
            if use_test_provider { "test (Random)" } else { "env (API keys)" },
            args.request_size);
    } else {
        logging::log_startup(ProcessId::current(), "producer service");
        process_info!(ProcessId::current(), "Orchestrator: {}", args.orchestrator_addr.as_ref().unwrap());
    }
    
    // Try to load .env file if it exists (for test mode convenience)
    if let Ok(_) = dotenvy::dotenv() {
        process_debug!(ProcessId::current(), "📄 Loaded .env file from current directory");
    } else if let Ok(_) = dotenvy::from_path("../.env") {
        process_debug!(ProcessId::current(), "📄 Loaded .env file from parent directory");
    } else {
        process_debug!(ProcessId::current(), "📄 No .env file found - using environment variables");
    }
    
    // Collect API keys from args and environment
    let mut api_keys = HashMap::new();
    
    // For CLI mode with test provider, use Random provider, otherwise collect API keys
    if use_test_provider {
        // Use Random provider for keyless testing
        process_debug!(ProcessId::current(), "🧪 Test provider mode: using Random provider (no API key needed)");
        api_keys.insert(ProviderId::Random, "dummy".to_string());
        process_debug!(ProcessId::current(), "Random provider configured for test mode (no API key needed)");
    } else {
        // Production/CLI mode with env provider - collect all available keys
        if let Some(key) = args.openai_key.or_else(|| env::var("OPENAI_API_KEY").ok()) {
            api_keys.insert(ProviderId::OpenAI, key);
            process_debug!(ProcessId::current(), "OpenAI API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ OpenAI API key not provided");
        }
        
        if let Some(key) = args.anthropic_key.or_else(|| env::var("ANTHROPIC_API_KEY").ok()) {
            api_keys.insert(ProviderId::Anthropic, key);
            process_debug!(ProcessId::current(), "Anthropic API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ Anthropic API key not provided");
        }
        
        if let Some(key) = args.gemini_key.or_else(|| env::var("GEMINI_API_KEY").ok()) {
            api_keys.insert(ProviderId::Gemini, key);
            process_debug!(ProcessId::current(), "Gemini API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ Gemini API key not provided");
        }
        
        // Random provider (always available as fallback)
        if let Some(key) = args.random_key.or_else(|| env::var("RANDOM_API_KEY").ok()) {
            api_keys.insert(ProviderId::Random, key);
            process_debug!(ProcessId::current(), "Random provider configured (for testing/fallback)");
        } else {
            // Random provider works without API key
            api_keys.insert(ProviderId::Random, "dummy".to_string());
            process_debug!(ProcessId::current(), "Random provider available (keyless)");
        }
        
        // In production mode, ensure we have at least one real API key (excluding Random)
        let real_providers: Vec<_> = api_keys.keys()
            .filter(|&&p| p != ProviderId::Random)
            .collect();
        if real_providers.is_empty() && !cli_mode {
            process_warn!(ProcessId::current(), "⚠️ No real API providers configured. Only Random provider available.");
            process_warn!(ProcessId::current(), "   Consider setting at least one of: OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY");
        }
    }
    
    // Update the configuration with collected API keys and CLI args
    config.api_keys = api_keys;
    config.max_concurrent_requests = args.max_concurrent;
    config.request_timeout_ms = args.timeout_ms;
    config.request_size = args.request_size;
    
    // Create services
    let api_client = RealApiClient::new(config.api_keys.clone(), args.timeout_ms);
    let communicator = if standalone_mode {
        RealCommunicator::new_standalone(ProcessId::current().clone())
    } else if let Some(port) = args.listen_port {
        RealCommunicator::with_listen_port(orchestrator_addr, port, ProcessId::current().clone())
    } else {
        RealCommunicator::new(orchestrator_addr, ProcessId::current().clone())
    };
    
    // Create execution configuration
    let orchestrator_endpoint = if standalone_mode { 
        None 
    } else { 
        args.orchestrator_addr.clone() 
    };
    let execution_config = ExecutionConfig::from_args_and_env(
        orchestrator_endpoint,
        config.topic.clone(),
        Some(2), // 2 second interval
        if standalone_mode { Some(50) } else { None }, // Only limit iterations in standalone mode
    ).map_err(|e| format!("Failed to create execution config: {}", e))?;
    
    // Create producer
    let mut producer = Producer::new(execution_config, api_client, communicator);
    
    // Set up signal handling for graceful shutdown
    let shutdown_sender = producer.shutdown_sender();
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                logging::log_shutdown(ProcessId::current(), "Received Ctrl+C signal");
                if shutdown_sender.send(()).await.is_err() {
                    process_error!(ProcessId::current(), "Failed to send shutdown signal");
                }
            }
            Err(err) => {
                logging::log_error(ProcessId::current(), "Signal handling", &err);
            }
        }
    });
    
    // Run producer
    match producer.run().await {
        Ok(()) => {
            logging::log_success(ProcessId::current(), "Producer completed successfully");
            Ok(())
        }
        Err(e) => {
            logging::log_error(ProcessId::current(), "Producer execution", &e);
            Err(e.into())
        }
    }
}