//! Environment Configuration Test
//!
//! This test verifies that API keys and models are loaded correctly from environment variables.

use producer::{RealApiClient, ApiClient};
use shared::{ProviderId, TokenUsage};
use std::env;

/// Test that verifies all providers can be configured via environment variables
#[tokio::test]
async fn test_env_based_configuration() {
    // Load .env file
    let _ = dotenvy::dotenv();
    
    println!("🔧 Testing environment-based API client configuration");
    
    // Create API client from environment variables
    let api_client = RealApiClient::new_from_env(30000);
    
    // Test OpenAI configuration
    match env::var("OPENAI_API_KEY") {
        Ok(_) => {
            let model = env::var("OPENAI_API_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
            println!("✅ OpenAI configured - Model: {}", model);
            
            // Test health check
            match api_client.health_check(ProviderId::OpenAI).await {
                Ok(is_healthy) => {
                    if is_healthy {
                        println!("✅ OpenAI health check: PASSED");
                    } else {
                        println!("❌ OpenAI health check: FAILED");
                    }
                }
                Err(e) => println!("⚠️  OpenAI health check error: {}", e),
            }
        }
        Err(_) => println!("⚠️  OpenAI not configured"),
    }
    
    // Test Anthropic configuration
    match env::var("ANTHROPIC_API_KEY") {
        Ok(_) => {
            let model = env::var("ANTHROPIC_API_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());
            println!("✅ Anthropic configured - Model: {}", model);
            
            match api_client.health_check(ProviderId::Anthropic).await {
                Ok(is_healthy) => {
                    if is_healthy {
                        println!("✅ Anthropic health check: PASSED");
                    } else {
                        println!("❌ Anthropic health check: FAILED");
                    }
                }
                Err(e) => println!("⚠️  Anthropic health check error: {}", e),
            }
        }
        Err(_) => println!("ℹ️  Anthropic not configured (key commented out in .env)"),
    }
    
    // Test Gemini configuration
    match env::var("GEMINI_API_KEY") {
        Ok(_) => {
            let model = env::var("GEMINI_API_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".to_string());
            println!("✅ Gemini configured - Model: {}", model);
            
            match api_client.health_check(ProviderId::Gemini).await {
                Ok(is_healthy) => {
                    if is_healthy {
                        println!("✅ Gemini health check: PASSED");
                    } else {
                        println!("❌ Gemini health check: FAILED");
                    }
                }
                Err(e) => println!("⚠️  Gemini health check error: {}", e),
            }
        }
        Err(_) => println!("ℹ️  Gemini not configured (key commented out in .env)"),
    }
    
    // Test Random provider configuration
    match env::var("RANDOM_API_KEY") {
        Ok(key) => {
            let model = env::var("RANDOM_API_MODEL").unwrap_or_else(|_| "random".to_string());
            println!("✅ Random provider configured - Key: '{}', Model: '{}'", key, model);
            
            match api_client.health_check(ProviderId::Random).await {
                Ok(is_healthy) => {
                    if is_healthy {
                        println!("✅ Random provider health check: PASSED");
                    } else {
                        println!("❌ Random provider health check: FAILED");
                    }
                    assert!(is_healthy, "Random provider should be healthy when key is configured");
                }
                Err(e) => {
                    println!("⚠️  Random provider health check error: {}", e);
                    panic!("Random provider health check should not error: {}", e);
                }
            }
        }
        Err(_) => {
            println!("❌ Random provider not configured");
            panic!("RANDOM_API_KEY should be set in .env file");
        }
    }
    
    // Test cost estimation
    println!("💰 Testing cost estimation:");
    let test_tokens = TokenUsage { input_tokens: 500, output_tokens: 500 };
    println!("   OpenAI (1000 tokens): ${:.6}", api_client.estimate_cost(ProviderId::OpenAI, &test_tokens));
    println!("   Anthropic (1000 tokens): ${:.6}", api_client.estimate_cost(ProviderId::Anthropic, &test_tokens));
    println!("   Gemini (1000 tokens): ${:.6}", api_client.estimate_cost(ProviderId::Gemini, &test_tokens));
    println!("   Random (1000 tokens): ${:.6}", api_client.estimate_cost(ProviderId::Random, &test_tokens));
    
    println!("🎉 Environment configuration test completed!");
}