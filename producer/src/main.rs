//! Producer binary entry point

use clap::Parser;
use producer::types::ExecutionConfig;
use producer::{Producer, ProducerConfig, RealApiClient, RealCommunicator};
use shared::types::RoutingStrategy;
use shared::{logging, process_debug, process_error, process_info, process_warn, ProcessId, ProviderId};
use std::collections::HashMap;
use std::env;
use std::error::Error;

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

    /// CLI mode: Provider selection (random uses Random provider, env uses environment API keys)
    #[arg(long, default_value = "env")]
    provider: String,

    /// Routing configuration from orchestrator (format: "strategy:backoff,provider:openai")
    #[arg(long)]
    routing_config: Option<String>,

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

    /// Routing strategy: backoff, roundrobin, priority, weighted (default: backoff)
    #[arg(long, default_value = "backoff")]
    routing_strategy: String,

    /// Provider for backoff strategy (default: random for test, from env for production)
    #[arg(long)]
    routing_provider: Option<String>,

    /// Comma-separated list of providers for roundrobin/priority strategies
    #[arg(long)]
    routing_providers: Option<String>,

    /// Provider weights for weighted strategy (format: "openai:0.5,anthropic:0.3")
    #[arg(long)]
    routing_weights: Option<String>,

    /// Maximum requests for testing (limits how many generation cycles to run)
    #[arg(long)]
    max_requests: Option<u32>,
}

/// Parse routing configuration from orchestrator with models
fn parse_routing_config(routing_config: &str) -> Result<RoutingStrategy, String> {
    let parts: Vec<&str> = routing_config.split(',').collect();
    let mut strategy_type = None;
    let mut provider = None;
    let mut model = None;
    let mut providers = None;
    let mut weights = None;

    for part in parts {
        let kv: Vec<&str> = part.split(':').collect();
        if kv.len() < 2 {
            return Err(format!("Invalid routing config format: '{}'", part));
        }
        
        match kv[0] {
            "strategy" => strategy_type = Some(kv[1]),
            "provider" => provider = Some(kv[1]),
            "model" => model = Some(kv[1]),
            "providers" => providers = Some(kv[1]),
            "weights" => weights = Some(kv[1]),
            _ => return Err(format!("Unknown routing config key: '{}'", kv[0])),
        }
    }

    let strategy = match strategy_type {
        Some("backoff") => {
            let provider_id = provider
                .ok_or("backoff strategy requires provider")?
                .parse()
                .map_err(|e| format!("Invalid provider: {}", e))?;
            let model_name = model
                .ok_or("backoff strategy requires model")?
                .to_string();
            let provider_config = shared::types::ProviderConfig::new(provider_id, model_name);
            RoutingStrategy::Backoff { provider: provider_config }
        },
        Some("roundrobin") => {
            let provider_configs = parse_provider_config_list(
                providers.ok_or("roundrobin strategy requires providers")?
            )?;
            RoutingStrategy::RoundRobin { providers: provider_configs }
        },
        Some("priority") => {
            let provider_configs = parse_provider_config_list(
                providers.ok_or("priority strategy requires providers")?
            )?;
            RoutingStrategy::PriorityOrder { providers: provider_configs }
        },
        Some("weighted") => {
            let weight_pairs = parse_weighted_provider_configs(
                weights.ok_or("weighted strategy requires weights")?
            )?;
            RoutingStrategy::Weighted { weights: weight_pairs }
        },
        Some(s) => return Err(format!("Unknown strategy: '{}'", s)),
        None => return Err("Missing strategy in routing config".to_string()),
    };

    Ok(strategy)
}

/// Parse provider config list from "provider1:model1,provider2:model2" format
fn parse_provider_config_list(providers_str: &str) -> Result<Vec<shared::types::ProviderConfig>, String> {
    providers_str
        .split(',')
        .map(|p| {
            let parts: Vec<&str> = p.split(':').collect();
            if parts.len() != 2 {
                return Err(format!("Invalid provider:model format: '{}'", p));
            }
            let provider_id = parts[0].parse()
                .map_err(|e| format!("Invalid provider '{}': {}", parts[0], e))?;
            let model = parts[1].to_string();
            Ok(shared::types::ProviderConfig::new(provider_id, model))
        })
        .collect()
}

/// Parse weighted provider configs from "provider1:model1:weight1,provider2:model2:weight2" format
fn parse_weighted_provider_configs(weights_str: &str) -> Result<HashMap<shared::types::ProviderConfig, f32>, String> {
    weights_str
        .split(',')
        .map(|pair| {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() != 3 {
                return Err(format!("Invalid weight format: '{}', expected 'provider:model:weight'", pair));
            }
            let provider_id = parts[0].parse()
                .map_err(|e| format!("Invalid provider '{}': {}", parts[0], e))?;
            let model = parts[1].to_string();
            let weight: f32 = parts[2].parse()
                .map_err(|e| format!("Invalid weight '{}': {}", parts[2], e))?;
            let provider_config = shared::types::ProviderConfig::new(provider_id, model);
            Ok((provider_config, weight))
        })
        .collect()
}

/// Parse routing strategy from command-line arguments with env fallback
fn parse_routing_strategy(args: &Args, use_test_provider: bool) -> Result<RoutingStrategy, String> {
    // First check if we should use environment-based routing entirely
    let strategy_type = if args.routing_strategy == "backoff" && args.routing_provider.is_none() 
        && args.routing_providers.is_none() && args.routing_weights.is_none() {
        // No routing args provided, check environment
        env::var("ROUTING_STRATEGY").unwrap_or_else(|_| args.routing_strategy.clone())
    } else {
        args.routing_strategy.clone()
    };
    
    match strategy_type.to_lowercase().as_str() {
        "backoff" => {
            let provider_id = if let Some(ref provider_str) = args.routing_provider {
                // Command-line arg takes priority
                provider_str.parse()
                    .map_err(|e| format!("Invalid routing provider '{}': {}", provider_str, e))?
            } else if use_test_provider {
                // Test mode defaults to random
                ProviderId::Random
            } else {
                // Production mode: try to get from environment or default to random
                match env::var("ROUTING_PRIMARY_PROVIDER") {
                    Ok(p) => p.parse()
                        .map_err(|e| format!("Invalid ROUTING_PRIMARY_PROVIDER '{}': {}", p, e))?,
                    Err(_) => ProviderId::Random,
                }
            };
            let provider_config = shared::types::ProviderConfig::with_default_model(provider_id);
            Ok(RoutingStrategy::Backoff { provider: provider_config })
        }
        "roundrobin" => {
            let provider_ids = if args.routing_providers.is_some() {
                parse_provider_list(&args.routing_providers)?
            } else {
                // Try environment variable
                let env_providers = env::var("ROUTING_PROVIDERS").ok();
                parse_provider_list(&env_providers)?
            };
            if provider_ids.is_empty() {
                return Err("--routing-providers or ROUTING_PROVIDERS env must be specified for roundrobin strategy".to_string());
            }
            let providers = provider_ids.into_iter().map(|id| shared::types::ProviderConfig::with_default_model(id)).collect();
            Ok(RoutingStrategy::RoundRobin { providers })
        }
        "priority" => {
            let provider_ids = if args.routing_providers.is_some() {
                parse_provider_list(&args.routing_providers)?
            } else {
                // Try environment variable
                let env_providers = env::var("ROUTING_PROVIDERS").ok();
                parse_provider_list(&env_providers)?
            };
            if provider_ids.is_empty() {
                return Err("--routing-providers or ROUTING_PROVIDERS env must be specified for priority strategy".to_string());
            }
            let providers = provider_ids.into_iter().map(|id| shared::types::ProviderConfig::with_default_model(id)).collect();
            Ok(RoutingStrategy::PriorityOrder { providers })
        }
        "weighted" => {
            let weights_str = if let Some(ref w) = args.routing_weights {
                w.clone()
            } else {
                env::var("ROUTING_WEIGHTS")
                    .map_err(|_| "--routing-weights or ROUTING_WEIGHTS env must be specified for weighted strategy".to_string())?
            };
            
            let provider_weights = parse_weights(&weights_str)?;
            if provider_weights.is_empty() {
                return Err("No valid weights provided".to_string());
            }
            let weights = provider_weights.into_iter().map(|(id, weight)| {
                (shared::types::ProviderConfig::with_default_model(id), weight)
            }).collect();
            Ok(RoutingStrategy::Weighted { weights })
        }
        _ => Err(format!("Unknown routing strategy '{}'. Valid options: backoff, roundrobin, priority, weighted", args.routing_strategy)),
    }
}

/// Parse comma-separated provider list
fn parse_provider_list(providers_str: &Option<String>) -> Result<Vec<ProviderId>, String> {
    match providers_str {
        Some(s) => s
            .split(',')
            .map(|p| p.trim().parse())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Invalid provider in list: {}", e)),
        None => Ok(Vec::new()),
    }
}

/// Parse provider weights (format: "provider1:weight1,provider2:weight2")
fn parse_weights(weights_str: &str) -> Result<HashMap<ProviderId, f32>, String> {
    let mut weights = HashMap::new();
    
    for pair in weights_str.split(',') {
        let parts: Vec<&str> = pair.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid weight format '{}'. Expected 'provider:weight'", pair));
        }
        
        let provider: ProviderId = parts[0].trim().parse()
            .map_err(|e| format!("Invalid provider '{}': {}", parts[0], e))?;
        let weight: f32 = parts[1].trim().parse()
            .map_err(|e| format!("Invalid weight '{}': {}", parts[1], e))?;
        
        if weight < 0.0 || weight > 1.0 {
            return Err(format!("Weight {} must be between 0.0 and 1.0", weight));
        }
        
        weights.insert(provider, weight);
    }
    
    // Validate weights sum to approximately 1.0
    let sum: f32 = weights.values().sum();
    if (sum - 1.0).abs() > 0.01 {
        return Err(format!("Weights sum to {:.3}, but should sum to 1.0", sum));
    }
    
    Ok(weights)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Initialize process ID with the provided ID
    ProcessId::init_producer(args.id);

    // Initialize tracing with optional endpoint and log level
    let trace_endpoint = args
        .trace_ep
        .as_ref()
        .map(|url| shared::logging::TracingEndpoint::new(url.clone()));
    
    // Debug: Print tracing configuration
    if let Some(ref endpoint) = trace_endpoint {
        println!("🔍 Producer tracing endpoint: {}", endpoint.url);
        println!("🔍 Producer log level: {}", args.log_level);
    } else {
        println!("🔍 Producer: No tracing endpoint configured");
    }
    
    shared::logging::init_tracing_with_endpoint_and_level(trace_endpoint, Some(&args.log_level));
    
    // Test trace immediately after initialization
    tracing::info!("🧪 Producer tracing test - this should appear in traces");
    println!("🔍 Producer: Tracing initialized, test message sent");

    // Determine operating mode based on orchestrator address availability (like webserver)
    let standalone_mode = args.orchestrator_addr.is_none();
    let cli_mode = args.topic.is_some() || standalone_mode; // CLI mode includes standalone mode
    
    // Determine provider selection and routing strategy - prioritize routing config from orchestrator
    let (use_test_provider, routing_strategy_from_orchestrator) = if let Some(ref routing_config) = args.routing_config {
        // Parse routing configuration from orchestrator
        let routing_strategy = parse_routing_config(routing_config)
            .map_err(|e| format!("Failed to parse routing config: {}", e))?;
        let use_test = contains_random_provider(&routing_strategy);
        process_debug!(ProcessId::current(), "🎯 Using routing config: '{}' -> strategy: {:?}", routing_config, routing_strategy);
        (use_test, Some(routing_strategy))
    } else {
        // Fallback to old provider argument logic
        let use_test = cli_mode && args.provider == "random";
        process_debug!(ProcessId::current(), "🎯 Using provider argument: '{}' -> test mode: {}", args.provider, use_test);
        (use_test, None)
    };
    
    /// Check if routing strategy contains Random provider
    fn contains_random_provider(strategy: &RoutingStrategy) -> bool {
        match strategy {
            RoutingStrategy::Backoff { provider } => provider.provider == ProviderId::Random,
            RoutingStrategy::RoundRobin { providers } => providers.iter().any(|p| p.provider == ProviderId::Random),
            RoutingStrategy::PriorityOrder { providers } => providers.iter().any(|p| p.provider == ProviderId::Random),
            RoutingStrategy::Weighted { weights } => weights.keys().any(|p| p.provider == ProviderId::Random),
        }
    }

    // Create producer configuration
    let topic = if args.topic.is_some() {
        args.topic.as_ref().unwrap().clone()
    } else if standalone_mode {
        "Standalone test topic".to_string()
    } else {
        "Production generation topic".to_string()
    };

    process_debug!(ProcessId::current(), "🎯 Producer main - topic set to: '{}'", topic);
    process_debug!(ProcessId::current(), "🎯 Producer main - args.topic: {:?}", args.topic);
    process_debug!(ProcessId::current(), "🎯 Producer main - standalone_mode: {}", standalone_mode);

    let orchestrator_addr = if standalone_mode {
        "127.0.0.1:0".parse().unwrap() // Dummy address for standalone mode
    } else {
        args.orchestrator_addr
            .as_ref()
            .unwrap()
            .parse()
            .map_err(|e| format!("Invalid orchestrator address: {}", e))?
    };

    let mut config = ProducerConfig::new(orchestrator_addr, topic.clone());

    if standalone_mode {
        process_info!(
            ProcessId::current(),
            "🔧 Starting producer in standalone mode (no orchestrator connection)"
        );
        process_info!(
            ProcessId::current(),
            "Topic: {}, Provider: {}, Request Size: {}",
            topic,
            if use_test_provider {
                "test (Random)"
            } else {
                "env (API keys)"
            },
            args.request_size
        );
    } else if cli_mode {
        process_info!(ProcessId::current(), "🖥️  Starting producer in CLI mode");
        process_info!(
            ProcessId::current(),
            "Topic: {}, Provider: {}, Request Size: {}",
            args.topic.as_ref().unwrap(),
            if use_test_provider {
                "test (Random)"
            } else {
                "env (API keys)"
            },
            args.request_size
        );
    } else {
        logging::log_startup(ProcessId::current(), "producer service");
        process_info!(
            ProcessId::current(),
            "Orchestrator: {}",
            args.orchestrator_addr.as_ref().unwrap()
        );
    }

    // Try to load .env file if it exists (for test mode convenience)
    if let Ok(_) = dotenvy::dotenv() {
        process_debug!(ProcessId::current(), "📄 Loaded .env file from current directory");
    } else if let Ok(_) = dotenvy::from_path("../.env") {
        process_debug!(ProcessId::current(), "📄 Loaded .env file from parent directory");
    } else {
        process_debug!(
            ProcessId::current(),
            "📄 No .env file found - using environment variables"
        );
    }

    // Collect API keys from args and environment
    let mut api_keys = HashMap::new();

    // For CLI mode with test provider, use Random provider, otherwise collect API keys
    if use_test_provider {
        // Use Random provider for keyless testing
        process_debug!(
            ProcessId::current(),
            "🧪 Test provider mode: using Random provider (no API key needed)"
        );
        api_keys.insert(ProviderId::Random, "dummy".to_string());
        process_debug!(
            ProcessId::current(),
            "Random provider configured for test mode (no API key needed)"
        );
    } else {
        // Production/CLI mode with env provider - collect all available keys
        if let Some(key) = args.openai_key.as_ref().cloned().or_else(|| env::var("OPENAI_API_KEY").ok()) {
            api_keys.insert(ProviderId::OpenAI, key);
            process_debug!(ProcessId::current(), "OpenAI API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ OpenAI API key not provided");
        }

        if let Some(key) = args.anthropic_key.as_ref().cloned().or_else(|| env::var("ANTHROPIC_API_KEY").ok()) {
            api_keys.insert(ProviderId::Anthropic, key);
            process_debug!(ProcessId::current(), "Anthropic API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ Anthropic API key not provided");
        }

        if let Some(key) = args.gemini_key.as_ref().cloned().or_else(|| env::var("GEMINI_API_KEY").ok()) {
            api_keys.insert(ProviderId::Gemini, key);
            process_debug!(ProcessId::current(), "Gemini API key configured");
        } else {
            process_warn!(ProcessId::current(), "⚠️ Gemini API key not provided");
        }

        // Random provider (always available as fallback)
        if let Some(key) = args.random_key.as_ref().cloned().or_else(|| env::var("RANDOM_API_KEY").ok()) {
            api_keys.insert(ProviderId::Random, key);
            process_debug!(
                ProcessId::current(),
                "Random provider configured (for testing/fallback)"
            );
        } else {
            // Random provider works without API key
            api_keys.insert(ProviderId::Random, "dummy".to_string());
            process_debug!(ProcessId::current(), "Random provider available (keyless)");
        }

        // In production mode, ensure we have at least one real API key (excluding Random)
        let real_providers: Vec<_> = api_keys.keys().filter(|&&p| p != ProviderId::Random).collect();
        if real_providers.is_empty() && !cli_mode {
            process_warn!(
                ProcessId::current(),
                "⚠️ No real API providers configured. Only Random provider available."
            );
            process_warn!(
                ProcessId::current(),
                "   Consider setting at least one of: OPENAI_API_KEY, ANTHROPIC_API_KEY, GEMINI_API_KEY"
            );
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

    // Use routing strategy from orchestrator if available, otherwise parse from command-line arguments
    let routing_strategy = if let Some(strategy) = routing_strategy_from_orchestrator {
        process_debug!(ProcessId::current(), "🎯 Using routing strategy from orchestrator: {:?}", strategy);
        strategy
    } else {
        let strategy = parse_routing_strategy(&args, use_test_provider)
            .map_err(|e| format!("Invalid routing strategy: {}", e))?;
        process_debug!(ProcessId::current(), "🎯 Using routing strategy from command-line: {:?}", strategy);
        strategy
    };
    
    process_info!(ProcessId::current(), "🎯 Routing strategy: {:?}", routing_strategy);
    
    // Create execution configuration
    let orchestrator_endpoint = if standalone_mode {
        None
    } else {
        args.orchestrator_addr.clone()
    };
    let execution_config = ExecutionConfig::from_args_and_env(
        orchestrator_endpoint,
        config.topic.clone(),
        Some(2),                                       // 2 second interval
        args.max_requests.or_else(|| if standalone_mode { Some(50) } else { None }), // Use command-line arg or default to 50 for standalone
        Some(routing_strategy),
    )
    .map_err(|e| format!("Failed to create execution config: {}", e))?;

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
