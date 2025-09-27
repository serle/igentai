//! Pure utility functions for producer operations

use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::types::{ApiRequest, ApiResponse};
use shared::types::{GenerationConfig, RoutingStrategy, ProviderConfig};
use shared::ProviderId;

/// Load routing strategy from environment variables with fallback
/// This replaces the old auto-selection logic with explicit environment configuration
pub fn load_routing_strategy() -> RoutingStrategy {
    RoutingStrategy::from_env().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load routing strategy from environment: {}. Using default backoff to random.", e);
        RoutingStrategy::Backoff { provider: ProviderConfig::with_default_model(ProviderId::Random) }
    })
}

/// Select provider based on routing strategy (pure function)
pub fn select_provider(routing_strategy: &Option<RoutingStrategy>, fallback: ProviderId) -> ProviderId {
    match routing_strategy {
        Some(RoutingStrategy::RoundRobin { providers }) if !providers.is_empty() => {
            let index = (Utc::now().timestamp_millis() / 1000) as usize % providers.len();
            providers[index].provider
        }
        Some(RoutingStrategy::Backoff { provider }) => provider.provider,
        Some(RoutingStrategy::PriorityOrder { providers }) => providers.first().map(|p| p.provider).unwrap_or(fallback),
        Some(RoutingStrategy::Weighted { weights }) => select_weighted_provider_config(weights).map(|pc| pc.provider).unwrap_or(fallback),
        Some(RoutingStrategy::RoundRobin { .. }) => fallback, // Empty providers case
        None => fallback,
    }
}

/// Select provider config based on routing strategy (pure function)
pub fn select_provider_config(routing_strategy: &Option<RoutingStrategy>, fallback: ProviderConfig) -> ProviderConfig {
    match routing_strategy {
        Some(RoutingStrategy::RoundRobin { providers }) if !providers.is_empty() => {
            let index = (Utc::now().timestamp_millis() / 1000) as usize % providers.len();
            providers[index].clone()
        }
        Some(RoutingStrategy::Backoff { provider }) => provider.clone(),
        Some(RoutingStrategy::PriorityOrder { providers }) => providers.first().cloned().unwrap_or(fallback),
        Some(RoutingStrategy::Weighted { weights }) => select_weighted_provider_config(weights).unwrap_or(fallback),
        Some(RoutingStrategy::RoundRobin { .. }) => fallback, // Empty providers case
        None => fallback,
    }
}

/// Select provider based on weights (pure function) - legacy support
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

/// Select provider config based on weights (pure function)
pub fn select_weighted_provider_config(weights: &HashMap<ProviderConfig, f32>) -> Option<ProviderConfig> {
    let total_weight: f32 = weights.values().sum();
    if total_weight == 0.0 {
        return None;
    }

    let random_value = (Utc::now().timestamp_millis() % 1000) as f32;
    let target = random_value % total_weight;

    let mut accumulated = 0.0;
    for (provider_config, weight) in weights {
        accumulated += weight;
        if target < accumulated {
            return Some(provider_config.clone());
        }
    }
    None
}

/// Build API request from provider config and generation config (pure function)
pub fn build_api_request_with_config(
    provider_config: &ProviderConfig,
    generation_config: &Option<GenerationConfig>,
    enhanced_prompt: String,
    request_id: Uuid,
) -> ApiRequest {
    let (max_tokens, temperature) = generation_config
        .as_ref()
        .map(|gc| (gc.max_tokens, gc.temperature))
        .unwrap_or((150, 0.7));

    ApiRequest {
        provider: provider_config.provider,
        prompt: enhanced_prompt,
        max_tokens,
        temperature,
        request_id,
        timestamp: Utc::now(),
    }
}

/// Build API request from routing strategy and generation config (pure function) - legacy support
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
    use shared::TokenUsage;

    #[test]
    fn test_select_provider_strategies() {
        // Test backoff strategy
        let backoff_strategy = Some(RoutingStrategy::Backoff {
            provider: ProviderConfig::with_default_model(ProviderId::Anthropic),
        });
        assert_eq!(
            select_provider(&backoff_strategy, ProviderId::OpenAI),
            ProviderId::Anthropic
        );

        // Test round robin strategy
        let round_robin_strategy = Some(RoutingStrategy::RoundRobin {
            providers: vec![
                ProviderConfig::with_default_model(ProviderId::Gemini),
                ProviderConfig::with_default_model(ProviderId::OpenAI),
            ],
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
            tokens_used: TokenUsage { input_tokens: 0, output_tokens: 0 },
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
    fn test_load_routing_strategy() {
        // Test environment-based routing strategy loading
        // In test mode (cfg!(test) = true), it should default to Random provider
        let strategy = load_routing_strategy();
        match strategy {
            RoutingStrategy::Backoff { provider } => {
                // In test mode, should use Random provider regardless of environment
                assert_eq!(provider.provider, ProviderId::Random);
            }
            _ => panic!("Expected backoff strategy in test mode"),
        }
    }
}
