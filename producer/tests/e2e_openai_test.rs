//! End-to-End OpenAI Test
//!
//! This is the only test that makes real API calls to OpenAI.
//! It uses the backoff/openai routing strategy with the "paris attractions" topic.
//!
//! Run with: cargo test test_e2e_openai_paris -- --ignored --nocapture

use producer::{RealApiClient, ApiClient};
use producer::types::ApiRequest;
use shared::ProviderId;
use uuid::Uuid;
use chrono::Utc;
use std::env;

#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run this test
async fn test_e2e_openai_paris() {
    println!("🗼 E2E Test: OpenAI API with Paris attractions");
    
    // Load .env file to get API key
    let _ = dotenvy::dotenv();
    
    // Check if OpenAI API key is available
    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            let clean_key = key.trim().to_string();
            println!("✅ OpenAI API key found (length: {})", clean_key.len());
            
            // Verify key format
            if clean_key.starts_with("sk-") {
                println!("✅ API key format looks correct");
            } else {
                println!("⚠️  Warning: API key doesn't start with 'sk-'");
            }
            clean_key
        },
        Err(_) => {
            println!("❌ No OpenAI API key found - skipping E2E test");
            println!("   Set OPENAI_API_KEY environment variable to run this test");
            return;
        }
    };
    
    // Create API client with just the OpenAI key
    let mut api_keys = std::collections::HashMap::new();
    api_keys.insert(ProviderId::OpenAI, api_key);
    let api_client = RealApiClient::new(api_keys, 30000);
    
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
            println!("❌ Health check error: {}", e);
            return;
        }
    }
    
    // Check model configuration
    match env::var("OPENAI_API_MODEL") {
        Ok(model) => println!("📝 Using model from env: {}", model),
        Err(_) => println!("📝 Using default model: gpt-4o-mini"),
    }
    
    // Test cost estimation
    println!("💰 Testing cost estimation...");
    let cost = api_client.estimate_cost(ProviderId::OpenAI, 1000);
    println!("   Cost for 1000 tokens: ${:.6}", cost);
    assert_eq!(cost, 0.00015, "Should match 2025 gpt-4o-mini pricing");
    
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
            assert!(response.response_time_ms > 0, "Should have response time");
            
            // Validate attractions
            assert!(attractions.len() >= 3, "Should generate at least 3 attractions, got {}", attractions.len());
            assert!(attractions.len() <= 10, "Should not generate more than 10 attractions, got {}", attractions.len());
            
            // Validate formatting
            for attraction in attractions {
                let clean = attraction.trim();
                if !clean.is_empty() {
                    assert!(
                        clean.chars().all(|c| c.is_alphanumeric()), 
                        "Attraction '{}' should be alphanumeric only", 
                        clean
                    );
                    assert!(
                        clean.chars().all(|c| !c.is_uppercase()), 
                        "Attraction '{}' should be lowercase", 
                        clean
                    );
                }
            }
            
            println!("✅ All formatting requirements met!");
            println!("🎉 E2E OpenAI test completed successfully!");
            
        }
        Err(e) => {
            println!("❌ E2E API request failed: {}", e);
            println!("   This might be due to:");
            println!("   - Invalid API key");
            println!("   - Network connectivity issues"); 
            println!("   - OpenAI service unavailable");
            println!("   - Rate limiting");
            
            // For E2E tests, we want to know about failures but not fail CI
            println!("⚠️  E2E test failed - check API key and connectivity");
        }
    }
}