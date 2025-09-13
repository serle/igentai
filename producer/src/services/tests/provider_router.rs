//! Tests for ProviderRouter service

use std::collections::HashMap;

use shared::{ApiFailure, ProviderId, RoutingStrategy};
use crate::services::provider_router::RealProviderRouter;
use crate::traits::ProviderRouter;
use crate::error::ProducerError;

#[tokio::test]
async fn test_provider_router_creation() {
    let router = RealProviderRouter::new();
    
    // Should be able to select a provider (default weighted strategy)
    let provider = router.select_provider().await.unwrap();
    assert!(matches!(provider, ProviderId::OpenAI | ProviderId::Anthropic | ProviderId::Gemini));
}

#[tokio::test]
async fn test_set_api_keys() {
    let router = RealProviderRouter::new();
    
    let mut api_keys = HashMap::new();
    api_keys.insert(ProviderId::OpenAI, "test-key".to_string());
    
    let result = router.set_api_keys(api_keys).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_routing_strategy_weighted() {
    let router = RealProviderRouter::new();
    
    let mut weights = HashMap::new();
    weights.insert(ProviderId::OpenAI, 1.0);
    weights.insert(ProviderId::Anthropic, 0.0);
    weights.insert(ProviderId::Gemini, 0.0);
    
    let strategy = RoutingStrategy::Weighted { weights };
    router.set_routing_strategy(strategy).await.unwrap();
    
    // Should always select OpenAI with weight 1.0 and others 0.0
    for _ in 0..10 {
        let provider = router.select_provider().await.unwrap();
        assert_eq!(provider, ProviderId::OpenAI);
    }
}

#[tokio::test]
async fn test_routing_strategy_priority_order() {
    let router = RealProviderRouter::new();
    
    let providers = vec![ProviderId::Anthropic, ProviderId::Gemini, ProviderId::OpenAI];
    let strategy = RoutingStrategy::PriorityOrder { providers };
    router.set_routing_strategy(strategy).await.unwrap();
    
    // Should always select first provider (Anthropic)
    for _ in 0..5 {
        let provider = router.select_provider().await.unwrap();
        assert_eq!(provider, ProviderId::Anthropic);
    }
}

#[tokio::test]
async fn test_routing_strategy_round_robin() {
    let router = RealProviderRouter::new();
    
    let providers = vec![ProviderId::OpenAI, ProviderId::Anthropic];
    let strategy = RoutingStrategy::RoundRobin { providers };
    router.set_routing_strategy(strategy).await.unwrap();
    
    // Should alternate between providers
    let first = router.select_provider().await.unwrap();
    let second = router.select_provider().await.unwrap();
    let third = router.select_provider().await.unwrap();
    
    assert_eq!(first, ProviderId::OpenAI);
    assert_eq!(second, ProviderId::Anthropic);
    assert_eq!(third, ProviderId::OpenAI);
}

#[tokio::test]
async fn test_empty_routing_strategy_error() {
    let router = RealProviderRouter::new();
    
    // Test empty weighted strategy
    let strategy = RoutingStrategy::Weighted { weights: HashMap::new() };
    router.set_routing_strategy(strategy).await.unwrap();
    
    let result = router.select_provider().await;
    assert!(matches!(result, Err(ProducerError::ConfigError { .. })));
    
    // Test empty priority order strategy
    let strategy = RoutingStrategy::PriorityOrder { providers: vec![] };
    router.set_routing_strategy(strategy).await.unwrap();
    
    let result = router.select_provider().await;
    assert!(matches!(result, Err(ProducerError::ConfigError { .. })));
    
    // Test empty round-robin strategy
    let strategy = RoutingStrategy::RoundRobin { providers: vec![] };
    router.set_routing_strategy(strategy).await.unwrap();
    
    let result = router.select_provider().await;
    assert!(matches!(result, Err(ProducerError::ConfigError { .. })));
}

#[tokio::test]
async fn test_generation_config_updates() {
    let router = RealProviderRouter::new();
    
    let mut config = shared::GenerationConfig::default();
    config.model = "gpt-4".to_string();
    config.temperature = 0.5;
    config.max_tokens = 500;
    
    let result = router.set_generation_config(config.clone()).await;
    assert!(result.is_ok());
    
    // Verify config was updated
    let retrieved_config = router.get_request_config(&ProviderId::OpenAI).await;
    assert_eq!(retrieved_config.model, "gpt-4");
    assert_eq!(retrieved_config.temperature, 0.5);
    assert_eq!(retrieved_config.max_tokens, 500);
}

#[tokio::test]
async fn test_prompt_management() {
    let router = RealProviderRouter::new();
    
    let test_prompt = "Generate creative attributes for the topic:".to_string();
    let result = router.set_prompt(test_prompt.clone()).await;
    assert!(result.is_ok());
}

#[tokio::test] 
async fn test_provider_stats_empty() {
    let router = RealProviderRouter::new();
    
    let stats = router.get_provider_stats().await.unwrap();
    assert!(stats.is_empty()); // Should be empty initially
}