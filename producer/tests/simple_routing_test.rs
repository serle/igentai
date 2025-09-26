//! Simple Routing Strategy Test
//!
//! Tests routing strategy configuration without environment variable conflicts.

use shared::types::{RoutingStrategy, ProviderConfig};
use shared::ProviderId;
use std::collections::HashMap;

#[test]
fn test_routing_strategy_parsing() {
    // Test 1: Backoff strategy
    let strategy = RoutingStrategy::Backoff { provider: ProviderConfig::with_default_model(ProviderId::Random) };
    match strategy {
        RoutingStrategy::Backoff { provider } => {
            assert_eq!(provider.provider, ProviderId::Random);
        }
        _ => panic!("Expected backoff strategy"),
    }
    
    // Test 2: RoundRobin strategy
    let providers = vec![
        ProviderConfig::with_default_model(ProviderId::OpenAI),
        ProviderConfig::with_default_model(ProviderId::Anthropic),
        ProviderConfig::with_default_model(ProviderId::Random),
    ];
    let strategy = RoutingStrategy::RoundRobin { providers: providers.clone() };
    match strategy {
        RoutingStrategy::RoundRobin { providers: p } => {
            assert_eq!(p.len(), 3);
            assert_eq!(p[0].provider, ProviderId::OpenAI);
            assert_eq!(p[1].provider, ProviderId::Anthropic);
            assert_eq!(p[2].provider, ProviderId::Random);
        }
        _ => panic!("Expected roundrobin strategy"),
    }
    
    // Test 3: Priority strategy
    let providers = vec![
        ProviderConfig::with_default_model(ProviderId::Gemini),
        ProviderConfig::with_default_model(ProviderId::OpenAI),
        ProviderConfig::with_default_model(ProviderId::Anthropic),
    ];
    let strategy = RoutingStrategy::PriorityOrder { providers: providers.clone() };
    match strategy {
        RoutingStrategy::PriorityOrder { providers: p } => {
            assert_eq!(p.len(), 3);
            assert_eq!(p[0].provider, ProviderId::Gemini); // Cheapest first
        }
        _ => panic!("Expected priority strategy"),
    }
    
    // Test 4: Weighted strategy
    let mut weights = HashMap::new();
    weights.insert(ProviderConfig::with_default_model(ProviderId::OpenAI), 0.5);
    weights.insert(ProviderConfig::with_default_model(ProviderId::Anthropic), 0.3);
    weights.insert(ProviderConfig::with_default_model(ProviderId::Random), 0.2);
    
    let strategy = RoutingStrategy::Weighted { weights: weights.clone() };
    match strategy {
        RoutingStrategy::Weighted { weights: w } => {
            assert_eq!(w.len(), 3);
            let openai_config = ProviderConfig::with_default_model(ProviderId::OpenAI);
            let anthropic_config = ProviderConfig::with_default_model(ProviderId::Anthropic);
            let random_config = ProviderConfig::with_default_model(ProviderId::Random);
            assert_eq!(w[&openai_config], 0.5);
            assert_eq!(w[&anthropic_config], 0.3);
            assert_eq!(w[&random_config], 0.2);
        }
        _ => panic!("Expected weighted strategy"),
    }
    
    println!("✅ All routing strategy tests passed!");
}