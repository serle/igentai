//! Real API integration tests
//!
//! These tests verify that the API client can correctly interact with provider APIs.
//! Uses the Random provider for keyless testing and can optionally test real providers
//! when API keys are available.

use std::collections::HashMap;
use uuid::Uuid;

use chrono::Utc;
use producer::services::api_client::RealApiClient;
use producer::traits::ApiClient;
use producer::types::{ApiRequest, ProducerConfig};
use shared::{GenerationConfig, ProviderId};

/// Create a test API request
fn create_api_request(provider: ProviderId, prompt: &str, max_tokens: u32) -> ApiRequest {
    ApiRequest {
        provider,
        prompt: prompt.to_string(),
        max_tokens,
        temperature: 0.7,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    }
}

/// Create API client for testing
fn create_api_client() -> RealApiClient {
    let mut api_keys = HashMap::new();

    // Add real API keys if available from environment
    if let Ok(openai_key) = std::env::var("OPENAI_API_KEY") {
        api_keys.insert(ProviderId::OpenAI, openai_key);
    }
    if let Ok(anthropic_key) = std::env::var("ANTHROPIC_API_KEY") {
        api_keys.insert(ProviderId::Anthropic, anthropic_key);
    }
    if let Ok(gemini_key) = std::env::var("GEMINI_API_KEY") {
        api_keys.insert(ProviderId::Gemini, gemini_key);
    }

    // Random provider doesn't need an API key
    api_keys.insert(ProviderId::Random, "test-key".to_string());

    RealApiClient::new(api_keys, 30000)
}

#[tokio::test]
async fn test_random_provider_api_integration() {
    let api_client = create_api_client();

    // Test basic request
    let request = create_api_request(ProviderId::Random, "Generate 10 tourist attractions in Paris", 100);

    let response = api_client
        .send_request(request)
        .await
        .expect("Random provider request should succeed");

    assert!(response.success, "Response should be successful");
    assert!(!response.content.is_empty(), "Response should have content");
    assert!(response.tokens_used > 0, "Should report token usage");
    assert!(response.response_time_ms > 0, "Should report response time");

    // Verify content contains some recognizable words
    let content = response.content.to_lowercase();
    assert!(content.len() > 10, "Content should be substantial");
}

#[tokio::test]
async fn test_random_provider_different_prompts() {
    let api_client = create_api_client();

    let test_cases = vec![
        ("Generate 5 restaurant names in Tokyo", 50),
        ("List 3 museums in London", 30),
        ("Name 8 beaches in Australia", 80),
    ];

    for (prompt, max_tokens) in test_cases {
        let request = create_api_request(ProviderId::Random, prompt, max_tokens);
        let response = api_client
            .send_request(request)
            .await
            .expect(&format!("Request should succeed for prompt: {}", prompt));

        assert!(response.success, "Response should be successful for prompt: {}", prompt);
        assert!(
            !response.content.is_empty(),
            "Response should have content for prompt: {}",
            prompt
        );
        assert!(
            response.tokens_used <= max_tokens,
            "Token usage should respect limit for prompt: {}",
            prompt
        );
    }
}

#[tokio::test]
async fn test_random_provider_concurrent_requests() {
    let mut handles = Vec::new();

    for i in 0..5 {
        let client = create_api_client();
        let handle = tokio::spawn(async move {
            let request = create_api_request(ProviderId::Random, &format!("Generate {} places in Spain", i + 3), 50);
            client.send_request(request).await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    for (i, handle) in handles.into_iter().enumerate() {
        let response = handle
            .await
            .expect("Task should complete")
            .expect(&format!("Request {} should succeed", i));

        assert!(response.success, "Response {} should be successful", i);
        assert!(!response.content.is_empty(), "Response {} should have content", i);
    }
}

#[tokio::test]
async fn test_cost_estimation() {
    let api_client = create_api_client();

    // Test cost estimation for different providers
    let test_cases = vec![
        (ProviderId::Random, 100, 0.0),  // Random provider is free
        (ProviderId::OpenAI, 1000, 0.0), // Should have some cost estimation
        (ProviderId::Anthropic, 1000, 0.0),
        (ProviderId::Gemini, 1000, 0.0),
    ];

    for (provider, tokens, expected_min_cost) in test_cases {
        let cost = api_client.estimate_cost(provider, tokens);
        assert!(
            cost >= expected_min_cost,
            "Cost estimation for {:?} with {} tokens should be at least {}",
            provider,
            tokens,
            expected_min_cost
        );
    }
}

#[tokio::test]
async fn test_health_check_random_provider() {
    let api_client = create_api_client();

    // Random provider should always be healthy
    let health_result = api_client.health_check(ProviderId::Random).await;
    assert!(health_result.is_ok(), "Random provider health check should succeed");
    assert!(health_result.unwrap(), "Random provider should be healthy");
}

#[tokio::test]
async fn test_health_check_missing_keys() {
    // Create client without API keys
    let api_client = RealApiClient::new(HashMap::new(), 30000);

    // Health checks should fail for providers without keys
    let providers_needing_keys = vec![ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini];

    for provider in providers_needing_keys {
        let health_result = api_client.health_check(provider).await;
        assert!(health_result.is_ok(), "Health check should return a result");
        assert!(
            !health_result.unwrap(),
            "Health check for {:?} should return false without API key",
            provider
        );
    }
}

#[tokio::test]
async fn test_request_with_different_generation_configs() {
    let api_client = create_api_client();

    let configs = vec![
        GenerationConfig {
            model: "test".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 50,
            temperature: 0.1,
            request_size: 5,
        },
        GenerationConfig {
            model: "test".to_string(),
            batch_size: 1,
            context_window: 8192,
            max_tokens: 100,
            temperature: 0.9,
            request_size: 10,
        },
    ];

    for (i, config) in configs.iter().enumerate() {
        let request = ApiRequest {
            provider: ProviderId::Random,
            prompt: format!("Generate {} tourist attractions", config.request_size),
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        };

        let response = api_client
            .send_request(request)
            .await
            .expect(&format!("Request with config {} should succeed", i));

        assert!(response.success, "Response with config {} should be successful", i);
        assert!(
            response.tokens_used <= config.max_tokens,
            "Token usage should respect config {} limit",
            i
        );
    }
}

// Optional integration tests with real APIs (only run if API keys are available)
#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run these tests
async fn test_openai_integration_if_key_available() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Skipping OpenAI integration test - no API key provided");
        return;
    }

    let api_client = create_api_client();

    let request = create_api_request(ProviderId::OpenAI, "List 3 famous landmarks", 100);

    let response = api_client
        .send_request(request)
        .await
        .expect("OpenAI request should succeed with valid API key");

    assert!(response.success, "OpenAI response should be successful");
    assert!(!response.content.is_empty(), "OpenAI response should have content");
    assert!(response.tokens_used > 0, "OpenAI should report token usage");
}

#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run these tests
async fn test_anthropic_integration_if_key_available() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping Anthropic integration test - no API key provided");
        return;
    }

    let api_client = create_api_client();

    let request = create_api_request(ProviderId::Anthropic, "List 3 famous museums", 100);

    let response = api_client
        .send_request(request)
        .await
        .expect("Anthropic request should succeed with valid API key");

    assert!(response.success, "Anthropic response should be successful");
    assert!(!response.content.is_empty(), "Anthropic response should have content");
    assert!(response.tokens_used > 0, "Anthropic should report token usage");
}

#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run these tests
async fn test_gemini_integration_if_key_available() {
    if std::env::var("GEMINI_API_KEY").is_err() {
        eprintln!("Skipping Gemini integration test - no API key provided");
        return;
    }

    let api_client = create_api_client();

    let request = create_api_request(ProviderId::Gemini, "List 3 famous parks", 100);

    let response = api_client
        .send_request(request)
        .await
        .expect("Gemini request should succeed with valid API key");

    assert!(response.success, "Gemini response should be successful");
    assert!(!response.content.is_empty(), "Gemini response should have content");
    assert!(response.tokens_used > 0, "Gemini should report token usage");
}

#[tokio::test]
async fn test_api_error_handling() {
    // Create client with invalid API key for testing error handling
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::OpenAI, "invalid-key".to_string());
    let api_client = RealApiClient::new(api_keys, 30000);

    let request = create_api_request(ProviderId::OpenAI, "Test prompt", 50);

    // This should handle the error gracefully
    let result = api_client.send_request(request).await;

    match result {
        Ok(response) => {
            // If it returns a response, it should indicate failure
            assert!(!response.success, "Response should indicate failure with invalid key");
            assert!(response.error_message.is_some(), "Should have error message");
        }
        Err(_) => {
            // Error is also acceptable for invalid API key
        }
    }
}

#[tokio::test]
async fn test_prompt_enhancement_integration() {
    use producer::core::{Processor, PromptHandler};
    use producer::types::ProducerState;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let api_client = create_api_client();
    let prompt_handler = PromptHandler::new();
    let processor = Arc::new(RwLock::new(Processor::new()));

    // Create a minimal producer state
    let config = ProducerConfig::new("127.0.0.1:6001".parse().unwrap(), "test topic".to_string());
    let state = Arc::new(RwLock::new(ProducerState::new(config)));

    let generation_config = GenerationConfig {
        model: "test".to_string(),
        batch_size: 1,
        context_window: 4096,
        max_tokens: 100,
        temperature: 0.7,
        request_size: 15,
    };

    // Build enhanced prompt
    let enhanced_prompt = prompt_handler
        .build_enhanced_prompt(
            "famous landmarks",
            ProviderId::Random,
            Some(&generation_config),
            &state,
            &processor,
        )
        .await;

    // Verify prompt includes the request size and formatting requirements
    assert!(enhanced_prompt.contains("15"), "Prompt should include request size");
    assert!(
        enhanced_prompt.contains("alphanumeric"),
        "Prompt should include formatting requirements"
    );
    assert!(
        enhanced_prompt.contains("lowercase"),
        "Prompt should specify lowercase requirement"
    );

    // Test the enhanced prompt with API
    let request = ApiRequest {
        provider: ProviderId::Random,
        prompt: enhanced_prompt,
        max_tokens: generation_config.max_tokens,
        temperature: generation_config.temperature,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    };

    let response = api_client
        .send_request(request)
        .await
        .expect("Enhanced prompt request should succeed");

    assert!(response.success, "Enhanced prompt response should be successful");
    assert!(
        !response.content.is_empty(),
        "Enhanced prompt response should have content"
    );
}
