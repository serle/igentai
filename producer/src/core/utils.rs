//! Pure utility functions for producer operations

use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::types::{ApiRequest, ApiResponse};
use shared::types::{GenerationConfig, RoutingStrategy};
use shared::ProviderId;

/// Auto-select optimal routing strategy based on available providers (pure function)
pub fn auto_select_routing_strategy(
    available_providers: &[ProviderId],
    override_strategy: Option<RoutingStrategy>,
) -> RoutingStrategy {
    // If explicit strategy provided, use it
    if let Some(strategy) = override_strategy {
        return strategy;
    }

    // Auto-select based on number of providers
    match available_providers.len() {
        0 => RoutingStrategy::Backoff {
            provider: ProviderId::Random,
        },
        1 => RoutingStrategy::Backoff {
            provider: available_providers[0],
        },
        _ => RoutingStrategy::RoundRobin {
            providers: available_providers.to_vec(),
        },
    }
}

/// Select provider based on routing strategy (pure function)
pub fn select_provider(routing_strategy: &Option<RoutingStrategy>, fallback: ProviderId) -> ProviderId {
    match routing_strategy {
        Some(RoutingStrategy::RoundRobin { providers }) if !providers.is_empty() => {
            let index = (Utc::now().timestamp_millis() / 1000) as usize % providers.len();
            providers[index]
        }
        Some(RoutingStrategy::Backoff { provider }) => *provider,
        Some(RoutingStrategy::PriorityOrder { providers }) => providers.first().copied().unwrap_or(fallback),
        Some(RoutingStrategy::Weighted { weights }) => select_weighted_provider(weights).unwrap_or(fallback),
        Some(RoutingStrategy::RoundRobin { .. }) => fallback, // Empty providers case
        None => fallback,
    }
}

/// Select provider based on weights (pure function)
pub fn select_weighted_provider(weights: &HashMap<ProviderId, f32>) -> Option<ProviderId> {
    let total_weight: f32 = weights.values().sum();
    if total_weight == 0.0 {
        return None;
    }

    let random_value = (Utc::now().timestamp_millis() % 1000) as f32;
    let target = random_value % total_weight;

    let mut accumulated = 0.0;
    for (provider, weight) in weights {
        accumulated += weight;
        if target < accumulated {
            return Some(*provider);
        }
    }
    None
}

/// Build API request from routing strategy and generation config (pure function)
pub fn build_api_request(
    routing_strategy: &Option<RoutingStrategy>,
    generation_config: &Option<GenerationConfig>,
    enhanced_prompt: String,
    request_id: Uuid,
) -> ApiRequest {
    let provider = select_provider(routing_strategy, ProviderId::Random);

    let (max_tokens, temperature) = generation_config
        .as_ref()
        .map(|gc| (gc.max_tokens, gc.temperature))
        .unwrap_or((150, 0.7));

    ApiRequest {
        provider,
        prompt: enhanced_prompt,
        max_tokens,
        temperature,
        request_id,
        timestamp: Utc::now(),
    }
}

/// Process API response and extract business logic (pure function)
pub fn should_retry_request(response: &ApiResponse, attempt: u32, max_retries: u32) -> Option<Duration> {
    if attempt >= max_retries || response.success {
        return None;
    }

    let error_msg = response.error_message.as_deref().unwrap_or("");
    let is_retryable = error_msg.contains("rate limit") || error_msg.contains("timeout") || error_msg.contains("503");

    if is_retryable {
        Some(Duration::from_millis(100 * (1 << attempt))) // Exponential backoff
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_provider_strategies() {
        // Test backoff strategy
        let backoff_strategy = Some(RoutingStrategy::Backoff {
            provider: ProviderId::Anthropic,
        });
        assert_eq!(
            select_provider(&backoff_strategy, ProviderId::OpenAI),
            ProviderId::Anthropic
        );

        // Test round robin strategy
        let round_robin_strategy = Some(RoutingStrategy::RoundRobin {
            providers: vec![ProviderId::Gemini, ProviderId::OpenAI],
        });
        let selected = select_provider(&round_robin_strategy, ProviderId::Random);
        assert!(selected == ProviderId::Gemini || selected == ProviderId::OpenAI);

        // Test no strategy (fallback)
        let no_strategy: Option<RoutingStrategy> = None;
        assert_eq!(select_provider(&no_strategy, ProviderId::Random), ProviderId::Random);
    }

    #[test]
    fn test_should_retry_request() {
        let mut response = ApiResponse {
            provider: ProviderId::OpenAI,
            request_id: Uuid::new_v4(),
            content: "".to_string(),
            tokens_used: 0,
            response_time_ms: 0,
            timestamp: Utc::now(),
            success: false,
            error_message: Some("rate limit exceeded".to_string()),
        };

        // Should retry on rate limit
        assert!(should_retry_request(&response, 0, 3).is_some());

        // Should not retry after max attempts
        assert!(should_retry_request(&response, 3, 3).is_none());

        // Should not retry on success
        response.success = true;
        assert!(should_retry_request(&response, 0, 3).is_none());
    }

    #[test]
    fn test_auto_select_routing_strategy() {
        // Single provider should use backoff
        let single_provider = vec![ProviderId::OpenAI];
        let strategy = auto_select_routing_strategy(&single_provider, None);
        match strategy {
            RoutingStrategy::Backoff { provider } => assert_eq!(provider, ProviderId::OpenAI),
            _ => panic!("Expected backoff strategy for single provider"),
        }

        // Multiple providers should use round robin
        let multiple_providers = vec![ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini];
        let strategy = auto_select_routing_strategy(&multiple_providers, None);
        match strategy {
            RoutingStrategy::RoundRobin { providers } => {
                assert_eq!(providers.len(), 3);
                assert!(providers.contains(&ProviderId::OpenAI));
            }
            _ => panic!("Expected round robin strategy for multiple providers"),
        }

        // No providers should default to Random with backoff
        let no_providers = vec![];
        let strategy = auto_select_routing_strategy(&no_providers, None);
        match strategy {
            RoutingStrategy::Backoff { provider } => assert_eq!(provider, ProviderId::Random),
            _ => panic!("Expected backoff strategy with Random fallback"),
        }

        // Override should be respected
        let override_strategy = Some(RoutingStrategy::Weighted {
            weights: HashMap::from([(ProviderId::OpenAI, 100.0)]),
        });
        let strategy = auto_select_routing_strategy(&multiple_providers, override_strategy.clone());
        match strategy {
            RoutingStrategy::Weighted { .. } => {} // Expected
            _ => panic!("Expected override strategy to be used"),
        }
    }
}
