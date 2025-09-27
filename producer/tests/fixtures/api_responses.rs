//! Mock API response fixtures for testing

#![allow(dead_code)] // Test utilities may not all be used currently

use chrono::Utc;
use producer::{ApiRequest, ApiResponse};
use shared::{ProviderId, TokenUsage};
use uuid::Uuid;

/// Create a successful API response for testing
pub fn create_success_response(provider: ProviderId, content: String, tokens: u32) -> ApiResponse {
    ApiResponse {
        provider,
        request_id: Uuid::new_v4(),
        content,
        tokens_used: TokenUsage { input_tokens: tokens as u64 / 2, output_tokens: tokens as u64 / 2 },
        response_time_ms: 500,
        timestamp: Utc::now(),
        success: true,
        error_message: None,
    }
}

/// Create a failed API response for testing
pub fn create_error_response(provider: ProviderId, error: String) -> ApiResponse {
    ApiResponse {
        provider,
        request_id: Uuid::new_v4(),
        content: String::new(),
        tokens_used: TokenUsage { input_tokens: 0, output_tokens: 0 },
        response_time_ms: 1000,
        timestamp: Utc::now(),
        success: false,
        error_message: Some(error),
    }
}

/// Create an API request for testing
pub fn create_test_request(provider: ProviderId, prompt: String) -> ApiRequest {
    producer::types::ApiRequest {
        provider,
        prompt,
        max_tokens: 150,
        temperature: 0.7,
        request_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    }
}

/// Sample content with various extractable attributes
pub fn sample_content_with_attributes() -> String {
    "Contact John Smith at john.smith@acme.com or call (555) 123-4567. \
     Our office is located at 123 Business Ave, Suite 100, Tech City, CA 90210. \
     For technical support, email support@acme.com or visit https://acme.com/support. \
     Emergency contact: (555) 987-6543. \
     Company registration: ACME Industries LLC, founded in 2020-01-15."
        .to_string()
}

/// Sample content with email addresses
pub fn sample_content_emails() -> String {
    "Please contact us at info@company.com, support@service.org, or admin@domain.net for assistance.".to_string()
}

/// Sample content with phone numbers
pub fn sample_content_phones() -> String {
    "Call us at (555) 123-4567, 555.987.6543, or +1 555-246-8135 for immediate help.".to_string()
}

/// Sample content with URLs
pub fn sample_content_urls() -> String {
    "Visit https://example.com, http://test.org, or https://secure-site.net for more information.".to_string()
}

/// Sample content with no extractable attributes
pub fn sample_content_no_attributes() -> String {
    "This is just plain text with no special patterns or extractable attributes.".to_string()
}

/// OpenAI-style response JSON
pub fn openai_response_json() -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-test123",
        "object": "chat.completion",
        "created": 1234567890,
        "model": "gpt-3.5-turbo",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": sample_content_with_attributes()
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 100,
            "total_tokens": 150
        }
    })
}

/// Anthropic-style response JSON
pub fn anthropic_response_json() -> serde_json::Value {
    serde_json::json!({
        "id": "msg_test456",
        "type": "message",
        "role": "assistant",
        "content": [{
            "type": "text",
            "text": sample_content_with_attributes()
        }],
        "model": "claude-3-sonnet-20240229",
        "stop_reason": "end_turn",
        "stop_sequence": null,
        "usage": {
            "input_tokens": 30,
            "output_tokens": 120
        }
    })
}

/// Gemini-style response JSON
pub fn gemini_response_json() -> serde_json::Value {
    serde_json::json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": sample_content_with_attributes()
                }],
                "role": "model"
            },
            "finishReason": "STOP",
            "index": 0,
            "safetyRatings": []
        }],
        "promptFeedback": {
            "safetyRatings": []
        }
    })
}

/// Create responses for all providers
pub fn create_provider_responses() -> Vec<ApiResponse> {
    vec![
        create_success_response(ProviderId::OpenAI, sample_content_emails(), 100),
        create_success_response(ProviderId::Anthropic, sample_content_phones(), 120),
        create_success_response(ProviderId::Gemini, sample_content_urls(), 80),
    ]
}

/// Create mixed success/error responses
pub fn create_mixed_responses() -> Vec<ApiResponse> {
    vec![
        create_success_response(ProviderId::OpenAI, sample_content_with_attributes(), 150),
        create_error_response(ProviderId::Anthropic, "Rate limit exceeded".to_string()),
        create_success_response(ProviderId::Gemini, sample_content_no_attributes(), 50),
    ]
}
