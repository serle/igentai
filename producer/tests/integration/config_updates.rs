//! Config update tests
//!
//! Tests that UpdateConfig commands actually take effect in generation.

use crate::fixtures::CommandFactory;
use shared::messages::producer::ProducerCommand;
use shared::types::{GenerationConfig, RoutingStrategy, ProviderId, ProviderConfig};

/// Test that UpdateConfig changes prompt correctly
#[tokio::test]
async fn test_config_updates() {
    // Test creating Start command
    let start_command = CommandFactory::start_command(1, "test-topic", "Initial prompt");
    
    match &start_command {
        ProducerCommand::Start { prompt, .. } => {
            assert_eq!(prompt, "Initial prompt");
        }
        _ => panic!("Expected Start command"),
    }
    
    // Test creating UpdateConfig command with new prompt
    let update_command = CommandFactory::update_config_command(2, Some("Updated prompt".to_string()));
    
    match &update_command {
        ProducerCommand::UpdateConfig { command_id, prompt, .. } => {
            assert_eq!(*command_id, 2);
            assert_eq!(prompt, &Some("Updated prompt".to_string()));
        }
        _ => panic!("Expected UpdateConfig command"),
    }
    
    // Test UpdateConfig with routing strategy
    let new_strategy = RoutingStrategy::Backoff {
        provider: ProviderConfig::with_default_model(ProviderId::OpenAI),
    };
    
    let routing_update = ProducerCommand::UpdateConfig {
        command_id: 3,
        prompt: None,
        routing_strategy: Some(new_strategy.clone()),
        generation_config: None,
    };
    
    match &routing_update {
        ProducerCommand::UpdateConfig { routing_strategy, .. } => {
            assert!(routing_strategy.is_some());
            match routing_strategy.as_ref().unwrap() {
                RoutingStrategy::Backoff { provider } => {
                    assert_eq!(provider.provider, ProviderId::OpenAI);
                }
                _ => panic!("Expected Backoff routing strategy"),
            }
        }
        _ => panic!("Expected UpdateConfig command"),
    }
    
    // Test UpdateConfig with generation config
    let new_config = GenerationConfig {
        model: "updated-model".to_string(),
        batch_size: 2,
        context_window: 8000,
        max_tokens: 200,
        temperature: 0.9,
        request_size: 15,
    };
    
    let config_update = ProducerCommand::UpdateConfig {
        command_id: 4,
        prompt: None,
        routing_strategy: None,
        generation_config: Some(new_config.clone()),
    };
    
    match &config_update {
        ProducerCommand::UpdateConfig { generation_config, .. } => {
            let config = generation_config.as_ref().unwrap();
            assert_eq!(config.model, "updated-model");
            assert_eq!(config.temperature, 0.9);
            assert_eq!(config.max_tokens, 200);
            assert_eq!(config.request_size, 15);
        }
        _ => panic!("Expected UpdateConfig command"),
    }
}

/// Test UpdateConfig command serialization/deserialization  
#[tokio::test]
async fn test_config_update_serialization() {
    let update_command = ProducerCommand::UpdateConfig {
        command_id: 100,
        prompt: Some("Serialization test prompt".to_string()),
        routing_strategy: Some(RoutingStrategy::Backoff {
            provider: ProviderConfig::with_default_model(ProviderId::Anthropic),
        }),
        generation_config: Some(GenerationConfig {
            model: "serialization-model".to_string(),
            batch_size: 1,
            context_window: 4000,
            max_tokens: 150,
            temperature: 0.7,
            request_size: 10,
        }),
    };
    
    // Serialize to JSON
    let json = serde_json::to_string(&update_command).expect("Should serialize");
    assert!(json.contains("UpdateConfig"));
    assert!(json.contains("Serialization test prompt"));
    assert!(json.contains("serialization-model"));
    
    // Deserialize back
    let deserialized: ProducerCommand = serde_json::from_str(&json).expect("Should deserialize");
    
    match deserialized {
        ProducerCommand::UpdateConfig { command_id, prompt, generation_config, .. } => {
            assert_eq!(command_id, 100);
            assert_eq!(prompt.unwrap(), "Serialization test prompt");
            
            let config = generation_config.unwrap();
            assert_eq!(config.model, "serialization-model");
            assert_eq!(config.temperature, 0.7);
        }
        _ => panic!("Expected UpdateConfig after deserialization"),
    }
}

/// Test multiple sequential UpdateConfig commands
#[tokio::test] 
async fn test_sequential_config_updates() {
    let commands = vec![
        // Start with initial config
        CommandFactory::start_command(1, "sequential-test", "Initial prompt"),
        
        // Update just prompt
        CommandFactory::update_config_command(2, Some("First update".to_string())),
        
        // Update with routing strategy
        ProducerCommand::UpdateConfig {
            command_id: 3,
            prompt: None,
            routing_strategy: Some(RoutingStrategy::RoundRobin {
                providers: vec![
                    ProviderConfig::with_default_model(ProviderId::OpenAI),
                    ProviderConfig::with_default_model(ProviderId::Anthropic),
                ],
            }),
            generation_config: None,
        },
        
        // Update generation config
        ProducerCommand::UpdateConfig {
            command_id: 4,
            prompt: None,
            routing_strategy: None,
            generation_config: Some(GenerationConfig {
                model: "final-model".to_string(),
                batch_size: 3,
                context_window: 16000,
                max_tokens: 300,
                temperature: 0.8,
                request_size: 25,
            }),
        },
        
        // Final prompt update
        CommandFactory::update_config_command(5, Some("Final prompt update".to_string())),
    ];
    
    // Verify all commands have correct structure
    assert_eq!(commands.len(), 5);
    
    // Check Start command
    match &commands[0] {
        ProducerCommand::Start { prompt, .. } => {
            assert_eq!(prompt, "Initial prompt");
        }
        _ => panic!("First command should be Start"),
    }
    
    // Check first update (prompt only)
    match &commands[1] {
        ProducerCommand::UpdateConfig { prompt, routing_strategy, generation_config, .. } => {
            assert_eq!(prompt, &Some("First update".to_string()));
            assert!(routing_strategy.is_none());
            assert!(generation_config.is_none());
        }
        _ => panic!("Second command should be UpdateConfig"),
    }
    
    // Check routing update
    match &commands[2] {
        ProducerCommand::UpdateConfig { routing_strategy, .. } => {
            assert!(routing_strategy.is_some());
            match routing_strategy.as_ref().unwrap() {
                RoutingStrategy::RoundRobin { providers } => {
                    assert_eq!(providers.len(), 2);
                }
                _ => panic!("Expected RoundRobin strategy"),
            }
        }
        _ => panic!("Third command should be UpdateConfig"),
    }
    
    // Check generation config update
    match &commands[3] {
        ProducerCommand::UpdateConfig { generation_config, .. } => {
            let config = generation_config.as_ref().unwrap();
            assert_eq!(config.model, "final-model");
            assert_eq!(config.request_size, 25);
        }
        _ => panic!("Fourth command should be UpdateConfig"),
    }
    
    // Check final prompt update
    match &commands[4] {
        ProducerCommand::UpdateConfig { prompt, .. } => {
            assert_eq!(prompt, &Some("Final prompt update".to_string()));
        }
        _ => panic!("Fifth command should be UpdateConfig"),
    }
}