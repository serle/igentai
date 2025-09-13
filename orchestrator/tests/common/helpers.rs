//! Test helpers and builder patterns for orchestrator tests
//!
//! This module provides convenient helper functions and builder patterns
//! to reduce test boilerplate and improve maintainability.
use orchestrator::*;
use orchestrator::traits::{RequiredKeyMissing, KeyValuePair};
use super::fixtures::TestFixtures;

/// Builder pattern for creating test orchestrators with sensible defaults
pub struct OrchestratorBuilder {
    producer_count: u32,
    port: u16,
    api_keys: MockApiKeySource,
    file_system: MockFileSystem,
    process_manager: MockProcessManager,
    message_transport: MockMessageTransport,
}

impl OrchestratorBuilder {
    /// Create a new builder with sensible defaults and basic mock setup
    pub fn new() -> Self {
        let mut api_keys = MockApiKeySource::new();
        let mut file_system = MockFileSystem::new();
        let process_manager = MockProcessManager::new();
        let mut message_transport = MockMessageTransport::new();
        
        // Set up default successful behaviors to prevent panics
        api_keys
            .expect_get_api_keys()
            .returning(|| Ok(vec![]))
            .times(0..);
            
        file_system
            .expect_write_attributes()
            .returning(|_| Ok(()))
            .times(0..);
            
        message_transport
            .expect_process_messages()
            .returning(|| Ok(vec![]))
            .times(0..);
            
        message_transport
            .expect_send_updates()
            .returning(|_| Ok(()))
            .times(0..);
        
        Self {
            producer_count: TestFixtures::DEFAULT_PRODUCER_COUNT,
            port: TestFixtures::DEFAULT_PORT,
            api_keys,
            file_system,
            process_manager,
            message_transport,
        }
    }
    
    /// Set the producer count
    pub fn with_producer_count(mut self, count: u32) -> Self {
        self.producer_count = count;
        self
    }
    
    /// Set the webserver port
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    /// Configure the API keys mock with a setup function
    pub fn with_api_keys<F>(mut self, setup: F) -> Self 
    where 
        F: FnOnce(&mut MockApiKeySource)
    {
        setup(&mut self.api_keys);
        self
    }
    
    /// Configure the file system mock with a setup function
    pub fn with_file_system<F>(mut self, setup: F) -> Self 
    where 
        F: FnOnce(&mut MockFileSystem)
    {
        setup(&mut self.file_system);
        self
    }
    
    /// Configure the process manager mock with a setup function
    pub fn with_process_manager<F>(mut self, setup: F) -> Self 
    where 
        F: FnOnce(&mut MockProcessManager)
    {
        setup(&mut self.process_manager);
        self
    }
    
    /// Configure the message transport mock with a setup function
    pub fn with_message_transport<F>(mut self, setup: F) -> Self 
    where 
        F: FnOnce(&mut MockMessageTransport)
    {
        setup(&mut self.message_transport);
        self
    }
    
    /// Build the orchestrator with all configured mocks
    pub fn build(self) -> TestOrchestrator {
        Orchestrator::new(
            self.producer_count,
            self.port,
            self.api_keys,
            self.file_system,
            self.process_manager,
            self.message_transport,
        )
    }
}

impl Default for OrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for test orchestrator with all mocks
pub type TestOrchestrator = Orchestrator<
    MockApiKeySource,
    MockFileSystem, 
    MockProcessManager,
    MockMessageTransport,
>;

/// Helper functions for common test operations
pub struct TestHelpers;

impl TestHelpers {
    /// Create a simple orchestrator with minimal setup for basic tests
    pub fn simple_orchestrator() -> TestOrchestrator {
        OrchestratorBuilder::new().build()
    }
    
    /// Create an orchestrator configured for API validation tests
    pub fn api_validation_orchestrator() -> TestOrchestrator {
        
        OrchestratorBuilder::new()
            .with_api_keys(|api_keys| {
                api_keys
                    .expect_get_api_keys()
                    .returning(|| Ok(vec![
                        KeyValuePair {
                            key: "OPENAI_API_KEY".to_string(),
                            value: TestFixtures::OPENAI_KEY.to_string(),
                        },
                        KeyValuePair {
                            key: "ANTHROPIC_API_KEY".to_string(),
                            value: TestFixtures::ANTHROPIC_KEY.to_string(),
                        },
                    ]));
            })
            .build()
    }
    
    /// Create an orchestrator that expects API key validation failure
    pub fn failing_api_orchestrator() -> TestOrchestrator {
        let mut api_keys = MockApiKeySource::new();
        let file_system = MockFileSystem::new();
        let process_manager = MockProcessManager::new();
        let mut message_transport = MockMessageTransport::new();
        
        // Set up failing validation
        api_keys
            .expect_get_api_keys()
            .times(1)
            .returning(|| Err(RequiredKeyMissing {
                key_name: "OPENAI_API_KEY".to_string(),
                message: "Missing required API keys: OPENAI_API_KEY. These keys must be set as environment variables.".to_string(),
            }));
            
        // Still need other default expectations
        message_transport
            .expect_send_updates()
            .returning(|_| Ok(()))
            .times(0..);
            
        Orchestrator::new(
            TestFixtures::DEFAULT_PRODUCER_COUNT,
            TestFixtures::DEFAULT_PORT,
            api_keys,
            file_system,
            process_manager,
            message_transport,
        )
    }
    
    /// Process a batch and return the unique count for testing
    pub async fn process_and_count(
        orchestrator: &mut TestOrchestrator, 
        producer_id: shared::ProducerId, 
        attributes: Vec<String>
    ) -> usize {
        let result = orchestrator.process_result(producer_id, attributes).await.unwrap();
        result.len()
    }
    
    /// Assert that orchestrator has expected state
    pub fn assert_state(
        orchestrator: &TestOrchestrator,
        expected_producer_count: u32,
        expected_port: u16,
        expected_total_count: u64,
        expected_topic: Option<&str>
    ) {
        assert_eq!(orchestrator.producer_count(), expected_producer_count);
        assert_eq!(orchestrator.port(), expected_port);
        assert_eq!(orchestrator.total_count(), expected_total_count);
        assert_eq!(orchestrator.current_topic(), expected_topic);
    }
}