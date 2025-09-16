//! Tests for Backoff routing strategy

use crate::fixtures::CommandFactory;
use shared::types::ProviderId;

/// Test Backoff routing strategy command creation
#[tokio::test]
async fn test_backoff_routing_strategy() {
    // Test Backoff strategy with OpenAI
    let start_command = CommandFactory::start_command_with_backoff(
        1,
        "Japanese dishes",
        "Generate unique Japanese dishes",
        ProviderId::OpenAI,
    );

    match &start_command {
        shared::ProducerCommand::Start {
            command_id,
            topic,
            prompt,
            routing_strategy,
            generation_config,
        } => {
            assert_eq!(*command_id, 1);
            assert_eq!(topic, "Japanese dishes");
            assert_eq!(prompt, "Generate unique Japanese dishes");

            // Verify Backoff routing strategy
            match routing_strategy {
                shared::types::RoutingStrategy::Backoff { provider } => {
                    assert_eq!(*provider, ProviderId::OpenAI);
                }
                _ => panic!("Expected Backoff routing strategy"),
            }

            // Verify generation config optimized for test mode
            assert_eq!(generation_config.model, "gpt-4o-mini");
            assert_eq!(generation_config.batch_size, 1);
            assert_eq!(generation_config.context_window, 4096);
            assert_eq!(generation_config.max_tokens, 1000);
            assert_eq!(generation_config.temperature, 0.8);
        }
        _ => panic!("Expected Start command"),
    }
}

/// Test Backoff routing strategy with different providers
#[tokio::test]
async fn test_backoff_routing_different_providers() {
    let providers = vec![ProviderId::OpenAI, ProviderId::Anthropic, ProviderId::Gemini];

    for (i, provider) in providers.iter().enumerate() {
        let start_command =
            CommandFactory::start_command_with_backoff(i as u64 + 1, "test topic", "test prompt", *provider);

        match &start_command {
            shared::ProducerCommand::Start { routing_strategy, .. } => match routing_strategy {
                shared::types::RoutingStrategy::Backoff { provider: p } => {
                    assert_eq!(*p, *provider);
                }
                _ => panic!("Expected Backoff routing strategy"),
            },
            _ => panic!("Expected Start command"),
        }
    }
}

/// Test that Backoff strategy is single-provider focused
#[tokio::test]
async fn test_backoff_single_provider_focus() {
    let start_command = CommandFactory::start_command_with_backoff(1, "test", "test prompt", ProviderId::Anthropic);

    match &start_command {
        shared::ProducerCommand::Start { routing_strategy, .. } => {
            match routing_strategy {
                shared::types::RoutingStrategy::Backoff { provider } => {
                    // Backoff should only have one provider
                    assert_eq!(*provider, ProviderId::Anthropic);

                    // Verify it's different from multi-provider strategies
                    // (this is a single provider, not a list)
                }
                _ => panic!("Expected Backoff routing strategy"),
            }
        }
        _ => panic!("Expected Start command"),
    }
}
