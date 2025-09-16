//! Message factory utilities for creating test messages

use shared::types::ProviderId as ProviderIdEnum;
use shared::{
    messages::producer::{ProducerPerformanceStats, ProducerSyncStatus},
    GenerationConfig, ProcessStatus, ProcessId, ProducerCommand, ProducerCommand as OrchestratorCommand,
    ProducerUpdate, ProviderMetadata, RoutingStrategy,
};
use std::collections::HashMap;

/// Factory for creating test orchestrator commands
pub struct CommandFactory;

impl CommandFactory {
    /// Create a Start command
    pub fn start_command(command_id: u64, topic: &str, prompt: &str) -> ProducerCommand {
        OrchestratorCommand::Start {
            command_id,
            topic: topic.to_string(),
            prompt: prompt.to_string(),
            routing_strategy: RoutingStrategy::RoundRobin {
                providers: vec![ProviderIdEnum::OpenAI, ProviderIdEnum::Anthropic],
            },
            generation_config: GenerationConfig {
                model: "gpt-4o-mini".to_string(),
                batch_size: 1,
                context_window: 4096,
                max_tokens: 150,
                temperature: 0.7,
                request_size: 50,
            },
        }
    }

    /// Create a Start command with Backoff routing strategy (for test mode)
    pub fn start_command_with_backoff(
        command_id: u64,
        topic: &str,
        prompt: &str,
        provider: ProviderIdEnum,
    ) -> ProducerCommand {
        OrchestratorCommand::Start {
            command_id,
            topic: topic.to_string(),
            prompt: prompt.to_string(),
            routing_strategy: RoutingStrategy::Backoff { provider },
            generation_config: GenerationConfig {
                model: "gpt-4o-mini".to_string(),
                batch_size: 1,
                context_window: 4096,
                max_tokens: 1000,
                temperature: 0.8,
                request_size: 100,
            },
        }
    }

    /// Create a Stop command
    pub fn stop_command(command_id: u64) -> ProducerCommand {
        OrchestratorCommand::Stop { command_id }
    }

    /// Create an UpdateConfig command
    pub fn update_config_command(command_id: u64, new_prompt: Option<String>) -> ProducerCommand {
        OrchestratorCommand::UpdateConfig {
            command_id,
            routing_strategy: None,
            generation_config: None,
            prompt: new_prompt,
        }
    }

    /// Create a SyncCheck command
    pub fn sync_check_command(sync_id: u64, bloom_filter: Option<Vec<u8>>, requires_dedup: bool) -> ProducerCommand {
        OrchestratorCommand::SyncCheck {
            sync_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            bloom_filter,
            bloom_version: Some(1),
            requires_dedup,
            seen_values: None,
        }
    }

    /// Create a SyncCheck command with seen values
    pub fn sync_check_command_with_seen(
        sync_id: u64,
        bloom_filter: Option<Vec<u8>>,
        requires_dedup: bool,
        seen_values: Option<Vec<String>>,
    ) -> ProducerCommand {
        OrchestratorCommand::SyncCheck {
            sync_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            bloom_filter,
            bloom_version: Some(1),
            requires_dedup,
            seen_values,
        }
    }

    /// Create a Ping command
    pub fn ping_command(ping_id: u64) -> ProducerCommand {
        OrchestratorCommand::Ping { ping_id }
    }
}

/// Factory for creating test producer updates
pub struct UpdateFactory;

impl UpdateFactory {
    /// Create an AttributeBatch update
    pub fn attribute_batch(
        producer_id: ProcessId,
        batch_id: u64,
        attributes: Vec<String>,
        provider: ProviderIdEnum,
    ) -> ProducerUpdate {
        ProducerUpdate::AttributeBatch {
            producer_id,
            batch_id,
            attributes,
            provider_metadata: ProviderMetadata {
                provider_id: provider,
                model: "gpt-4o-mini".to_string(),
                tokens: shared::types::TokenUsage {
                    input_tokens: 50,
                    output_tokens: 50,
                },
                response_time_ms: 500,
                request_timestamp: chrono::Utc::now().timestamp_millis() as u64,
            },
        }
    }

    /// Create a StatusUpdate
    pub fn status_update(
        producer_id: ProcessId,
        status: ProcessStatus,
        message: Option<String>,
        include_stats: bool,
    ) -> ProducerUpdate {
        let performance_stats = if include_stats {
            Some(ProducerPerformanceStats {
                attributes_generated_last_minute: 50,
                unique_contributed_last_minute: 48,
                requests_made_last_minute: 2,
                provider_usage: HashMap::new(),
                current_batch_rate: 25.0,
                memory_usage_mb: Some(64),
                bloom_filter_size_mb: Some(2.5),
            })
        } else {
            None
        };

        ProducerUpdate::StatusUpdate {
            producer_id,
            status,
            message,
            performance_stats,
        }
    }

    /// Create a SyncAck update
    pub fn sync_ack(producer_id: ProcessId, sync_id: u64, status: ProducerSyncStatus) -> ProducerUpdate {
        ProducerUpdate::SyncAck {
            producer_id,
            sync_id,
            bloom_version: Some(1),
            status,
        }
    }

    /// Create a Pong update
    pub fn pong(producer_id: ProcessId, ping_id: u64) -> ProducerUpdate {
        ProducerUpdate::Pong { producer_id, ping_id }
    }

    /// Create an Error update
    pub fn error(producer_id: ProcessId, error_code: &str, message: &str, command_id: Option<u64>) -> ProducerUpdate {
        ProducerUpdate::Error {
            producer_id,
            error_code: error_code.to_string(),
            message: message.to_string(),
            command_id,
        }
    }
}

/// Factory for creating test process IDs
pub struct ProcessIdFactory;

#[allow(dead_code)] // Utility methods may not all be used
impl ProcessIdFactory {
    /// Create a test process ID
    pub fn create() -> ProcessId {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        ProcessId::Producer(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    /// Create a producer process ID with a specific number
    pub fn with_number(num: u32) -> ProcessId {
        ProcessId::Producer(num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_factory() {
        let start_cmd = CommandFactory::start_command(1, "test topic", "test prompt");
        match start_cmd {
            OrchestratorCommand::Start {
                command_id,
                topic,
                prompt,
                ..
            } => {
                assert_eq!(command_id, 1);
                assert_eq!(topic, "test topic");
                assert_eq!(prompt, "test prompt");
            }
            _ => panic!("Expected Start command"),
        }

        let stop_cmd = CommandFactory::stop_command(2);
        match stop_cmd {
            OrchestratorCommand::Stop { command_id } => {
                assert_eq!(command_id, 2);
            }
            _ => panic!("Expected Stop command"),
        }
    }

    #[test]
    fn test_update_factory() {
        let producer_id = ProcessIdFactory::create();
        let attributes = vec!["attr1".to_string(), "attr2".to_string()];

        let batch_update =
            UpdateFactory::attribute_batch(producer_id.clone(), 1, attributes.clone(), ProviderIdEnum::OpenAI);

        match batch_update {
            ProducerUpdate::AttributeBatch {
                batch_id,
                attributes: attrs,
                ..
            } => {
                assert_eq!(batch_id, 1);
                assert_eq!(attrs, attributes);
            }
            _ => panic!("Expected AttributeBatch update"),
        }

        let status_update = UpdateFactory::status_update(
            producer_id,
            ProcessStatus::Running,
            Some("Running test".to_string()),
            true,
        );

        match status_update {
            ProducerUpdate::StatusUpdate {
                status,
                message,
                performance_stats,
                ..
            } => {
                assert_eq!(status, ProcessStatus::Running);
                assert_eq!(message, Some("Running test".to_string()));
                assert!(performance_stats.is_some());
            }
            _ => panic!("Expected StatusUpdate"),
        }
    }
}
