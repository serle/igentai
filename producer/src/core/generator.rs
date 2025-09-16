//! Command generation for test mode simulation

use crate::core::utils::auto_select_routing_strategy;
use shared::types::GenerationConfig;
use shared::{ProducerCommand, ProviderId};
use std::time::{Duration, Instant};

/// Generates commands for test mode simulation
#[derive(Debug)]
pub struct CommandGenerator {
    available_providers: Vec<ProviderId>,
    iteration_count: u32,
    max_iterations: Option<u32>,
    last_request: Instant,
    request_interval: Duration,
}

impl CommandGenerator {
    pub fn new(available_providers: Vec<ProviderId>, max_iterations: Option<u32>, interval: Duration) -> Self {
        Self {
            available_providers,
            iteration_count: 0,
            max_iterations,
            last_request: Instant::now() - interval, // Allow immediate first request
            request_interval: interval,
        }
    }

    /// Generate the next command for test mode (pure function)
    pub fn next_command(&mut self, base_prompt: &str) -> Option<ProducerCommand> {
        let now = Instant::now();

        // Check if we should stop due to max iterations
        if let Some(max) = self.max_iterations {
            if self.iteration_count >= max {
                return Some(ProducerCommand::Stop {
                    command_id: self.iteration_count as u64 + 1000,
                });
            }
        }

        // Check if enough time has passed
        if now.duration_since(self.last_request) < self.request_interval {
            return None;
        }

        self.last_request = now;
        self.iteration_count += 1;

        // Generate start command on first iteration
        if self.iteration_count == 1 {
            // Auto-select optimal routing strategy based on available providers
            let routing_strategy = auto_select_routing_strategy(&self.available_providers, None);

            Some(ProducerCommand::Start {
                command_id: 1,
                topic: "test_topic".to_string(),
                prompt: base_prompt.to_string(),
                routing_strategy,
                generation_config: GenerationConfig {
                    model: "test_model".to_string(),
                    batch_size: 1,
                    context_window: 4096,
                    max_tokens: 1000,
                    temperature: 0.8,
                    request_size: 10,
                },
            })
        } else {
            None // Let the main loop handle request generation
        }
    }

    pub fn get_iteration_count(&self) -> u32 {
        self.iteration_count
    }

    pub fn get_available_providers(&self) -> &[ProviderId] {
        &self.available_providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::RoutingStrategy;

    #[test]
    fn test_command_generator_creation() {
        let providers = vec![ProviderId::OpenAI, ProviderId::Anthropic];
        let generator = CommandGenerator::new(providers.clone(), Some(10), Duration::from_secs(1));

        assert_eq!(generator.get_iteration_count(), 0);
        assert_eq!(generator.get_available_providers(), &providers);
    }

    #[test]
    fn test_command_generator_first_command() {
        let providers = vec![ProviderId::OpenAI];
        let mut generator = CommandGenerator::new(providers, Some(10), Duration::from_secs(1));

        let command = generator.next_command("test prompt");
        assert!(command.is_some());

        if let Some(ProducerCommand::Start {
            topic,
            prompt,
            routing_strategy,
            ..
        }) = command
        {
            assert_eq!(topic, "test_topic");
            assert_eq!(prompt, "test prompt");
            // Single provider should use backoff strategy
            assert!(matches!(routing_strategy, RoutingStrategy::Backoff { .. }));
        } else {
            panic!("Expected Start command");
        }

        assert_eq!(generator.get_iteration_count(), 1);
    }

    #[test]
    fn test_command_generator_max_iterations() {
        let providers = vec![ProviderId::Random];
        let mut generator = CommandGenerator::new(providers, Some(1), Duration::from_millis(1));

        // First command should be Start
        let first = generator.next_command("test");
        assert!(matches!(first, Some(ProducerCommand::Start { .. })));

        // Wait a bit to ensure interval passes
        std::thread::sleep(Duration::from_millis(2));

        // Second command should be Stop since max_iterations is 1
        let second = generator.next_command("test");
        assert!(matches!(second, Some(ProducerCommand::Stop { .. })));
    }

    #[test]
    fn test_command_generator_interval_throttling() {
        let providers = vec![ProviderId::Random];
        let mut generator = CommandGenerator::new(providers, None, Duration::from_secs(1));

        // First command should succeed
        let first = generator.next_command("test");
        assert!(first.is_some());

        // Immediate second call should return None due to interval
        let second = generator.next_command("test");
        assert!(second.is_none());
    }
}
