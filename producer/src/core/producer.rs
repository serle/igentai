//! Functional producer implementation with unified execution model
//!
//! This design uses:
//! - Configuration-driven initialization
//! - Strategy pattern for test vs production modes
//! - Pure functions for business logic
//! - Unified event loop for both modes
//! - Composition over inheritance

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use uuid::Uuid;

use crate::core::generator::CommandGenerator;
use crate::core::utils::{build_api_request, select_provider, should_retry_request};
use crate::core::{Metrics, Processor, PromptHandler};
use crate::error::{ProducerError, ProducerResult};
use crate::traits::{ApiClient, Communicator};
use crate::types::{ApiRequest, ApiResponse, CommandSource, ExecutionConfig, ExecutionMode, ProducerState};
use shared::messages::producer::{ProducerPerformanceStats, ProducerSyncStatus};
use shared::types::{GenerationConfig, ProcessStatus, RoutingStrategy};
use shared::{process_debug, process_error, process_info, process_warn};
use shared::{ProcessId, ProducerCommand, ProducerUpdate, ProviderId};

// ============================================================================
// Producer Implementation
// ============================================================================

/// Producer with unified execution model
pub struct Producer<A, C>
where
    A: ApiClient + Send + Sync + 'static,
    C: Communicator + Send + Sync + 'static,
{
    // Configuration (immutable after construction)
    config: ExecutionConfig,

    // Dependencies (injected, immutable)
    api_client: Arc<A>,
    communicator: Arc<RwLock<C>>,

    // Business logic components
    processor: Arc<RwLock<Processor>>,
    metrics: Arc<RwLock<Metrics>>,
    prompt_handler: Arc<PromptHandler>,

    // Runtime state
    state: Arc<RwLock<ProducerState>>,

    // Control channels
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
}

impl<A, C> Producer<A, C>
where
    A: ApiClient + Send + Sync + 'static,
    C: Communicator + Send + Sync + 'static,
{
    /// Create producer from unified configuration
    pub fn new(config: ExecutionConfig, api_client: A, communicator: C) -> Self {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Producer {
            api_client: Arc::new(api_client),
            communicator: Arc::new(RwLock::new(communicator)),
            processor: Arc::new(RwLock::new(Processor::new())),
            metrics: Arc::new(RwLock::new(Metrics::new())),
            prompt_handler: Arc::new(PromptHandler::new()),
            state: Arc::new(RwLock::new(ProducerState::new(config.producer_config.clone()))),
            config,
            shutdown_tx,
            shutdown_rx: Some(shutdown_rx),
        }
    }

    /// Main unified run loop - same for both test and production modes
    pub async fn run(&mut self) -> ProducerResult<()> {
        process_info!(
            ProcessId::current(),
            "🚀 Starting Producer in {:?} mode",
            format!("{:?}", self.config.mode)
        );

        // Initialize metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.start();
        }

        // Initialize command source based on mode
        let command_source = self.initialize_command_source().await?;

        // Start background tasks
        let status_reporter = self.start_status_reporter();
        let request_processor = self.start_request_processor().await?;

        // Main event loop (unified for both modes)
        let result = self.run_event_loop(command_source).await;

        // Cleanup
        status_reporter.abort();
        request_processor.abort();
        self.cleanup().await?;

        result
    }

    /// Initialize command source based on execution mode
    async fn initialize_command_source(&self) -> ProducerResult<CommandSource> {
        match &self.config.mode {
            ExecutionMode::Production { .. } => {
                process_info!(
                    ProcessId::current(),
                    "📡 Initializing production mode with orchestrator connection"
                );

                let (command_receiver, listen_port) = {
                    let mut communicator = self.communicator.write().await;
                    communicator.initialize().await?;
                    let command_receiver = communicator.get_commands().await?;
                    let listen_port = communicator.get_listen_port().unwrap_or(0);
                    (command_receiver, listen_port)
                };

                // Send ready signal to orchestrator after IPC listener is initialized
                let ready_msg = shared::ProducerUpdate::Ready {
                    producer_id: ProcessId::current().clone(),
                    listen_port,
                };

                process_info!(ProcessId::current(), "📤 Sending ready signal to orchestrator");
                {
                    let communicator = self.communicator.read().await;
                    if let Err(e) = communicator.send_update(ready_msg).await {
                        process_warn!(ProcessId::current(), "⚠️ Failed to send ready signal: {}", e);
                    } else {
                        process_info!(ProcessId::current(), "✅ Ready signal sent to orchestrator");
                    }
                }

                process_info!(ProcessId::current(), "✅ Connected to orchestrator successfully");
                Ok(CommandSource::Orchestrator(command_receiver))
            }
            ExecutionMode::Standalone { max_iterations } => {
                let available_providers = ExecutionConfig::detect_available_providers(&self.config.producer_config);
                process_info!(
                    ProcessId::current(),
                    "🔧 Initializing standalone mode with providers {:?}",
                    available_providers
                );

                let generator =
                    CommandGenerator::new(available_providers, *max_iterations, self.config.request_interval);

                Ok(CommandSource::Simulator(generator))
            }
        }
    }

    /// Unified event loop for both modes
    async fn run_event_loop(&mut self, mut command_source: CommandSource) -> ProducerResult<()> {
        let mut shutdown_rx = self.shutdown_rx.take().unwrap();
        let mut command_interval = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                // Handle shutdown signal
                Some(_) = shutdown_rx.recv() => {
                    process_info!(ProcessId::current(),"🛑 Shutting down Producer...");
                    break;
                }

                // Check for commands (unified handling)
                _ = command_interval.tick() => {
                    if let Some(command) = self.get_next_command(&mut command_source).await? {
                        if let Err(e) = self.handle_command(command).await {
                            process_error!(ProcessId::current(),"❌ Error handling command: {}", e);

                            // In orchestrator mode, if we can't communicate after retries, terminate
                            if matches!(self.config.mode, ExecutionMode::Production { .. }) &&
                               e.to_string().contains("Cannot communicate with orchestrator after retries") {
                                process_error!(ProcessId::current(), "💀 Producer terminating: Command handler lost orchestrator connection");
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get next command from unified command source
    async fn get_next_command(&self, command_source: &mut CommandSource) -> ProducerResult<Option<ProducerCommand>> {
        match command_source {
            CommandSource::Orchestrator(ref mut rx) => {
                // Non-blocking check for orchestrator commands
                match rx.try_recv() {
                    Ok(command) => Ok(Some(command)),
                    Err(mpsc::error::TryRecvError::Empty) => Ok(None),
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        Err(ProducerError::ipc("Orchestrator disconnected"))
                    }
                }
            }
            CommandSource::Simulator(ref mut generator) => {
                let state = self.state.read().await;
                let base_prompt = state.config.topic.clone();
                drop(state);

                Ok(generator.next_command(&base_prompt))
            }
        }
    }

    /// Unified command handling (same logic for both modes)
    async fn handle_command(&self, command: ProducerCommand) -> ProducerResult<()> {
        process_debug!(ProcessId::current(), "Processing command: {:?}", command);

        match command {
            ProducerCommand::Start {
                prompt,
                routing_strategy,
                generation_config,
                ..
            } => {
                let mut state = self.state.write().await;
                if !state.is_running {
                    state.current_prompt = Some(prompt.clone());
                    state.routing_strategy = Some(routing_strategy);
                    state.generation_config = Some(generation_config);
                    state.start();
                    process_info!("✅ Producer started with prompt: {}", prompt);
                }
            }

            ProducerCommand::Stop { .. } => {
                let mut state = self.state.write().await;
                if state.is_running {
                    state.stop();
                    process_info!(ProcessId::current(), "⏹️ Producer stopped");
                }
            }

            ProducerCommand::UpdateConfig {
                prompt,
                routing_strategy,
                generation_config,
                ..
            } => {
                let mut state = self.state.write().await;
                if let Some(new_prompt) = prompt {
                    state.current_prompt = Some(new_prompt);
                }
                if let Some(new_strategy) = routing_strategy {
                    state.routing_strategy = Some(new_strategy);
                }
                if let Some(new_config) = generation_config {
                    state.generation_config = Some(new_config);
                }
                process_info!(ProcessId::current(), "🔄 Updated producer configuration");
            }

            ProducerCommand::SyncCheck {
                sync_id,
                bloom_version,
                bloom_filter,
                seen_values,
                ..
            } => {
                self.handle_sync_check(sync_id, bloom_version, bloom_filter, seen_values)
                    .await?;
            }

            ProducerCommand::Ping { ping_id } => {
                self.handle_ping(ping_id).await?;
            }
        }

        Ok(())
    }

    /// Handle sync check command
    async fn handle_sync_check(
        &self,
        sync_id: u64,
        bloom_version: Option<u64>,
        bloom_filter: Option<Vec<u8>>,
        seen_values: Option<Vec<String>>,
    ) -> ProducerResult<()> {
        process_info!(
            "🔄 Processing SyncCheck: sync_id={}, bloom_version={:?}",
            sync_id,
            bloom_version
        );

        // Update processor with bloom filter
        if let Some(values) = seen_values {
            let mut processor = self.processor.write().await;
            processor.update_bloom_filter(bloom_filter, values);
        }

        // Update state
        {
            let mut state = self.state.write().await;
            state.last_sync_version = bloom_version;
        }

        // Send response only in production mode
        if matches!(self.config.mode, ExecutionMode::Production { .. }) {
            let producer_id = ProcessId::current().clone();

            let response = ProducerUpdate::SyncAck {
                producer_id,
                sync_id,
                bloom_version,
                status: ProducerSyncStatus::Ready,
            };

            let communicator = self.communicator.read().await;
            if let Err(e) = communicator.send_update(response).await {
                if e.to_string().contains("Failed to send update after") {
                    return Err(ProducerError::ipc("Cannot communicate with orchestrator after retries"));
                } else {
                    return Err(e);
                }
            }
            process_info!("✅ Sent SyncAck for sync_id={}", sync_id);
        }

        Ok(())
    }

    /// Handle ping command
    async fn handle_ping(&self, ping_id: u64) -> ProducerResult<()> {
        // Send response only in production mode
        if matches!(self.config.mode, ExecutionMode::Production { .. }) {
            let producer_id = ProcessId::current().clone();

            let response = ProducerUpdate::Pong { producer_id, ping_id };

            let communicator = self.communicator.read().await;
            if let Err(e) = communicator.send_update(response).await {
                if e.to_string().contains("Failed to send update after") {
                    return Err(ProducerError::ipc("Cannot communicate with orchestrator after retries"));
                } else {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Start request processor (unified for both modes)
    async fn start_request_processor(&self) -> ProducerResult<tokio::task::JoinHandle<()>> {
        let api_client = self.api_client.clone();
        let processor = self.processor.clone();
        let metrics = self.metrics.clone();
        let state = self.state.clone();
        let prompt_handler = self.prompt_handler.clone();
        let communicator = self.communicator.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut request_interval = interval(config.request_interval);

            loop {
                request_interval.tick().await;

                // Get current state
                let (is_running, prompt, routing_strategy, generation_config) = {
                    let state = state.read().await;
                    (
                        state.is_running,
                        state.current_prompt.clone(),
                        state.routing_strategy.clone(),
                        state.generation_config.clone(),
                    )
                };

                if !is_running || prompt.is_none() {
                    continue;
                }

                let base_prompt = prompt.unwrap();

                // Process request using pure functions
                if let Err(e) = Self::process_single_request(
                    &api_client,
                    &processor,
                    &metrics,
                    &prompt_handler,
                    &communicator,
                    &state,
                    &routing_strategy,
                    &generation_config,
                    &base_prompt,
                    &config,
                )
                .await
                {
                    process_error!(ProcessId::current(), "❌ Request processing failed: {}", e);

                    // In orchestrator mode, if we can't communicate with orchestrator after retries, terminate
                    if matches!(config.mode, ExecutionMode::Production { .. })
                        && e.to_string().contains("Failed to send update after")
                    {
                        process_error!(
                            ProcessId::current(),
                            "💀 Producer terminating: Cannot communicate with orchestrator after retries"
                        );
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Process a single request (pure function composition)
    async fn process_single_request(
        api_client: &Arc<A>,
        processor: &Arc<RwLock<Processor>>,
        metrics: &Arc<RwLock<Metrics>>,
        prompt_handler: &Arc<PromptHandler>,
        communicator: &Arc<RwLock<C>>,
        state: &Arc<RwLock<ProducerState>>,
        routing_strategy: &Option<RoutingStrategy>,
        generation_config: &Option<GenerationConfig>,
        base_prompt: &str,
        config: &ExecutionConfig,
    ) -> ProducerResult<()> {
        // Build enhanced prompt
        let provider = select_provider(routing_strategy, ProviderId::Random);
        let enhanced_prompt = prompt_handler
            .build_enhanced_prompt(base_prompt, provider, generation_config.as_ref(), state, processor)
            .await;

        // Create request
        let request = build_api_request(routing_strategy, generation_config, enhanced_prompt, Uuid::new_v4());

        // Record request
        {
            let mut metrics_guard = metrics.write().await;
            metrics_guard.record_request_sent(provider);
        }

        // Make API call with retries
        let response = Self::make_request_with_retries(api_client, request, config.max_retries).await?;

        // Record response
        {
            let mut metrics_guard = metrics.write().await;
            metrics_guard.record_response_received(&response);

            if response.success {
                let cost = api_client.estimate_cost(provider, response.tokens_used);
                metrics_guard.record_cost(provider, cost);
            }
        }

        // Process response if successful
        if response.success {
            let mut processor_guard = processor.write().await;
            let processing_stats = processor_guard.process_response(response.clone())?;

            if processing_stats.has_new_values() {
                // Send attributes to orchestrator if connected, otherwise just log
                if matches!(config.mode, ExecutionMode::Production { .. }) {
                    Self::send_attributes_to_orchestrator(
                        communicator,
                        &processing_stats.new_values,
                        provider,
                        &response,
                    )
                    .await?;
                } else {
                    // Log for standalone mode
                    process_debug!(
                        ProcessId::current(),
                        "✨ Found {} new attributes: {:?}",
                        processing_stats.new_values.len(),
                        processing_stats.new_values.iter().take(3).collect::<Vec<_>>()
                    );
                }

                // Record stats
                let mut metrics_guard = metrics.write().await;
                metrics_guard.record_processing_stats(&processing_stats);
            }
        }

        Ok(())
    }

    /// Make API request with exponential backoff (pure function)
    async fn make_request_with_retries(
        api_client: &Arc<A>,
        request: ApiRequest,
        max_retries: u32,
    ) -> ProducerResult<ApiResponse> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            match api_client.send_request(request.clone()).await {
                Ok(response) => {
                    if response.success {
                        return Ok(response);
                    }

                    // Check if we should retry
                    if let Some(delay) = should_retry_request(&response, attempt, max_retries) {
                        process_warn!(
                            ProcessId::current(),
                            "⏳ API error (attempt {}), retrying in {}ms",
                            attempt + 1,
                            delay.as_millis()
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        let delay = Duration::from_millis(100 * (1 << attempt));
                        process_warn!(
                            ProcessId::current(),
                            "⏳ Network error (attempt {}), retrying in {}ms",
                            attempt + 1,
                            delay.as_millis()
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| ProducerError::api("unknown", "Max retries exceeded")))
    }

    /// Send attributes to orchestrator (pure function)
    async fn send_attributes_to_orchestrator(
        communicator: &Arc<RwLock<C>>,
        attributes: &[String],
        provider: ProviderId,
        api_response: &crate::types::ApiResponse,
    ) -> ProducerResult<()> {
        let producer_id = ProcessId::current().clone();

        let update = ProducerUpdate::AttributeBatch {
            producer_id,
            attributes: attributes.to_vec(),
            batch_id: chrono::Utc::now().timestamp_millis() as u64,
            provider_metadata: shared::types::ProviderMetadata {
                provider_id: provider,
                model: format!("{provider:?}_model").to_lowercase(),
                response_time_ms: api_response.response_time_ms,
                tokens: shared::types::TokenUsage {
                    // For Random provider, calculate realistic split; for others, rough split
                    input_tokens: if provider == shared::ProviderId::Random {
                        (api_response.request_id.to_string().len() / 4) as u64 // Based on prompt length
                    } else {
                        (api_response.tokens_used / 3) as u64 // Input is typically smaller
                    },
                    output_tokens: if provider == shared::ProviderId::Random {
                        api_response.tokens_used as u64 - (api_response.request_id.to_string().len() / 4) as u64
                    } else {
                        ((api_response.tokens_used * 2) / 3) as u64 // Output is typically larger
                    },
                },
                request_timestamp: api_response.timestamp.timestamp_millis() as u64,
            },
        };

        let communicator = communicator.read().await;
        communicator.send_update(update).await?;
        process_debug!(
            ProcessId::current(),
            "📤 Sent {} attributes to orchestrator",
            attributes.len()
        );

        Ok(())
    }

    /// Start status reporter (same for both modes)
    fn start_status_reporter(&self) -> tokio::task::JoinHandle<()> {
        let communicator = self.communicator.clone();
        let state = self.state.clone();
        let metrics = self.metrics.clone();
        let status_interval_duration = self.config.status_report_interval;
        let has_orchestrator = matches!(self.config.mode, ExecutionMode::Production { .. });

        tokio::spawn(async move {
            let mut status_interval = interval(status_interval_duration);

            loop {
                status_interval.tick().await;

                // Send status updates when orchestrator is present (mandatory for fast termination detection)
                if !has_orchestrator {
                    continue;
                }

                let (is_running, current_topic) = {
                    let state = state.read().await;
                    (state.is_running, state.current_prompt.clone())
                };

                let current_metrics = {
                    let metrics = metrics.read().await;
                    metrics.get_current_metrics()
                };

                let status_update = ProducerUpdate::StatusUpdate {
                    producer_id: ProcessId::current().clone(),
                    status: if is_running {
                        ProcessStatus::Running
                    } else {
                        ProcessStatus::Stopped
                    },
                    message: current_topic,
                    performance_stats: Some(ProducerPerformanceStats {
                        attributes_generated_last_minute: current_metrics.attributes_extracted,
                        unique_contributed_last_minute: current_metrics.unique_attributes,
                        requests_made_last_minute: current_metrics.requests_sent,
                        provider_usage: HashMap::new(),
                        current_batch_rate: current_metrics.attributes_per_minute(),
                        memory_usage_mb: None,
                        bloom_filter_size_mb: None,
                    }),
                };

                let communicator = communicator.read().await;
                if let Err(e) = communicator.send_update(status_update).await {
                    process_error!(ProcessId::current(), "❌ Status update failed: {}", e);
                    if e.to_string().contains("Failed to send update after") {
                        process_error!(
                            ProcessId::current(),
                            "💀 Orchestrator unreachable after retries - producer terminating NOW"
                        );
                        std::process::exit(1);
                    }
                } else {
                    process_debug!(ProcessId::current(), "📊 Status update sent successfully");
                }
            }
        })
    }

    /// Clean shutdown
    async fn cleanup(&self) -> ProducerResult<()> {
        {
            let mut state = self.state.write().await;
            if state.is_running {
                state.stop();
            }
        }

        if matches!(self.config.mode, ExecutionMode::Production { .. }) {
            let communicator = self.communicator.read().await;
            let _ = communicator.disconnect().await;
        }

        process_info!(ProcessId::current(), "✅ Functional Producer shutdown complete");
        Ok(())
    }

    /// Get shutdown sender for external control
    pub fn shutdown_sender(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create producer from command line arguments (pure function)
pub fn create_producer_from_args<A, C>(
    args: Vec<String>,
    api_client: A,
    communicator: C,
) -> ProducerResult<Producer<A, C>>
where
    A: ApiClient + Send + Sync + 'static,
    C: Communicator + Send + Sync + 'static,
{
    // Parse arguments (this would be more sophisticated in real implementation)
    let orchestrator_endpoint = args.get(1).cloned();
    let topic = args.get(2).cloned().unwrap_or_else(|| "default topic".to_string());
    let max_requests = args.get(3).and_then(|s| s.parse().ok());

    let config = ExecutionConfig::from_args_and_env(
        orchestrator_endpoint,
        topic,
        Some(2), // 2 second intervals
        max_requests,
    )?;

    Ok(Producer::new(config, api_client, communicator))
}
