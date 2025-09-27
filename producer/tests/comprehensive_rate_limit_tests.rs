//! Comprehensive rate limiting functionality tests
//! 
//! This file consolidates all rate limiting tests including:
//! - Rate limit detection and parsing (all providers)
//! - Exponential backoff algorithms  
//! - Provider-specific rate limit handling
//! - Edge cases and error scenarios

use producer::services::api_client::RealApiClient;
use shared::ProviderId;
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use reqwest::header::{HeaderMap, HeaderValue};
use chrono::Utc;

/// Create a test API client for rate limit testing
fn create_test_client() -> RealApiClient {
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::OpenAI, "test-key".to_string());
    api_keys.insert(ProviderId::Anthropic, "test-key".to_string());
    api_keys.insert(ProviderId::Gemini, "test-key".to_string());
    RealApiClient::new(api_keys, 5000)
}

#[cfg(test)]
mod backoff_extraction_tests {
    use super::*;

    #[test]
    fn test_openai_rate_limit_parsing() {
        let client = create_test_client();
        
        let test_cases = vec![
            ("Please try again in 442ms", Some(442)),
            ("Please try again in 1000ms", Some(1000)),
            ("Please try again in 50ms", Some(50)),
            ("Please try again in 2500ms", Some(2500)),
            ("Please try again later", None),
            ("Rate limit exceeded", None),
            ("Please try again in 0ms", Some(0)),
        ];
        
        for (message, expected) in test_cases {
            let error = json!({
                "error": {
                    "message": message,
                    "type": "tokens",
                    "code": "rate_limit_exceeded"
                }
            });
            let body = error.to_string();
            let headers = HeaderMap::new();
            
            let backoff = client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, &body);
            assert_eq!(backoff, expected, "Failed for OpenAI message: {}", message);
        }
    }

    #[test]  
    fn test_openai_complex_error_format() {
        let client = create_test_client();
        
        let openai_error = json!({
            "error": {
                "message": "Rate limit reached for gpt-4o-mini in project proj_123 on tokens per min (TPM): Limit 60000, Used 60000, Requested 442. Please try again in 442ms.",
                "type": "tokens",
                "param": null,
                "code": "rate_limit_exceeded"
            }
        });
        
        let body = openai_error.to_string();
        let headers = HeaderMap::new();
        
        let backoff = client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, &body);
        assert_eq!(backoff, Some(442));
    }

    #[test]
    fn test_anthropic_retry_after_header() {
        let client = create_test_client();
        
        let test_cases = vec![
            ("5", Some(5000)),   // 5 seconds -> 5000ms
            ("10", Some(10000)), // 10 seconds -> 10000ms
            ("60", Some(60000)), // 60 seconds -> 60000ms
            ("0", Some(0)),      // 0 seconds -> 0ms
        ];
        
        for (header_value, expected) in test_cases {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", HeaderValue::from_static(header_value));
            
            let backoff = client.extract_backoff_ms(ProviderId::Anthropic, 429, &headers, "");
            assert_eq!(backoff, expected, "Failed for Anthropic retry-after: {}", header_value);
        }
    }

    #[test]
    fn test_anthropic_ratelimit_reset_header() {
        let client = create_test_client();
        
        // Test future timestamp
        let future_timestamp = (Utc::now().timestamp() + 15).to_string();
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-ratelimit-tokens-reset", HeaderValue::from_str(&future_timestamp).unwrap());
        
        let backoff = client.extract_backoff_ms(ProviderId::Anthropic, 429, &headers, "");
        assert!(backoff.is_some());
        let backoff_ms = backoff.unwrap();
        
        // Should be around 15 seconds (15000ms), allow tolerance for timing
        assert!(backoff_ms >= 14000 && backoff_ms <= 16000, "Backoff should be ~15s, got {}ms", backoff_ms);
        
        // Test past timestamp (should get minimum delay)
        let past_timestamp = (Utc::now().timestamp() - 5).to_string();
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-ratelimit-tokens-reset", HeaderValue::from_str(&past_timestamp).unwrap());
        
        let backoff = client.extract_backoff_ms(ProviderId::Anthropic, 429, &headers, "");
        assert_eq!(backoff, Some(1000)); // Minimum 1 second
    }

    #[test]
    fn test_gemini_retry_after_header() {
        let client = create_test_client();
        
        let test_cases = vec![
            ("30", Some(30000)), // 30 seconds -> 30000ms
            ("60", Some(60000)), // 60 seconds -> 60000ms
            ("1", Some(1000)),   // 1 second -> 1000ms
        ];
        
        for (header_value, expected) in test_cases {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", HeaderValue::from_static(header_value));
            
            let backoff = client.extract_backoff_ms(ProviderId::Gemini, 429, &headers, "");
            assert_eq!(backoff, expected, "Failed for Gemini retry-after: {}", header_value);
        }
    }

    #[test]
    fn test_random_provider_no_rate_limits() {
        let client = create_test_client();
        
        // Random provider should never have rate limits, even with headers/body
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("30"));
        
        let error_body = json!({
            "error": {"message": "Please try again in 1000ms"}
        }).to_string();
        
        let backoff = client.extract_backoff_ms(ProviderId::Random, 429, &headers, &error_body);
        assert_eq!(backoff, None, "Random provider should never have rate limit backoff");
    }

    #[test]
    fn test_no_rate_limit_info() {
        let client = create_test_client();
        
        let headers = HeaderMap::new();
        let empty_body = "";
        
        // All providers should return None when no rate limit info is available
        for provider in [ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini] {
            let backoff = client.extract_backoff_ms(provider, 429, &headers, empty_body);
            assert_eq!(backoff, None, "Provider {:?} should return None without rate limit info", provider);
        }
    }
}

#[cfg(test)]
mod exponential_backoff_tests {
    use super::*;

    #[test]
    fn test_exponential_backoff_progression() {
        let client = create_test_client();
        
        // Test exponential progression with jitter
        let attempts = vec![0, 1, 2, 3, 4];
        let expected_base_delays = vec![1000, 2000, 4000, 8000, 16000]; // Base delays in ms
        
        for (attempt, expected_base) in attempts.into_iter().zip(expected_base_delays.into_iter()) {
            let backoff = client.calculate_exponential_backoff_ms(attempt);
            
            // Allow 10% jitter on either side
            let min_expected = (expected_base as f64 * 0.9) as u32;
            let max_expected = (expected_base as f64 * 1.1) as u32;
            
            assert!(
                backoff >= min_expected && backoff <= max_expected,
                "Attempt {} should be ~{}ms (±10%), got {}ms",
                attempt, expected_base, backoff
            );
        }
    }

    #[test]
    fn test_exponential_backoff_max_limit() {
        let client = create_test_client();
        
        // Test that backoff doesn't exceed maximum (60 seconds + jitter)
        let large_attempts = vec![10, 15, 20, 100];
        
        for attempt in large_attempts {
            let backoff = client.calculate_exponential_backoff_ms(attempt);
            assert!(
                backoff <= 66000, // 60000ms + 10% jitter
                "Backoff for attempt {} should not exceed 66s, got {}ms",
                attempt, backoff
            );
        }
    }

    #[test]
    fn test_exponential_backoff_jitter() {
        let client = create_test_client();
        
        // Generate multiple values for same attempt to verify jitter
        let mut values = Vec::new();
        for _ in 0..10 {
            values.push(client.calculate_exponential_backoff_ms(1));
        }
        
        // All values should be different due to jitter
        let unique_values: std::collections::HashSet<_> = values.iter().collect();
        assert!(unique_values.len() > 1, "Jitter should produce different values, got: {:?}", values);
        
        // All values should be in reasonable range around 2000ms
        for value in values {
            assert!(
                value >= 1800 && value <= 2200,
                "Value {} should be in range 1800-2200ms", value
            );
        }
    }

    #[test]
    fn test_exponential_backoff_minimum() {
        let client = create_test_client();
        
        // First attempt should have reasonable minimum
        let backoff = client.calculate_exponential_backoff_ms(0);
        assert!(
            backoff >= 900 && backoff <= 1100,
            "First attempt should be ~1000ms ±10%, got {}ms", backoff
        );
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_malformed_json_handling() {
        let client = create_test_client();
        
        let malformed_cases = vec![
            "{invalid json}",
            "not json at all",
            "{\"error\": {}}",
            "{\"error\": {\"message\": null}}",
            "",
        ];
        
        for malformed_json in malformed_cases {
            let headers = HeaderMap::new();
            let backoff = client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, malformed_json);
            assert_eq!(backoff, None, "Malformed JSON should return None: {}", malformed_json);
        }
    }

    #[test]
    fn test_invalid_header_values() {
        let client = create_test_client();
        
        let invalid_headers = vec![
            "invalid",
            "not_a_number", 
            "-5",
            "999999999999999999999", // Very large number
            "",
        ];
        
        for invalid_value in invalid_headers {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", HeaderValue::from_str(invalid_value).unwrap_or(HeaderValue::from_static("0")));
            
            let backoff = client.extract_backoff_ms(ProviderId::Anthropic, 429, &headers, "");
            // Should either return None or handle gracefully
            if let Some(backoff_val) = backoff {
                assert!(backoff_val <= 600000, "Backoff should be reasonable even for invalid input"); // Max 10 minutes
            }
        }
    }

    #[test]
    fn test_missing_error_fields() {
        let client = create_test_client();
        
        let incomplete_errors = vec![
            json!({}),
            json!({"error": {}}),
            json!({"error": {"type": "rate_limit"}}),
            json!({"error": {"message": null}}),
            json!({"other_field": "value"}),
        ];
        
        for incomplete_error in incomplete_errors {
            let body = incomplete_error.to_string();
            let headers = HeaderMap::new();
            
            let backoff = client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, &body);
            assert_eq!(backoff, None, "Incomplete error should return None: {}", body);
        }
    }

    #[test]
    fn test_zero_and_negative_delays() {
        let client = create_test_client();
        
        // Test zero delay
        let error = json!({"error": {"message": "Please try again in 0ms"}});
        let body = error.to_string();
        let headers = HeaderMap::new();
        
        let backoff = client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, &body);
        assert_eq!(backoff, Some(0));
        
        // Test past timestamp for Anthropic
        let mut headers = HeaderMap::new();
        headers.insert("anthropic-ratelimit-tokens-reset", HeaderValue::from_static("0"));
        
        let backoff = client.extract_backoff_ms(ProviderId::Anthropic, 429, &headers, "");
        assert_eq!(backoff, Some(1000)); // Should be minimum 1 second
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_backoff_extraction_performance() {
        let client = create_test_client();
        
        // Test with large response body
        let large_error = json!({
            "error": {
                "message": format!("Please try again in 500ms. {}", "a".repeat(10000)),
                "large_field": "x".repeat(50000)
            }
        });
        let body = large_error.to_string();
        let headers = HeaderMap::new();
        
        let start = Instant::now();
        for _ in 0..1000 {
            client.extract_backoff_ms(ProviderId::OpenAI, 429, &headers, &body);
        }
        let elapsed = start.elapsed();
        
        // Should be reasonably fast (less than 500ms for 1000 operations)
        assert!(elapsed < Duration::from_millis(500), "Extraction should be fast, took {:?}", elapsed);
    }

    #[test]
    fn test_exponential_backoff_performance() {
        let client = create_test_client();
        
        let start = Instant::now();
        for attempt in 0..1000 {
            client.calculate_exponential_backoff_ms(attempt % 10);
        }
        let elapsed = start.elapsed();
        
        // Should be very fast
        assert!(elapsed < Duration::from_millis(50), "Backoff calculation should be fast, took {:?}", elapsed);
    }
}

#[cfg(test)]
mod provider_specific_tests {
    use super::*;

    #[test]
    fn test_all_providers_rate_limit_formats() {
        let client = create_test_client();
        
        // Test each provider with their specific format
        let test_cases = vec![
            (
                ProviderId::OpenAI,
                json!({"error": {"message": "Please try again in 123ms"}}).to_string(),
                HeaderMap::new(),
                Some(123)
            ),
            (
                ProviderId::Anthropic,
                String::new(),
                {
                    let mut h = HeaderMap::new();
                    h.insert("retry-after", HeaderValue::from_static("5"));
                    h
                },
                Some(5000)
            ),
            (
                ProviderId::Gemini,
                String::new(),
                {
                    let mut h = HeaderMap::new();
                    h.insert("retry-after", HeaderValue::from_static("30"));
                    h
                },
                Some(30000)
            ),
            (
                ProviderId::Random,
                json!({"error": {"message": "Please try again in 999ms"}}).to_string(),
                {
                    let mut h = HeaderMap::new();
                    h.insert("retry-after", HeaderValue::from_static("999"));
                    h
                },
                None // Random provider never has rate limits
            ),
        ];
        
        for (provider, body, headers, expected) in test_cases {
            let result = client.extract_backoff_ms(provider, 429, &headers, &body);
            assert_eq!(result, expected, "Failed for provider {:?}", provider);
        }
    }

    #[test]
    fn test_provider_fallback_behavior() {
        let client = create_test_client();
        
        // Test when provider-specific parsing fails, should return None
        let providers = [ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini];
        
        for provider in providers {
            let headers = HeaderMap::new();
            let invalid_body = "not a valid response";
            
            let backoff = client.extract_backoff_ms(provider, 429, &headers, invalid_body);
            assert_eq!(backoff, None, "Provider {:?} should return None for invalid response", provider);
        }
    }
}

#[cfg(test)]
mod integration_simulation_tests {
    use super::*;

    #[test]
    fn test_rate_limit_workflow_simulation() {
        let client = create_test_client();
        
        // Simulate a complete rate limiting workflow
        
        // 1. Extract backoff from provider response
        let openai_error = json!({
            "error": {"message": "Please try again in 2000ms"}
        });
        let backoff = client.extract_backoff_ms(
            ProviderId::OpenAI, 
            429, 
            &HeaderMap::new(), 
            &openai_error.to_string()
        );
        assert_eq!(backoff, Some(2000));
        
        // 2. If no provider-specific backoff, fall back to exponential
        let no_backoff = client.extract_backoff_ms(
            ProviderId::OpenAI,
            429,
            &HeaderMap::new(),
            ""
        );
        assert_eq!(no_backoff, None);
        
        // Use exponential backoff for attempt 1
        let exp_backoff = client.calculate_exponential_backoff_ms(1);
        assert!(exp_backoff >= 1800 && exp_backoff <= 2200); // ~2000ms ±10%
        
        // 3. Simulate multiple retry attempts
        for attempt in 0..5 {
            let delay = client.calculate_exponential_backoff_ms(attempt);
            assert!(delay > 0, "Delay should be positive for attempt {}", attempt);
            
            // Delays should generally increase (allowing for jitter)
            if attempt > 0 {
                let prev_base = 1000 * (1 << (attempt - 1));
                
                // Current delay should be in ballpark of expected exponential growth
                assert!(delay as u64 >= (prev_base as f64 * 0.8) as u64, 
                       "Delay should grow exponentially");
            }
        }
    }

    #[test]
    fn test_concurrent_rate_limit_handling() {        
        // Simulate multiple threads extracting backoff values concurrently
        let handles: Vec<_> = (0..10).map(|i| {
            let client = create_test_client();
            std::thread::spawn(move || {
                let error = json!({
                    "error": {"message": format!("Please try again in {}ms", 100 + i * 50)}
                });
                let body = error.to_string();
                client.extract_backoff_ms(ProviderId::OpenAI, 429, &HeaderMap::new(), &body)
            })
        }).collect();
        
        let results: Vec<_> = handles.into_iter()
            .map(|h| h.join().unwrap())
            .collect();
        
        // All should succeed and return expected values
        for (i, result) in results.into_iter().enumerate() {
            let expected = 100 + i * 50;
            assert_eq!(result, Some(expected as u32));
        }
    }
}