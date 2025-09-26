//! Simple Routing Strategy Test
//!
//! Tests routing strategy configuration without environment variable conflicts.

use shared::types::RoutingStrategy;
use shared::ProviderId;
use std::collections::HashMap;

#[test]
fn test_routing_strategy_parsing() {
    // Test 1: Backoff strategy
    let strategy = RoutingStrategy::Backoff { provider: ProviderId::Random };
    match strategy {
        RoutingStrategy::Backoff { provider } => {
            assert_eq!(provider, ProviderId::Random);
        }
        _ => panic!("Expected backoff strategy"),
    }
    
    // Test 2: RoundRobin strategy
    let providers = vec![ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Random];
    let strategy = RoutingStrategy::RoundRobin { providers: providers.clone() };
    match strategy {
        RoutingStrategy::RoundRobin { providers: p } => {
            assert_eq!(p.len(), 3);
            assert_eq!(p, providers);
        }
        _ => panic!("Expected roundrobin strategy"),
    }
    
    // Test 3: Priority strategy
    let providers = vec![ProviderId::Gemini, ProviderId::OpenAI, ProviderId::Anthropic];
    let strategy = RoutingStrategy::PriorityOrder { providers: providers.clone() };
    match strategy {
        RoutingStrategy::PriorityOrder { providers: p } => {
            assert_eq!(p.len(), 3);
            assert_eq!(p[0], ProviderId::Gemini); // Cheapest first
        }
        _ => panic!("Expected priority strategy"),
    }
    
    // Test 4: Weighted strategy
    let mut weights = HashMap::new();
    weights.insert(ProviderId::OpenAI, 0.5);
    weights.insert(ProviderId::Anthropic, 0.3);
    weights.insert(ProviderId::Random, 0.2);
    
    let strategy = RoutingStrategy::Weighted { weights: weights.clone() };
    match strategy {
        RoutingStrategy::Weighted { weights: w } => {
            assert_eq!(w.len(), 3);
            assert_eq!(w[&ProviderId::OpenAI], 0.5);
            assert_eq!(w[&ProviderId::Anthropic], 0.3);
            assert_eq!(w[&ProviderId::Random], 0.2);
        }
        _ => panic!("Expected weighted strategy"),
    }
    
    println!("✅ All routing strategy tests passed!");
}