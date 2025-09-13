//! Producer implementation with dependency injection

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;

use shared::{ProducerId, ProducerCommand, ProducerUpdate, ProviderRequestMetadata};
use crate::error::ProducerResult;
use crate::traits::{IpcCommunicator, ProviderRouter, ResponseProcessor, PerformanceTracker};
use crate::types::ProducerState;

/// Producer with dependency injection
pub struct Producer<I, R, A, P>
where
    I: IpcCommunicator,
    R: ProviderRouter,
    A: ResponseProcessor,
    P: PerformanceTracker,
{
    pub state: Arc<RwLock<ProducerState>>,
    pub ipc_communicator: I,
    pub provider_router: R,
    pub response_processor: A,
    pub performance_tracker: P,
}

impl<I, R, A, P> Producer<I, R, A, P>
where
    I: IpcCommunicator,
    R: ProviderRouter,
    A: ResponseProcessor,
    P: PerformanceTracker,
{
    /// Create new producer instance
    pub fn new(
        id: ProducerId,
        orchestrator_addr: SocketAddr,
        ipc_communicator: I,
        provider_router: R,
        response_processor: A,
        performance_tracker: P,
    ) -> Self {
        let state = ProducerState::new(id, orchestrator_addr);
        
        Self {
            state: Arc::new(RwLock::new(state)),
            ipc_communicator,
            provider_router,
            response_processor,
            performance_tracker,
        }
    }

    /// Shutdown the producer and clean up all services
    pub async fn shutdown(&self) -> ProducerResult<()> {
        println!("Shutting down producer...");
        
        // Set shutdown flag
        {
            let state = self.state.read().await;
            state.should_stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        // Disconnect from orchestrator
        self.ipc_communicator.disconnect().await?;
        
        println!("Producer shutdown complete");
        Ok(())
    }

    /// Set API keys from environment (requires at least one key)
    pub async fn set_api_keys(&self, keys: HashMap<String, String>) -> ProducerResult<()> {
        if keys.is_empty() {
            return Err(crate::error::ProducerError::ConfigError {
                message: "At least one API key must be provided".to_string(),
            });
        }
        
        // Convert String keys to ProviderId
        let provider_keys: HashMap<shared::ProviderId, String> = keys.into_iter()
            .filter_map(|(k, v)| shared::ProviderId::from_str(&k).map(|provider_id| (provider_id, v)))
            .collect();
        
        if provider_keys.is_empty() {
            return Err(crate::error::ProducerError::ConfigError {
                message: "No valid provider keys found (supported: openai, anthropic, gemini)".to_string(),
            });
        }
        
        let num_keys = provider_keys.len();
        
        // Set keys in the provider router (only place they're stored now)
        self.provider_router.set_api_keys(provider_keys).await?;
        
        println!("Set {} API keys for producer", num_keys);
        Ok(())
    }

    /// Start the producer main loop
    pub async fn start(&self) -> ProducerResult<()> {
        // Connect to orchestrator
        let orchestrator_addr = {
            let state = self.state.read().await;
            state.orchestrator_addr
        };
        self.ipc_communicator.connect(orchestrator_addr).await?;

        // Start command listener
        let mut command_rx = self.ipc_communicator.listen_for_commands().await?;

        // Set running flag
        {
            let state = self.state.read().await;
            state.is_running.store(true, Ordering::Relaxed);
        }

        println!("Producer started and waiting for commands");

        // Main command processing loop
        while let Some(command) = command_rx.recv().await {
            let should_stop = {
                let state = self.state.read().await;
                state.should_stop.load(Ordering::Relaxed)
            };
            
            if should_stop {
                break;
            }

            match command {
                ProducerCommand::Start { topic, prompt, routing_strategy, generation_config } => {
                    {
                        let mut state = self.state.write().await;
                        state.topic = topic.clone();
                    }
                    
                    // Set prompt, routing strategy, and generation config in provider router
                    self.provider_router.set_prompt(prompt).await?;
                    self.provider_router.set_routing_strategy(routing_strategy).await?;
                    self.provider_router.set_generation_config(generation_config).await?;
                    
                    // Start generation task
                    self.start_generation_task().await?;
                }
                ProducerCommand::UpdatePrompt { prompt } => {
                    self.provider_router.set_prompt(prompt).await?;
                }
                ProducerCommand::UpdateRoutingStrategy { routing_strategy } => {
                    self.provider_router.set_routing_strategy(routing_strategy).await?;
                }
                ProducerCommand::UpdateGenerationConfig { generation_config } => {
                    self.provider_router.set_generation_config(generation_config).await?;
                }
                ProducerCommand::UpdateBloomFilter { bloom_filter } => {
                    self.response_processor.set_bloom_filter(bloom_filter).await?;
                }
                ProducerCommand::Stop => {
                    let state = self.state.read().await;
                    state.should_stop.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }

        // Cleanup
        self.ipc_communicator.disconnect().await?;
        println!("Producer stopped");
        Ok(())
    }

    /// Start the generation task
    async fn start_generation_task(&self) -> ProducerResult<()> {
        let state_clone = self.state.clone();
        let producer_id = {
            let state = self.state.read().await;
            state.id.clone()
        };

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let should_stop = {
                    let state = state_clone.read().await;
                    state.should_stop.load(Ordering::Relaxed)
                };
                
                if should_stop {
                    break;
                }
                
                // This is where the generation would happen
                println!("Producer {} would generate attributes here", producer_id);
            }
        });

        Ok(())
    }

    /// Generate attributes using selected provider
    async fn generate_attributes(&self) -> ProducerResult<Vec<String>> {
        // Select provider
        let provider = self.provider_router.select_provider().await?;
        
        // Get current topic and model from config
        let (topic, model) = {
            let state = self.state.read().await;
            // Get model from provider's generation config
            let config = self.provider_router.get_request_config(&provider).await;
            (state.topic.clone(), config.model)
        };

        // Make provider request using router (router handles prompt + topic combination)
        match self.provider_router.make_provider_request(&provider, &model, &topic).await {
            Ok(provider_response) => {
                // Record success
                self.performance_tracker.record_success(
                    &provider.to_string(),
                    provider_response.response_time,
                    provider_response.tokens_used,
                ).await?;

                // Extract attributes first
                let extracted_attributes = self.response_processor.extract_attributes(&provider_response.content).await?;
                
                // Filter for duplicates using bloom filter
                let unique_attributes = self.response_processor.filter_duplicates(&extracted_attributes).await?;

                // Send data update
                let producer_id = {
                    let state = self.state.read().await;
                    state.id.clone()
                };

                let metadata = ProviderRequestMetadata {
                    response_time_ms: provider_response.response_time.as_millis() as u64,
                    tokens_used: provider_response.tokens_used,
                    prompt_tokens: provider_response.prompt_tokens,
                    completion_tokens: provider_response.completion_tokens,
                    provider_status: shared::ProviderStatus::Healthy,
                    success: true,
                };

                // Get accurate bloom filter stats
                let bloom_stats = shared::messages::metrics::BloomFilterStats {
                    total_candidates: extracted_attributes.len() as u64,
                    filtered_candidates: unique_attributes.len() as u64,
                    filter_effectiveness: if extracted_attributes.len() > 0 {
                        (extracted_attributes.len() - unique_attributes.len()) as f64 / extracted_attributes.len() as f64
                    } else {
                        0.0
                    },
                    last_filter_update: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                let update = ProducerUpdate::DataUpdate {
                    producer_id: producer_id.to_string(),
                    attributes: unique_attributes.clone(),
                    provider_used: provider,
                    provider_metadata: metadata,
                    bloom_stats: Some(bloom_stats),
                };

                self.ipc_communicator.send_update(update).await?;

                Ok(unique_attributes)
            }
            Err(failure) => {
                // Record failure
                self.performance_tracker.record_failure(&provider.to_string(), failure.clone()).await?;
                
                Err(crate::error::ProducerError::ProviderError {
                    provider: provider.to_string(),
                    reason: failure,
                })
            }
        }
    }
}