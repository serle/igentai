//! Unit tests for routing strategy provider selection

use producer::core::producer::Producer;
use shared::types::{RoutingStrategy, ProviderId, GenerationConfig};
use std::collections::HashMap;

/// Test provider selection for different routing strategies
#[test]
fn test_routing_strategy_provider_selection() {
    // Test RoundRobin
    let round_robin = RoutingStrategy::RoundRobin {
        providers: vec![ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini],
    };
    let provider = Producer::<(), ()>::select_provider(&Some(round_robin), &None);
    // Should be one of the providers (deterministic based on timestamp modulo)
    assert!(matches!(provider, ProviderId::OpenAI | ProviderId::Anthropic | ProviderId::Gemini));
    
    // Test Weighted
    let mut weights = HashMap::new();
    weights.insert(ProviderId::OpenAI, 0.7);
    weights.insert(ProviderId::Anthropic, 0.3);
    let weighted = RoutingStrategy::Weighted { weights };
    let provider = Producer::<(), ()>::select_provider(&Some(weighted), &None);
    // Should be OpenAI or Anthropic based on weights
    assert!(matches!(provider, ProviderId::OpenAI | ProviderId::Anthropic));
    
    // Test PriorityOrder
    let priority = RoutingStrategy::PriorityOrder {
        providers: vec![ProviderId::Gemini, ProviderId::OpenAI],
    };
    let provider = Producer::<(), ()>::select_provider(&Some(priority), &None);
    // Should always pick first provider
    assert_eq!(provider, ProviderId::Gemini);
    
    // Test Backoff
    let backoff = RoutingStrategy::Backoff {
        provider: ProviderId::Anthropic,
    };
    let provider = Producer::<(), ()>::select_provider(&Some(backoff), &None);
    // Should always return the specified provider
    assert_eq!(provider, ProviderId::Anthropic);
    
    // Test None (default)
    let provider = Producer::<(), ()>::select_provider(&None, &None);
    assert_eq!(provider, ProviderId::OpenAI);
}

/// Test that Backoff strategy always returns the same provider
#[test]  
fn test_backoff_consistency() {
    let backoff_openai = RoutingStrategy::Backoff {
        provider: ProviderId::OpenAI,
    };
    
    let backoff_anthropic = RoutingStrategy::Backoff {
        provider: ProviderId::Anthropic,
    };
    
    let backoff_gemini = RoutingStrategy::Backoff {
        provider: ProviderId::Gemini,
    };
    
    // Test multiple calls return same provider
    for _ in 0..10 {
        assert_eq!(
            Producer::<(), ()>::select_provider(&Some(backoff_openai.clone()), &None),
            ProviderId::OpenAI
        );
        
        assert_eq!(
            Producer::<(), ()>::select_provider(&Some(backoff_anthropic.clone()), &None),
            ProviderId::Anthropic
        );
        
        assert_eq!(
            Producer::<(), ()>::select_provider(&Some(backoff_gemini.clone()), &None),
            ProviderId::Gemini
        );
    }
}

/// Test empty providers handling
#[test]
fn test_empty_providers_fallback() {
    // Empty RoundRobin should fallback to OpenAI
    let empty_round_robin = RoutingStrategy::RoundRobin {
        providers: vec![],
    };
    let provider = Producer::<(), ()>::select_provider(&Some(empty_round_robin), &None);
    assert_eq!(provider, ProviderId::OpenAI);
    
    // Empty PriorityOrder should fallback to OpenAI
    let empty_priority = RoutingStrategy::PriorityOrder {
        providers: vec![],
    };
    let provider = Producer::<(), ()>::select_provider(&Some(empty_priority), &None);
    assert_eq!(provider, ProviderId::OpenAI);
    
    // Empty Weighted should fallback to OpenAI
    let empty_weighted = RoutingStrategy::Weighted {
        weights: HashMap::new(),
    };
    let provider = Producer::<(), ()>::select_provider(&Some(empty_weighted), &None);
    assert_eq!(provider, ProviderId::OpenAI);
}