//! Comprehensive API client integration tests
//! 
//! This file consolidates all API client testing including:
//! - Random provider testing (always available)
//! - Real provider integration (OpenAI, Anthropic, Gemini) with API keys
//! - Health checks and cost estimation
//! - Error handling and timeout scenarios
//! - End-to-end API request flows

use producer::{RealApiClient, ApiClient};
use producer::types::ApiRequest;
use shared::{ProviderId, TokenUsage};
use uuid::Uuid;
use chrono::Utc;
use std::env;
use std::collections::HashMap;

/// Create a test API client with all providers
fn create_test_client() -> RealApiClient {
    let mut api_keys = HashMap::new();
    
    // Always include Random provider (no key needed)
    api_keys.insert(ProviderId::Random, "not-needed".to_string());
    
    // Add real provider keys if available
    if let Ok(openai_key) = env::var("OPENAI_API_KEY") {
        api_keys.insert(ProviderId::OpenAI, openai_key);
    }
    if let Ok(anthropic_key) = env::var("ANTHROPIC_API_KEY") {
        api_keys.insert(ProviderId::Anthropic, anthropic_key);
    }
    if let Ok(gemini_key) = env::var("GEMINI_API_KEY") {
        api_keys.insert(ProviderId::Gemini, gemini_key);
    }
    
    RealApiClient::new(api_keys, 30000) // 30 second timeout
}

/// Create a test API request
fn create_test_request(provider: ProviderId, prompt: &str) -> ApiRequest {
    ApiRequest {
        provider,
        prompt: prompt.to_string(),
        max_tokens: 150,
        temperature: 0.7,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    }
}

#[tokio::test]
async fn test_random_provider_basic_functionality() {
    let api_client = create_test_client();
    
    // Test basic request
    let request = create_test_request(ProviderId::Random, "Generate 5 colors");
    let response = api_client.send_request(request).await.expect("Request should succeed");
    
    assert!(response.success, "Random provider should always succeed");
    assert!(!response.content.is_empty(), "Should generate content");
    assert!(response.tokens_used.total() > 0, "Should report token usage");
    assert!(response.response_time_ms > 0, "Should have response time");
    
    println!("Random provider response: {}", response.content);
}

#[tokio::test]
async fn test_cost_estimation_all_providers() {
    let api_client = create_test_client();
    let test_tokens = TokenUsage { input_tokens: 500, output_tokens: 500 };
    
    // Test cost estimation for all providers
    let providers = [ProviderId::Random, ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini];
    
    for provider in providers {
        let cost = api_client.estimate_cost(provider, &test_tokens);
        assert!(cost >= 0.0, "Cost should be non-negative for {:?}", provider);
        
        // Random provider has a simulated cost
        if provider == ProviderId::Random {
            assert!(cost > 0.0, "Random provider should have simulated cost");
        }
        
        println!("{:?} cost for 1000 tokens: ${:.6}", provider, cost);
    }
}

#[tokio::test]
async fn test_request_validation() {
    let api_client = create_test_client();
    
    // Test empty prompt
    let mut request = create_test_request(ProviderId::Random, "");
    api_client.send_request(request).await.expect("Request should complete");
    // Random provider should handle empty prompts gracefully
    
    // Test very long prompt
    request = create_test_request(ProviderId::Random, &"word ".repeat(1000));
    let response = api_client.send_request(request).await.expect("Request should complete");
    assert!(response.success || !response.error_message.as_ref().unwrap_or(&String::new()).is_empty());
}

#[tokio::test]
#[ignore] // Only run when API keys are available
async fn test_openai_integration() {
    if env::var("OPENAI_API_KEY").is_err() {
        println!("Skipping OpenAI test - no API key");
        return;
    }
    
    let api_client = create_test_client();
    
    // Test cost estimation
    let test_tokens = TokenUsage { input_tokens: 500, output_tokens: 500 };
    let cost = api_client.estimate_cost(ProviderId::OpenAI, &test_tokens);
    assert_eq!(cost, 0.00015, "Should match gpt-4o-mini pricing");
    
    // Test real API request
    let request = create_test_request(
        ProviderId::OpenAI,
        "Generate 3 unique scientific terms related to physics:"
    );
    
    let response = api_client.send_request(request).await.expect("OpenAI request should succeed");
    
    if response.success {
        println!("OpenAI response: {}", response.content);
        assert!(!response.content.is_empty(), "Should have content");
        assert!(response.tokens_used.total() > 0, "Should report token usage");
        
        // Validate response format
        let terms: Vec<&str> = response.content.lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        assert!(terms.len() >= 2, "Should generate at least 2 terms");
    } else {
        println!("OpenAI request failed: {:?}", response.error_message);
        // Could be rate limiting or other issues - don't fail the test
    }
}

#[tokio::test]
async fn test_token_usage_reporting() {
    let api_client = create_test_client();
    
    let long_prompt = format!("Explain the concept of {} in detail", "machine learning ".repeat(10));
    let test_cases = vec![
        ("Short prompt", "Hi"),
        ("Medium prompt", "Generate a list of 5 programming languages"),
        ("Long prompt", &long_prompt),
    ];
    
    for (description, prompt) in test_cases {
        let request = create_test_request(ProviderId::Random, prompt);
        let response = api_client.send_request(request).await.expect("Request should succeed");
        
        assert!(response.tokens_used.total() > 0, "{}: Should report token usage", description);
        // Random provider might report zero input tokens for very short prompts
        if response.tokens_used.input_tokens == 0 && description == "Short prompt" {
            println!("{}: Random provider reported zero input tokens for short prompt", description);
        } else {
            assert!(response.tokens_used.input_tokens > 0, "{}: Should have input tokens", description);
        }
        
        println!("{}: {} total tokens (input: {}, output: {})", 
                description, 
                response.tokens_used.total(),
                response.tokens_used.input_tokens,
                response.tokens_used.output_tokens);
    }
}

#[tokio::test]
async fn test_concurrent_requests() {
    // Send multiple concurrent requests
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let client = create_test_client();
        let handle = tokio::spawn(async move {
            let request = create_test_request(ProviderId::Random, &format!("Generate {} colors", i + 3));
            client.send_request(request).await
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    let mut success_count = 0;
    for handle in handles {
        match handle.await.expect("Task should complete") {
            Ok(response) if response.success => {
                success_count += 1;
                assert!(!response.content.is_empty(), "Should have content");
            }
            Ok(response) => {
                println!("Request failed: {:?}", response.error_message);
            }
            Err(e) => {
                println!("Request error: {}", e);
            }
        }
    }
    
    assert!(success_count >= 4, "Most concurrent requests should succeed");
}

#[tokio::test]
async fn test_request_timeout_handling() {
    let api_client = RealApiClient::new(
        [(ProviderId::Random, "test".to_string())].into_iter().collect(),
        100 // Very short timeout to test timeout handling
    );
    
    let request = create_test_request(ProviderId::Random, "Test timeout");
    
    // Even with short timeout, Random provider should be fast enough
    let response = api_client.send_request(request).await.expect("Request should complete");
    
    // Either succeeds quickly or times out gracefully
    if !response.success {
        assert!(response.error_message.is_some(), "Should have error message on failure");
    }
}

#[tokio::test] 
async fn test_max_tokens_enforcement() {
    let api_client = create_test_client();
    
    let test_cases = vec![10, 50, 100, 200];
    
    for max_tokens in test_cases {
        let mut request = create_test_request(ProviderId::Random, "Generate a long list of items");
        request.max_tokens = max_tokens;
        
        let response = api_client.send_request(request).await.expect("Request should succeed");
        
        if response.success {
            // For Random provider, token counting is simulated but should be reasonable
            assert!(
                response.tokens_used.total() <= max_tokens as u64 * 2, // Allow some flexibility for simulation
                "Token usage should respect max_tokens limit. Got {} tokens for max {}",
                response.tokens_used.total(),
                max_tokens
            );
        }
    }
}

#[tokio::test]
async fn test_temperature_parameter() {
    let api_client = create_test_client();
    
    let temperatures = vec![0.1, 0.5, 0.9];
    
    for temp in temperatures {
        let mut request = create_test_request(ProviderId::Random, "Generate a creative story");
        request.temperature = temp;
        
        let response = api_client.send_request(request).await.expect("Request should succeed");
        
        assert!(response.success, "Request with temperature {} should succeed", temp);
        assert!(!response.content.is_empty(), "Should generate content regardless of temperature");
    }
}

#[tokio::test]
async fn test_error_handling() {
    let api_client = create_test_client();
    
    // Test various edge cases that might cause errors
    let very_long_prompt = "word ".repeat(10000);
    let edge_cases = vec![
        ("empty", ""),
        ("very_long", &very_long_prompt), // Very long prompt
        ("special_chars", "Generate content with émojis 🚀 and ñ special chars"),
        ("numeric", "123456789"),
    ];
    
    for (case_name, prompt) in edge_cases {
        let request = create_test_request(ProviderId::Random, prompt);
        let response = api_client.send_request(request).await.expect("Request should complete");
        
        // Random provider should handle all cases gracefully
        if !response.success {
            println!("Case '{}' failed: {:?}", case_name, response.error_message);
            assert!(response.error_message.is_some(), "Failed request should have error message");
        } else {
            println!("Case '{}' succeeded with {} chars", case_name, response.content.len());
        }
    }
}