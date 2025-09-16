//! Test Random provider implementation

use producer::{ApiClient, RealApiClient};
use shared::ProviderId;
use std::collections::HashMap;

#[tokio::test]
async fn test_random_provider_basic() {
    // Create API client with Random provider
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::Random, "dummy".to_string());

    let api_client = RealApiClient::new(api_keys, 5000);

    // Test health check
    let health = api_client.health_check(ProviderId::Random).await.unwrap();
    assert!(health, "Random provider should always be healthy");

    // Test cost estimation
    let cost = api_client.estimate_cost(ProviderId::Random, 100);
    assert_eq!(cost, 0.0, "Random provider should be free");
}

#[tokio::test]
async fn test_random_provider_requests() {
    // Create API client with Random provider
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::Random, "dummy".to_string());

    let api_client = RealApiClient::new(api_keys, 5000);

    // Create a test request
    let request = producer::types::ApiRequest {
        provider: ProviderId::Random,
        prompt: "Generate random content".to_string(),
        max_tokens: 50,
        temperature: 0.7,
        request_id: uuid::Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
    };

    // Make request
    let response = api_client.send_request(request).await.unwrap();

    // Verify response
    assert!(response.success, "Random provider request should succeed");
    assert!(!response.content.is_empty(), "Response should have content");
    assert!(response.tokens_used > 0, "Should report token usage");
    assert!(response.response_time_ms > 0, "Should have response time");
    assert!(response.error_message.is_none(), "Should have no error");

    println!("Random provider generated: {}", response.content);
    println!("Tokens used: {}", response.tokens_used);
    println!("Response time: {}ms", response.response_time_ms);
}

#[tokio::test]
async fn test_random_provider_multiple_requests() {
    // Create API client with Random provider
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::Random, "dummy".to_string());

    let api_client = RealApiClient::new(api_keys, 5000);

    let mut responses = Vec::new();

    // Make multiple requests to test variation
    for i in 0..3 {
        let request = producer::types::ApiRequest {
            provider: ProviderId::Random,
            prompt: format!("Request {}", i),
            max_tokens: 20,
            temperature: 0.7,
            request_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
        };

        let response = api_client.send_request(request).await.unwrap();
        assert!(response.success);
        responses.push(response.content);
    }

    // Verify that we get different random content
    println!("Response 1: {}", responses[0]);
    println!("Response 2: {}", responses[1]);
    println!("Response 3: {}", responses[2]);

    // Since it's random, they should likely be different (though not guaranteed)
    // We'll just verify they're not empty
    for response in &responses {
        assert!(!response.is_empty());
    }
}

#[tokio::test]
async fn test_random_provider_different_token_limits() {
    // Create API client with Random provider
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::Random, "dummy".to_string());

    let api_client = RealApiClient::new(api_keys, 5000);

    // Test with different max_tokens values
    for max_tokens in [5, 20, 50] {
        let request = producer::types::ApiRequest {
            provider: ProviderId::Random,
            prompt: "Generate content".to_string(),
            max_tokens,
            temperature: 0.7,
            request_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
        };

        let response = api_client.send_request(request).await.unwrap();
        assert!(response.success);

        // Token usage should be approximately the max_tokens requested
        let word_count = response.content.split_whitespace().count();
        assert_eq!(response.tokens_used as usize, word_count);
        assert!(
            response.tokens_used <= max_tokens,
            "Tokens used ({}) should not exceed max_tokens ({})",
            response.tokens_used,
            max_tokens
        );

        println!(
            "Max tokens: {}, Used: {}, Content: {}",
            max_tokens, response.tokens_used, response.content
        );
    }
}
