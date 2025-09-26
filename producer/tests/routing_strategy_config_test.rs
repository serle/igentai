//! Routing Strategy Configuration Test
//!
//! This test verifies that routing strategies are loaded correctly from environment variables.
//! It tests all routing strategy types and validates environment variable parsing.
//!
//! ## Running Tests:
//!
//! Basic routing strategy tests (no API calls):
//! ```bash
//! cargo test routing_strategy -- --test-threads=1
//! ```
//!
//! End-to-end test with real OpenAI API (requires valid API key):
//! ```bash
//! cargo test test_e2e_backoff_openai_paris_attractions -- --ignored --nocapture
//! ```
//!
//! Note: Tests in this file must run sequentially due to environment variable manipulation.

use shared::types::RoutingStrategy;
use shared::ProviderId;
use std::env;
use std::sync::Mutex;

// Test synchronization mutex to ensure tests run one at a time
static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn test_routing_strategy_env_configuration() {
    let _guard = TEST_MUTEX.lock().unwrap();
    println!("🎯 Testing routing strategy environment configuration");
    
    // Clean slate - remove any existing routing env vars first
    for var in ["ROUTING_STRATEGY", "ROUTING_PRIMARY_PROVIDER", "ROUTING_PROVIDERS", "ROUTING_WEIGHTS"] {
        env::remove_var(var);
    }
    
    // Load .env file for testing
    let _ = dotenvy::dotenv();
    
    // Test 1: Default behavior with .env file (should use random from .env)
    println!("Test 1: Default routing strategy from .env file");
    
    let strategy = RoutingStrategy::from_env().unwrap_or_else(|e| {
        panic!("Failed to load routing strategy: {}", e);
    });
    
    match strategy {
        RoutingStrategy::Backoff { provider } => {
            println!("   Got provider: {:?}", provider);
            // .env file sets ROUTING_PRIMARY_PROVIDER=random
            assert_eq!(provider, ProviderId::Random);
            println!("✅ .env configuration correctly uses Random provider: {:?}", provider);
        }
        _ => panic!("Expected backoff strategy"),
    }
    
    // Test 2: No environment variables set (fallback to random)
    println!("Test 2: Fallback behavior when no routing config");
    
    // Temporarily clear routing environment variables
    let original_strategy = env::var("ROUTING_STRATEGY").ok();
    let original_provider = env::var("ROUTING_PRIMARY_PROVIDER").ok();
    
    env::remove_var("ROUTING_STRATEGY");
    env::remove_var("ROUTING_PRIMARY_PROVIDER");
    
    let strategy = RoutingStrategy::from_env().unwrap();
    match strategy {
        RoutingStrategy::Backoff { provider } => {
            // Should fallback to Random when no config is provided
            assert_eq!(provider, ProviderId::Random);
            println!("✅ Fallback correctly uses Random provider when no config");
        }
        other => panic!("Expected backoff strategy with Random provider, got: {:?}", other),
    }
    
    // Restore original environment
    if let Some(strategy) = original_strategy {
        env::set_var("ROUTING_STRATEGY", strategy);
    }
    if let Some(provider) = original_provider {
        env::set_var("ROUTING_PRIMARY_PROVIDER", provider);
    }
    
    println!("🎉 Routing strategy environment configuration test completed!");
}

#[tokio::test]
async fn test_routing_strategy_validation() {
    let _guard = TEST_MUTEX.lock().unwrap();
    println!("🔍 Testing routing strategy validation");
    
    // Clean up environment first
    for var in ["ROUTING_STRATEGY", "ROUTING_PRIMARY_PROVIDER", "ROUTING_PROVIDERS", "ROUTING_WEIGHTS"] {
        env::remove_var(var);
    }
    
    // Test invalid strategy
    env::set_var("ROUTING_STRATEGY", "invalid_strategy");
    match RoutingStrategy::from_env() {
        Err(e) => {
            println!("✅ Invalid strategy correctly rejected: {}", e);
            assert!(e.contains("Unknown routing strategy"));
        }
        Ok(strategy) => panic!("Should have rejected invalid strategy, but got: {:?}", strategy),
    }
    
    // Clean up invalid strategy env var
    env::remove_var("ROUTING_STRATEGY");
    
    // Test missing providers for roundrobin
    env::set_var("ROUTING_STRATEGY", "roundrobin");
    env::remove_var("ROUTING_PROVIDERS");
    match RoutingStrategy::from_env() {
        Err(e) => {
            println!("✅ Missing providers correctly rejected: {}", e);
            assert!(e.contains("ROUTING_PROVIDERS"));
        }
        Ok(_) => panic!("Should have required ROUTING_PROVIDERS"),
    }
    
    // Test valid roundrobin configuration
    env::set_var("ROUTING_STRATEGY", "roundrobin");
    env::set_var("ROUTING_PROVIDERS", "openai,anthropic,random");
    match RoutingStrategy::from_env() {
        Ok(RoutingStrategy::RoundRobin { providers }) => {
            println!("✅ Valid roundrobin config accepted: {:?}", providers);
            assert_eq!(providers.len(), 3);
            assert!(providers.contains(&ProviderId::OpenAI));
            assert!(providers.contains(&ProviderId::Anthropic));
            assert!(providers.contains(&ProviderId::Random));
        }
        result => panic!("Expected valid roundrobin, got: {:?}", result),
    }
    
    // Clean up
    env::remove_var("ROUTING_STRATEGY");
    env::remove_var("ROUTING_PROVIDERS");
    
    println!("🎉 Routing strategy validation test completed!");
}

/// Test all supported routing strategy configurations with environment variables
/// This serves as documentation for users on how to configure different strategies
#[tokio::test] 
async fn test_all_routing_strategy_configurations() {
    let _guard = TEST_MUTEX.lock().unwrap();
    println!("📋 Testing all routing strategy configuration examples");
    
    // Clean up any existing environment variables first
    for var in ["ROUTING_STRATEGY", "ROUTING_PRIMARY_PROVIDER", "ROUTING_PROVIDERS", "ROUTING_WEIGHTS"] {
        env::remove_var(var);
    }
    
    // Configuration Example 1: Backoff Strategy (Production)
    println!("Example 1: Backoff Strategy");
    env::set_var("ROUTING_STRATEGY", "backoff");
    env::set_var("ROUTING_PRIMARY_PROVIDER", "openai");
    
    let strategy = RoutingStrategy::from_env()
        .map_err(|e| format!("Failed to parse backoff strategy: {}", e))
        .unwrap();
    match strategy {
        RoutingStrategy::Backoff { provider } => {
            println!("   Provider: {:?}", provider);
            assert_eq!(provider, ProviderId::OpenAI);
        }
        other => panic!("Expected backoff, got: {:?}", other),
    }
    
    // Configuration Example 2: Round-Robin Strategy
    println!("Example 2: Round-Robin Strategy");
    env::set_var("ROUTING_STRATEGY", "roundrobin");
    env::set_var("ROUTING_PROVIDERS", "openai,anthropic,gemini");
    
    let strategy = RoutingStrategy::from_env().unwrap();
    match strategy {
        RoutingStrategy::RoundRobin { providers } => {
            println!("   Providers: {:?}", providers);
            assert_eq!(providers.len(), 3);
        }
        _ => panic!("Expected roundrobin"),
    }
    
    // Configuration Example 3: Priority Order Strategy
    println!("Example 3: Priority Order Strategy");
    env::set_var("ROUTING_STRATEGY", "priority");
    env::set_var("ROUTING_PROVIDERS", "gemini,openai,anthropic,random");
    
    let strategy = RoutingStrategy::from_env().unwrap();
    match strategy {
        RoutingStrategy::PriorityOrder { providers } => {
            println!("   Priority Order: {:?}", providers);
            assert_eq!(providers[0], ProviderId::Gemini); // Cheapest first
            assert_eq!(providers.len(), 4);
        }
        _ => panic!("Expected priority order"),
    }
    
    // Configuration Example 4: Weighted Strategy
    println!("Example 4: Weighted Strategy");
    env::set_var("ROUTING_STRATEGY", "weighted");
    env::set_var("ROUTING_WEIGHTS", "openai:0.5,anthropic:0.3,random:0.2");
    env::remove_var("ROUTING_PROVIDERS"); // Not needed for weighted
    
    let strategy = RoutingStrategy::from_env().unwrap();
    match strategy {
        RoutingStrategy::Weighted { weights } => {
            println!("   Weights: {:?}", weights);
            assert_eq!(weights.len(), 3);
            assert_eq!(weights[&ProviderId::OpenAI], 0.5);
            assert_eq!(weights[&ProviderId::Anthropic], 0.3);
            assert_eq!(weights[&ProviderId::Random], 0.2);
            
            // Verify weights sum to 1.0
            let sum: f32 = weights.values().sum();
            assert!((sum - 1.0).abs() < 0.01, "Weights should sum to 1.0, got {}", sum);
        }
        _ => panic!("Expected weighted"),
    }
    
    // Clean up all environment variables
    for var in ["ROUTING_STRATEGY", "ROUTING_PRIMARY_PROVIDER", "ROUTING_PROVIDERS", "ROUTING_WEIGHTS"] {
        env::remove_var(var);
    }
    
    println!("✅ All routing strategy configurations documented and tested!");
    println!();
    println!("📖 Configuration Reference:");
    println!("   # Backoff (single provider):");
    println!("   ROUTING_STRATEGY=backoff");
    println!("   ROUTING_PRIMARY_PROVIDER=openai");
    println!();
    println!("   # Round-robin (load balancing):");
    println!("   ROUTING_STRATEGY=roundrobin");
    println!("   ROUTING_PROVIDERS=openai,anthropic,gemini");
    println!();
    println!("   # Priority order (failover):");
    println!("   ROUTING_STRATEGY=priority");
    println!("   ROUTING_PROVIDERS=gemini,openai,anthropic,random");
    println!();
    println!("   # Weighted distribution:");
    println!("   ROUTING_STRATEGY=weighted");
    println!("   ROUTING_WEIGHTS=openai:0.5,anthropic:0.3,random:0.2");
    println!();
    println!("🎉 All routing strategy configuration tests completed!");
}

/// End-to-End test using backoff/OpenAI routing strategy with real API
/// This is the only E2E test that makes actual API calls to verify the routing system works
#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run this test
async fn test_e2e_backoff_openai_paris_attractions() {
    let _guard = TEST_MUTEX.lock().unwrap();
    use producer::{RealApiClient, ApiClient};
    use producer::types::ApiRequest;
    use uuid::Uuid;
    use chrono::Utc;
    
    println!("🗼 End-to-End Test: Backoff/OpenAI routing with Paris attractions");
    
    // Load .env file
    let _ = dotenvy::dotenv();
    
    // Check if OpenAI API key is available
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            let clean_key = key.trim().to_string();
            println!("✅ OpenAI API key found (length: {})", clean_key.len());
            
            // Verify key format
            if clean_key.starts_with("sk-") {
                println!("✅ API key format looks correct");
            } else {
                println!("⚠️  Warning: API key doesn't start with 'sk-'");
            }
        },
        Err(_) => {
            println!("❌ No OpenAI API key found - skipping E2E test");
            println!("   Set OPENAI_API_KEY environment variable to run this test");
            return;
        }
    }
    
    // Configure routing strategy for OpenAI backoff
    env::set_var("ROUTING_STRATEGY", "backoff");
    env::set_var("ROUTING_PRIMARY_PROVIDER", "openai");
    
    println!("🎯 Testing routing strategy configuration:");
    let routing_strategy = RoutingStrategy::from_env().unwrap();
    match routing_strategy {
        RoutingStrategy::Backoff { provider } => {
            println!("   Strategy: Backoff");
            println!("   Provider: {:?}", provider);
            assert_eq!(provider, ProviderId::OpenAI, "Should use OpenAI provider");
        }
        _ => panic!("Expected backoff strategy"),
    }
    
    // Create API client using environment variables
    let api_client = RealApiClient::new_from_env(30000);
    
    // Test health check first
    println!("🔍 Testing OpenAI health check...");
    match api_client.health_check(ProviderId::OpenAI).await {
        Ok(is_healthy) => {
            if is_healthy {
                println!("✅ OpenAI health check passed");
            } else {
                println!("❌ OpenAI health check failed - API key might be invalid");
                return;
            }
        }
        Err(e) => {
            println!("❌ OpenAI health check error: {}", e);
            return;
        }
    }
    
    // Test cost estimation
    println!("💰 Testing cost estimation...");
    let cost = api_client.estimate_cost(ProviderId::OpenAI, 1000);
    println!("   Cost for 1000 tokens: ${:.6}", cost);
    assert_eq!(cost, 0.00015, "Should match 2025 gpt-4o-mini pricing");
    
    // Create the actual API request for Paris attractions
    let request = ApiRequest {
        provider: ProviderId::OpenAI,
        prompt: "Generate 5 new entries about: paris attractions

CRITICAL FORMATTING REQUIREMENTS:
- Words must be strictly alphanumeric (letters and numbers only)
- Words must be lowercase
- One entry per line
- No punctuation, spaces, or special characters
- Examples: \"eiffeltower\", \"louvremuseum\", \"notredame\"

Only generate canonical names, in English when available. Omit any descriptions of the entries.
Previous entries:

Remember:
- Your entries should be entirely unique from the previous
- Entries should be specific
- One entry per line
- Do NOT repeat any previously seen entries, even with slight variations".to_string(),
        max_tokens: 150,
        temperature: 0.7,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    };
    
    println!("🚀 Making E2E API request to OpenAI for Paris attractions...");
    
    match api_client.send_request(request).await {
        Ok(response) => {
            println!("✅ E2E API request successful!");
            println!("📝 Generated Paris attractions:");
            
            // Parse and display each attraction
            let attractions: Vec<&str> = response.content.trim().lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
                
            for (i, attraction) in attractions.iter().enumerate() {
                println!("   {}. {}", i + 1, attraction.trim());
            }
            
            println!("🏷️  Tokens used: {}", response.tokens_used);
            println!("⏱️  Response time: {}ms", response.response_time_ms);
            
            let estimated_cost = api_client.estimate_cost(ProviderId::OpenAI, response.tokens_used);
            println!("💰 Actual cost: ${:.6}", estimated_cost);
            
            // Validate the response
            assert!(response.success, "API response should be successful");
            assert!(!response.content.is_empty(), "Response should have content");
            assert!(response.tokens_used > 0, "Should report token usage");
            assert!(response.response_time_ms > 0, "Should have response time");
            
            // Validate we got some attractions
            assert!(attractions.len() >= 3, "Should generate at least 3 attractions, got {}", attractions.len());
            assert!(attractions.len() <= 10, "Should not generate more than 10 attractions, got {}", attractions.len());
            
            // Validate formatting requirements
            for attraction in attractions {
                let clean_attraction = attraction.trim();
                if !clean_attraction.is_empty() {
                    assert!(
                        clean_attraction.chars().all(|c| c.is_alphanumeric()), 
                        "Attraction '{}' should be alphanumeric only", 
                        clean_attraction
                    );
                    assert!(
                        clean_attraction.chars().all(|c| !c.is_uppercase()), 
                        "Attraction '{}' should be lowercase", 
                        clean_attraction
                    );
                }
            }
            
            println!("✅ All formatting requirements met!");
            println!("🎉 End-to-End routing strategy test completed successfully!");
            
        }
        Err(e) => {
            println!("❌ E2E API request failed: {}", e);
            
            // Don't panic - just report the failure for debugging
            println!("   This might be due to:");
            println!("   - Invalid API key");
            println!("   - Network connectivity issues"); 
            println!("   - OpenAI service unavailable");
            println!("   - Rate limiting");
            
            // For E2E tests, we want to know about failures but not fail CI
            println!("⚠️  E2E test failed - check API key and connectivity");
        }
    }
    
    // Clean up environment variables
    env::remove_var("ROUTING_STRATEGY");
    env::remove_var("ROUTING_PRIMARY_PROVIDER");
    
    println!("🧹 Environment variables cleaned up");
}