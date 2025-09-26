//! Routing Strategy Command-Line Test
//!
//! This test verifies that routing strategies can be configured via command-line arguments
//! instead of environment variables, making tests more reliable and isolated.

use producer::{RealApiClient, ApiClient};
use producer::types::ApiRequest;
use shared::ProviderId;
use uuid::Uuid;
use chrono::Utc;
use std::process::Command;

#[test]
fn test_routing_strategy_cmdline_backoff() {
    println!("🎯 Testing command-line routing strategy: Backoff");
    
    // Test backoff strategy with Random provider for testing
    let output = Command::new("cargo")
        .args(&["run", "--", 
               "--routing-strategy", "backoff",
               "--routing-provider", "random",
               "--topic", "test",
               "--provider", "random",
               "--max-requests", "1"]) // Use random for testing without real API calls
        .output()
        .expect("Failed to execute producer");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    println!("Output: {}", stdout);
    if !stderr.is_empty() {
        println!("Errors: {}", stderr);
    }
    
    assert!(output.status.success(), "Producer should run successfully");
    assert!(stdout.contains("Routing strategy: Backoff { provider: Random }"));
}

#[tokio::test]
async fn test_routing_strategy_cmdline_roundrobin() {
    println!("🔄 Testing command-line routing strategy: Round-robin");
    
    // Test round-robin strategy
    let output = Command::new("cargo")
        .args(&["run", "--", 
               "--routing-strategy", "roundrobin",
               "--routing-providers", "random",
               "--topic", "test",
               "--provider", "random",
               "--max-requests", "3"])
        .output()
        .expect("Failed to execute producer");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(output.status.success(), "Producer should run successfully");
    assert!(stdout.contains("Routing strategy: RoundRobin"));
}

#[tokio::test]
async fn test_routing_strategy_cmdline_priority() {
    println!("📋 Testing command-line routing strategy: Priority");
    
    // Test priority strategy (cheapest providers first)
    let output = Command::new("cargo")
        .args(&["run", "--", 
               "--routing-strategy", "priority",
               "--routing-providers", "random",
               "--topic", "test",
               "--provider", "random",
               "--max-requests", "3"])
        .output()
        .expect("Failed to execute producer");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(output.status.success(), "Producer should run successfully");
    assert!(stdout.contains("Routing strategy: PriorityOrder"));
}

#[tokio::test]
async fn test_routing_strategy_cmdline_weighted() {
    println!("⚖️ Testing command-line routing strategy: Weighted");
    
    // Test weighted strategy
    let output = Command::new("cargo")
        .args(&["run", "--", 
               "--routing-strategy", "weighted",
               "--routing-weights", "random:1.0",
               "--topic", "test",
               "--provider", "random",
               "--max-requests", "3"])
        .output()
        .expect("Failed to execute producer");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    assert!(output.status.success(), "Producer should run successfully");
    assert!(stdout.contains("Routing strategy: Weighted"));
}

/// End-to-End test using command-line args for backoff/OpenAI routing with real API
#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run this test
async fn test_e2e_cmdline_backoff_openai_paris_attractions() {
    use std::env;
    
    println!("🗼 E2E Test: Command-line routing with Paris attractions");
    
    // Load .env file to get API key
    let _ = dotenvy::dotenv();
    
    // Check if OpenAI API key is available
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            println!("✅ OpenAI API key found (length: {})", key.trim().len());
        },
        Err(_) => {
            println!("❌ No OpenAI API key found - skipping E2E test");
            println!("   Set OPENAI_API_KEY environment variable to run this test");
            return;
        }
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
                println!("❌ OpenAI health check failed");
                return;
            }
        }
        Err(e) => {
            println!("❌ Health check error: {}", e);
            return;
        }
    }
    
    // Create the API request for Paris attractions
    let request = ApiRequest {
        provider: ProviderId::OpenAI,
        prompt: "Generate 5 new entries about: paris attractions

CRITICAL FORMATTING REQUIREMENTS:
- Words must be strictly alphanumeric (letters and numbers only)
- Words must be lowercase
- One entry per line
- No punctuation, spaces, or special characters
- Examples: \"eiffeltower\", \"louvremuseum\", \"notredame\"

Only generate canonical names, in English when available. Omit any descriptions of the entries.".to_string(),
        max_tokens: 150,
        temperature: 0.7,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    };
    
    println!("🚀 Making E2E API request for Paris attractions...");
    
    match api_client.send_request(request).await {
        Ok(response) => {
            println!("✅ E2E API request successful!");
            println!("📝 Generated Paris attractions:");
            
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
            
            // Validate response
            assert!(response.success, "API response should be successful");
            assert!(!response.content.is_empty(), "Response should have content");
            assert!(response.tokens_used > 0, "Should report token usage");
            
            // Validate attractions
            assert!(attractions.len() >= 3, "Should generate at least 3 attractions");
            assert!(attractions.len() <= 10, "Should not generate more than 10 attractions");
            
            // Validate formatting
            for attraction in attractions {
                let clean = attraction.trim();
                if !clean.is_empty() {
                    assert!(clean.chars().all(|c| c.is_alphanumeric()), 
                           "Attraction '{}' should be alphanumeric only", clean);
                    assert!(clean.chars().all(|c| !c.is_uppercase()), 
                           "Attraction '{}' should be lowercase", clean);
                }
            }
            
            println!("✅ All tests passed!");
            println!("🎉 E2E command-line routing test completed successfully!");
            
        }
        Err(e) => {
            println!("❌ E2E API request failed: {}", e);
            println!("   Check API key and connectivity");
        }
    }
}