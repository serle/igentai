//! Main orchestrator implementation
//!
//! This is the primary orchestrator that coordinates between webserver, producers,
//! and manages the overall system state using dependency injection.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};

use shared::messages::webserver::CompletionReason;
use shared::{
    logging, process_debug, process_error, process_info, GenerationConstraints, OptimizationMode, OrchestratorCommand,
    OrchestratorUpdate, ProcessId, ProcessStatus, ProducerUpdate, WebServerRequest,
};

use crate::{
    core::OrchestratorState,
    error::OrchestratorResult,
    traits::{ApiKeySource, Communicator, FileSystem, ProcessManager},
};

/// Main orchestrator that coordinates the entire system
pub struct Orchestrator<A, C, F, P>
where
    A: ApiKeySource + Send + Sync + 'static,
    C: Communicator + Send + Sync + 'static,
    F: FileSystem + Send + Sync + 'static,
    P: ProcessManager + Send + Sync + 'static,
{
    /// Core state management
    state: Arc<Mutex<OrchestratorState>>,

    /// Injected services
    api_keys: A,
    communicator: C,
    file_system: F,
    process_manager: P,

    /// Active message receivers
    webserver_rx: Option<mpsc::Receiver<WebServerRequest>>,
    producer_rx: Option<mpsc::Receiver<ProducerUpdate>>,

    /// Producer communication address (for spawning producers)
    producer_addr: Option<std::net::SocketAddr>,

    /// WebServer communication address (for spawning webserver)
    webserver_addr: Option<std::net::SocketAddr>,

    /// Shutdown signal
    shutdown_tx: mpsc::Sender<()>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl<A, C, F, P> Orchestrator<A, C, F, P>
where
    A: ApiKeySource + Send + Sync + 'static,
    C: Communicator + Send + Sync + 'static,
    F: FileSystem + Send + Sync + 'static,
    P: ProcessManager + Send + Sync + 'static,
{
    /// Create new orchestrator with injected dependencies
    pub fn new(api_keys: A, communicator: C, file_system: F, process_manager: P) -> Self {
        let state = Arc::new(Mutex::new(OrchestratorState::new()));
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Self {
            state,
            api_keys,
            communicator,
            file_system,
            process_manager,
            webserver_rx: None,
            producer_rx: None,
            producer_addr: None,
            webserver_addr: None,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Initialize the orchestrator and start listening for messages
    pub async fn initialize(
        &mut self,
        webserver_bind_addr: std::net::SocketAddr,
        producer_bind_addr: std::net::SocketAddr,
    ) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🚀 Initializing orchestrator...");

        // Load API keys
        let api_keys = self.api_keys.get_api_keys().await?;
        process_debug!(ProcessId::current(), "🔑 Loaded {} API keys", api_keys.len());

        // Start communication listeners
        let webserver_rx = self.communicator.start_webserver_listener(webserver_bind_addr).await?;
        let producer_rx = self.communicator.start_producer_listener(producer_bind_addr).await?;

        self.webserver_rx = Some(webserver_rx);
        self.producer_rx = Some(producer_rx);
        self.producer_addr = Some(producer_bind_addr);
        self.webserver_addr = Some(webserver_bind_addr);

        process_debug!(ProcessId::current(), "🌐 WebServer listener: {}", webserver_bind_addr);
        process_debug!(ProcessId::current(), "🏭 Producer listener: {}", producer_bind_addr);

        // Spawn and register webserver process
        let webserver_addr = self.webserver_addr.expect("WebServer address not initialized");
        let webserver_info = self.process_manager.spawn_webserver(8080, webserver_addr).await?;
        self.communicator.register_webserver(webserver_info.api_address).await?;

        process_debug!(
            ProcessId::current(),
            "🌍 WebServer spawned and registered at {}",
            webserver_info.api_address
        );
        logging::log_success(ProcessId::current(), "Orchestrator initialized successfully");

        Ok(())
    }

    /// Initialize for CLI mode (no webserver)
    pub async fn initialize_cli_mode(&mut self, producer_bind_addr: std::net::SocketAddr) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🚀 Initializing orchestrator in CLI mode...");

        // Load API keys
        let api_keys = self.api_keys.get_api_keys().await?;
        process_debug!(ProcessId::current(), "🔑 Loaded {} API keys", api_keys.len());

        // Start only producer listener (no webserver)
        let producer_rx = self.communicator.start_producer_listener(producer_bind_addr).await?;

        self.producer_rx = Some(producer_rx);
        self.producer_addr = Some(producer_bind_addr);

        process_debug!(ProcessId::current(), "🏭 Producer listener: {}", producer_bind_addr);
        logging::log_success(ProcessId::current(), "Orchestrator initialized in CLI mode");

        Ok(())
    }

    /// Start generation immediately for CLI mode
    pub async fn start_cli_generation(
        &mut self,
        topic: String,
        producer_count: u32,
        iterations: Option<u32>,
        request_size: usize,
        routing_strategy: Option<String>,
        routing_provider: Option<String>,
    ) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🚀 Starting CLI generation");

        // Use default optimization mode and constraints for CLI
        let optimization_mode = OptimizationMode::MaximizeEfficiency;
        let constraints = GenerationConstraints {
            max_cost_per_minute: 1.0,
            target_uam: 100.0,
            max_runtime_seconds: None,
        };

        // Store iterations limit in state for tracking
        {
            let mut state = self.state.lock().await;
            state.set_cli_iterations(iterations);
        }

        // Log topic start with iteration budget
        let budget_str = match iterations {
            Some(limit) => format!("with {limit} iteration budget"),
            None => "with no iteration budget".to_string(),
        };
        process_info!(ProcessId::current(), "✅ Topic '{}' started {}", topic, budget_str);

        // Start generation with a fake request ID and custom request size
        self.start_generation_with_config(1, topic, producer_count, optimization_mode, constraints, request_size, routing_strategy, routing_provider)
            .await?;

        Ok(())
    }

    /// Main event loop - processes messages and coordinates the system
    pub async fn run(&mut self) -> OrchestratorResult<()> {
        let mut metrics_interval = interval(Duration::from_secs(3));
        let mut health_interval = interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                // Handle webserver messages
                Some(request) = async {
                    if let Some(rx) = &mut self.webserver_rx {
                        rx.recv().await
                    } else {
                        None
                    }
                } => {
                    if let Err(e) = self.handle_webserver_request(request).await {
                        process_error!(ProcessId::current(), "❌ Error handling webserver request: {}", e);
                    }
                },

                // Handle producer messages
                Some(update) = async {
                    if let Some(rx) = &mut self.producer_rx {
                        rx.recv().await
                    } else {
                        None
                    }
                } => {
                    if let Err(e) = self.handle_producer_update(update).await {
                        process_error!(ProcessId::current(), "❌ Error handling producer update: {}", e);
                    }
                },

                // Periodic metrics collection and optimization
                _ = metrics_interval.tick() => {
                    if let Err(e) = self.collect_and_send_metrics().await {
                        process_error!(ProcessId::current(), "⚠️ Error collecting metrics: {}. Will retry on next interval.", e);
                        
                        // Log current webserver connection status for debugging
                        process_debug!(ProcessId::current(), "📊 Metrics interval continuing despite communication error");
                    }
                },

                // Periodic health checks
                _ = health_interval.tick() => {
                    if let Err(e) = self.perform_health_checks().await {
                        process_error!(ProcessId::current(), "⚠️ Error during health check: {}", e);
                    }
                },

                // Shutdown signal
                Some(_) = self.shutdown_rx.recv() => {
                    process_debug!(ProcessId::current(), "🛑 Shutting down orchestrator...");
                    self.shutdown().await?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle requests from webserver
    async fn handle_webserver_request(&self, request: WebServerRequest) -> OrchestratorResult<()> {
        match request {
            WebServerRequest::StartGeneration {
                request_id,
                topic,
                producer_count,
                optimization_mode,
                constraints,
                iterations,
            } => {
                // Set iterations in state if provided
                if let Some(iter_limit) = iterations {
                    let mut state = self.state.lock().await;
                    state.set_cli_iterations(Some(iter_limit));
                }

                self.start_generation(request_id, topic, producer_count, optimization_mode, constraints)
                    .await
            }

            WebServerRequest::StopGeneration { request_id } => self.stop_generation(request_id).await,

            WebServerRequest::GetStatus { request_id } => self.send_status_update(request_id).await,

            WebServerRequest::UpdateConfig {
                request_id,
                optimization_mode,
                constraints,
            } => self.update_config(request_id, optimization_mode, constraints).await,

            WebServerRequest::Ready { listen_port, http_port } => {
                self.handle_webserver_ready(listen_port, http_port).await
            }
        }
    }

    /// Handle webserver ready signal
    async fn handle_webserver_ready(&self, listen_port: u16, http_port: u16) -> OrchestratorResult<()> {
        let webserver_addr = SocketAddr::from(([127, 0, 0, 1], listen_port));

        // Mark webserver as ready in communicator
        self.communicator.mark_webserver_ready(webserver_addr).await?;

        process_debug!(
            ProcessId::current(),
            "🚀 WebServer ready: HTTP on port {}, IPC on port {}",
            http_port,
            listen_port
        );

        Ok(())
    }

    /// Handle producer ready signal
    async fn handle_producer_ready(&self, producer_id: ProcessId, listen_port: u16) -> OrchestratorResult<()> {
        let producer_addr = SocketAddr::from(([127, 0, 0, 1], listen_port));

        // Mark producer as ready in communicator
        self.communicator
            .mark_producer_ready(producer_id.clone(), producer_addr)
            .await?;

        process_debug!(
            ProcessId::current(),
            "🚀 Producer {} ready on port {}",
            producer_id,
            listen_port
        );

        // Check if there's a pending start command for this producer
        let pending_command = {
            let mut state = self.state.lock().await;
            state.take_pending_start_command(&producer_id)
        };

        if let Some(command) = pending_command {
            process_debug!(
                ProcessId::current(),
                "📤 Sending queued start command to producer {}",
                producer_id
            );
            self.communicator.send_producer_command(producer_id, command).await?;
        }

        Ok(())
    }

    /// Handle updates from producers
    async fn handle_producer_update(&self, update: ProducerUpdate) -> OrchestratorResult<()> {
        match update {
            ProducerUpdate::AttributeBatch {
                producer_id,
                batch_id: _,
                attributes,
                provider_metadata,
            } => {
                self.process_attribute_batch(producer_id, attributes, provider_metadata)
                    .await
            }

            ProducerUpdate::SyncAck {
                producer_id,
                sync_id: _,
                bloom_version: _,
                status: _,
            } => {
                // Process sync acknowledgment
                self.handle_sync_acknowledgment(producer_id).await
            }

            ProducerUpdate::StatusUpdate {
                producer_id,
                status,
                message: _,
                performance_stats: _,
            } => self.update_producer_status(producer_id, status).await,

            ProducerUpdate::Pong {
                producer_id,
                ping_id: _,
            } => self.handle_pong(producer_id).await,

            ProducerUpdate::Error {
                producer_id,
                error_code,
                message,
                command_id: _,
            } => self.handle_producer_error(producer_id, error_code, message).await,

            ProducerUpdate::Ready {
                producer_id,
                listen_port,
            } => self.handle_producer_ready(producer_id, listen_port).await,
        }
    }

    /// Start generation process with custom request size
    async fn start_generation_with_config(
        &self,
        _request_id: u64,
        topic: String,
        producer_count: u32,
        optimization_mode: OptimizationMode,
        constraints: GenerationConstraints,
        request_size: usize,
        routing_strategy_override: Option<String>,
        routing_provider_override: Option<String>,
    ) -> OrchestratorResult<()> {
        // Get iteration budget from state and log topic start consistently
        let budget_str = {
            let state = self.state.lock().await;
            match state.get_cli_iterations() {
                Some(limit) => format!("with {limit} iteration budget"),
                None => "with no iteration budget".to_string(),
            }
        };
        process_info!(ProcessId::current(), "✅ Topic '{}' started {}", topic, budget_str);

        // Create topic directory
        self.file_system.create_topic_directory(&topic).await?;

        // Get API keys
        let api_keys = self.api_keys.get_api_keys().await?;

        // Update state
        {
            let mut state = self.state.lock().await;
            state.start_generation(topic.clone(), optimization_mode, constraints);
        }

        // Generate initial prompt and configuration with custom request_size
        let (prompt, mut routing_strategy, mut generation_config) = {
            let state = self.state.lock().await;
            let optimizer = state.get_optimizer();
            let optimization = optimizer.optimize_for_topic(
                &topic,
                &state.get_performance_stats(),
                &state.context.optimization_targets.optimization_mode,
            )?;

            tracing::debug!("🎯 CLI Orchestrator generated prompt: '{}'", optimization.optimized_prompt);
            tracing::debug!("🎯 CLI Topic: '{}', Routing: {:?}", topic, optimization.routing_strategy);

            (
                optimization.optimized_prompt,
                optimization.routing_strategy,
                optimization.generation_config,
            )
        };

        // Apply CLI routing overrides if provided
        if let (Some(strategy), Some(provider)) = (&routing_strategy_override, &routing_provider_override) {
            let provider_id = match provider.to_lowercase().as_str() {
                "openai" => shared::ProviderId::OpenAI,
                "anthropic" => shared::ProviderId::Anthropic,
                "gemini" => shared::ProviderId::Gemini,
                "random" => shared::ProviderId::Random,
                _ => {
                    tracing::warn!("⚠️ Unknown routing provider '{}', using Random", provider);
                    shared::ProviderId::Random
                }
            };

            routing_strategy = match strategy.to_lowercase().as_str() {
                "backoff" => shared::RoutingStrategy::Backoff { provider: provider_id },
                "roundrobin" => shared::RoutingStrategy::RoundRobin { providers: vec![provider_id] },
                "priority" => shared::RoutingStrategy::PriorityOrder { providers: vec![provider_id] },
                "weighted" => shared::RoutingStrategy::Weighted { weights: std::collections::HashMap::from([(provider_id, 1.0)]) },
                _ => {
                    tracing::warn!("⚠️ Unknown routing strategy '{}', using Backoff", strategy);
                    shared::RoutingStrategy::Backoff { provider: provider_id }
                }
            };

            tracing::debug!("🎯 CLI routing override: {} with provider {}", strategy, provider);
        } else if api_keys.len() == 1 && api_keys.contains_key(&shared::ProviderId::Random) {
            // Fallback: Override routing strategy for test mode (single Random provider)
            routing_strategy = shared::RoutingStrategy::Backoff {
                provider: shared::ProviderId::Random,
            };
            tracing::debug!("🎯 Test mode: using Random provider fallback");
        }

        // Override request_size with CLI parameter
        generation_config.request_size = request_size;

        // Spawn producers with the finalized routing strategy
        let producer_addr = self.producer_addr.expect("Producer address not initialized");
        let producer_infos = self
            .process_manager
            .spawn_producers(producer_count, &topic, api_keys.clone(), producer_addr, Some(routing_strategy.clone()))
            .await?;

        // Register producers with communicator
        for info in &producer_infos {
            self.communicator
                .register_producer(info.id.clone(), info.command_address)
                .await?;
        }

        // Note: Producers will send Ready signals when their IPC listeners are initialized
        // No more artificial delays needed!

        // Queue start commands for all producers (will be sent when they become ready)
        {
            let mut state = self.state.lock().await;
            for info in &producer_infos {
                let command = OrchestratorCommand::Start {
                    command_id: 1,
                    topic: topic.clone(),
                    prompt: prompt.clone(),
                    routing_strategy: routing_strategy.clone(),
                    generation_config: generation_config.clone(),
                };

                state.queue_start_command(info.id.clone(), command);
            }
        }

        process_debug!(ProcessId::current(), "✅ Generation started successfully");
        Ok(())
    }

    /// Start generation process
    async fn start_generation(
        &self,
        request_id: u64,
        topic: String,
        producer_count: u32,
        optimization_mode: OptimizationMode,
        constraints: GenerationConstraints,
    ) -> OrchestratorResult<()> {
        // Log topic start consistently (webserver mode typically has no iteration limit)
        process_info!(
            ProcessId::current(),
            "✅ Topic '{}' started with no iteration budget",
            topic
        );

        // Create topic directory
        self.file_system.create_topic_directory(&topic).await?;

        // Get API keys
        let api_keys = self.api_keys.get_api_keys().await?;

        // Update state
        {
            let mut state = self.state.lock().await;
            state.start_generation(topic.clone(), optimization_mode, constraints);
        }

        // Spawn producers
        let producer_addr = self.producer_addr.expect("Producer address not initialized");
        let producer_infos = self
            .process_manager
            .spawn_producers(producer_count, &topic, api_keys, producer_addr, None)
            .await?;

        // Register producers with communicator
        for info in &producer_infos {
            self.communicator
                .register_producer(info.id.clone(), info.command_address)
                .await?;
        }

        // Generate initial prompt and configuration
        let (prompt, routing_strategy, generation_config) = {
            let state = self.state.lock().await;
            let optimizer = state.get_optimizer();
            let optimization = optimizer.optimize_for_topic(
                &topic,
                &state.get_performance_stats(),
                &state.context.optimization_targets.optimization_mode,
            )?;

            tracing::debug!("🎯 Orchestrator generated prompt: '{}'", optimization.optimized_prompt);
            tracing::debug!("🎯 Topic: '{}', Routing: {:?}", topic, optimization.routing_strategy);

            (
                optimization.optimized_prompt,
                optimization.routing_strategy,
                optimization.generation_config,
            )
        };

        // Queue start commands for all producers (will be sent when they become ready)
        {
            let mut state = self.state.lock().await;
            for info in &producer_infos {
                let command = OrchestratorCommand::Start {
                    command_id: 1,
                    topic: topic.clone(),
                    prompt: prompt.clone(),
                    routing_strategy: routing_strategy.clone(),
                    generation_config: generation_config.clone(),
                };

                state.queue_start_command(info.id.clone(), command);
                // Update state with active producers
                state.add_producer(info.id.clone(), info.process_id, ProcessStatus::Running);
            }
        }

        // Send acknowledgment to webserver
        let ack = OrchestratorUpdate::RequestAck {
            request_id,
            success: true,
            message: Some(format!("Started generation with {} producers", producer_infos.len())),
        };

        self.communicator.send_webserver_update(ack).await?;

        process_debug!(ProcessId::current(), "✅ Generation started successfully");
        Ok(())
    }

    /// Stop generation process
    async fn stop_generation(&self, request_id: u64) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🛑 Stopping generation...");

        // Stop all producers
        self.process_manager.stop_all().await?;

        // Update state and send completion notification
        {
            let mut state = self.state.lock().await;

            // Send GenerationComplete notification before stopping
            if let Some(topic) = &state.context.topic {
                let current_iteration = state.get_current_iteration();
                let final_unique_count = state.get_unique_attribute_count();

                process_info!(
                    ProcessId::current(),
                    "✅ Topic '{}' completed after {} iterations",
                    topic,
                    current_iteration
                );

                let completion_update = OrchestratorUpdate::GenerationComplete {
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    topic: topic.clone(),
                    total_iterations: current_iteration,
                    final_unique_count,
                    completion_reason: CompletionReason::ManualStop,
                };
                let _ = self.communicator.send_webserver_update(completion_update).await;
            }

            state.stop_generation();
        }

        // Send acknowledgment
        let ack = OrchestratorUpdate::RequestAck {
            request_id,
            success: true,
            message: Some("Generation stopped".to_string()),
        };

        self.communicator.send_webserver_update(ack).await?;

        process_debug!(ProcessId::current(), "✅ Generation stopped");
        Ok(())
    }

    /// Process new batch of attributes from producer
    async fn process_attribute_batch(
        &self,
        producer_id: ProcessId,
        attributes: Vec<String>,
        provider_metadata: shared::ProviderMetadata,
    ) -> OrchestratorResult<()> {
        process_debug!(
            ProcessId::current(),
            "📥 Received {} attributes from producer {}",
            attributes.len(),
            producer_id
        );

        if attributes.is_empty() {
            return Ok(());
        }

        let unique_attributes = {
            let mut state = self.state.lock().await;
            let unique_attrs = state.add_attributes(producer_id.clone(), attributes, &provider_metadata);

            // Store unique attributes to filesystem
            if let Some(topic) = &state.context.topic {
                self.file_system
                    .write_unique_attributes_with_metadata(topic, &unique_attrs, &provider_metadata)
                    .await?;
            }

            unique_attrs
        };

        if !unique_attributes.is_empty() {
            process_debug!(
                ProcessId::current(),
                "📤 Sending {} unique attributes to webserver",
                unique_attributes.len()
            );
            // Always send to webserver if we have unique attributes
            let update = OrchestratorUpdate::NewAttributes {
                attributes: unique_attributes,
                provider_metadata: None, // TODO: Pass actual provider metadata from the batch
            };
            self.communicator.send_webserver_update(update).await?;
        }

        // Check if we've reached iteration limit in CLI mode
        {
            let mut state = self.state.lock().await;

            // Append current iteration items to output.txt before incrementing
            let iteration_items = state.get_current_iteration_items();
            if !iteration_items.is_empty() {
                if let Some(topic) = &state.context.topic {
                    self.file_system.append_to_output(topic, &iteration_items).await?;
                }
            }

            if state.increment_iteration() {
                // Reached iteration limit - initiate shutdown
                process_debug!(
                    ProcessId::current(),
                    "🏁 CLI mode: Iteration limit reached, shutting down"
                );

                // Send GenerationComplete notification to webserver (if it exists)
                if let Some(topic) = &state.context.topic {
                    let current_iteration = state.get_current_iteration();
                    let final_unique_count = state.get_unique_attribute_count();

                    process_info!(
                        ProcessId::current(),
                        "✅ Topic '{}' completed after {} iterations",
                        topic,
                        current_iteration
                    );

                    if self.webserver_rx.is_some() {
                        let completion_update = OrchestratorUpdate::GenerationComplete {
                            timestamp: chrono::Utc::now().timestamp() as u64,
                            topic: topic.clone(),
                            total_iterations: current_iteration,
                            final_unique_count,
                            completion_reason: CompletionReason::IterationLimitReached,
                        };
                        let _ = self.communicator.send_webserver_update(completion_update).await;
                    }

                    // Export performance data
                    if let Err(e) = state.export_cycle_performance(&self.file_system).await {
                        process_error!(ProcessId::current(), "⚠️ Failed to export cycle performance: {}", e);
                    }

                    if let Err(e) = state.export_provider_performance(&self.file_system).await {
                        process_error!(ProcessId::current(), "⚠️ Failed to export provider performance: {}", e);
                    }
                }

                let _ = self.shutdown_tx.send(()).await;
            }
        }

        Ok(())
    }

    /// Update producer status and heal if needed
    async fn update_producer_status(&self, producer_id: ProcessId, status: ProcessStatus) -> OrchestratorResult<()> {
        let mut state = self.state.lock().await;
        state.update_producer_status(producer_id.clone(), status);

        // Only send start command to producers that are running but haven't been started for current topic yet
        if status == ProcessStatus::Running && !state.is_producer_started(&producer_id) {
            if let Some(topic) = &state.context.topic {
                // Generate and send start command to heal the producer
                let optimizer = state.get_optimizer();
                if let Ok(optimization) = optimizer.optimize_for_topic(
                    topic,
                    &state.get_performance_stats(),
                    &state.context.optimization_targets.optimization_mode,
                ) {
                    let command = OrchestratorCommand::Start {
                        command_id: chrono::Utc::now().timestamp_millis() as u64,
                        topic: topic.clone(),
                        prompt: optimization.optimized_prompt,
                        routing_strategy: optimization.routing_strategy,
                        generation_config: optimization.generation_config,
                    };

                    process_debug!(
                        ProcessId::current(),
                        "📋 Sending start command to producer {} for topic '{}' (first time)",
                        producer_id,
                        topic
                    );

                    // Mark producer as started for current topic
                    state.mark_producer_started(producer_id.clone());

                    // Drop the state lock before async call
                    drop(state);

                    // Send the command directly
                    if let Err(e) = self
                        .communicator
                        .send_producer_command(producer_id.clone(), command)
                        .await
                    {
                        process_error!(
                            ProcessId::current(),
                            "❌ Failed to send start command to producer {}: {}",
                            producer_id,
                            e
                        );
                    } else {
                        process_debug!(
                            ProcessId::current(),
                            "✅ Start command sent to producer {} for first time",
                            producer_id
                        );
                    }

                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Handle sync acknowledgment from producer
    async fn handle_sync_acknowledgment(&self, _producer_id: ProcessId) -> OrchestratorResult<()> {
        // For now, just log - could be extended for bloom filter sync tracking
        process_debug!(ProcessId::current(), "📡 Received sync ack from producer");
        Ok(())
    }

    /// Handle pong response from producer
    async fn handle_pong(&self, _producer_id: ProcessId) -> OrchestratorResult<()> {
        // Health check response received
        Ok(())
    }

    /// Handle error from producer
    async fn handle_producer_error(
        &self,
        producer_id: ProcessId,
        error_code: String,
        message: String,
    ) -> OrchestratorResult<()> {
        process_error!(
            ProcessId::current(),
            "❌ Producer {} error [{}]: {}",
            producer_id,
            error_code,
            message
        );

        // Update producer status to failed
        {
            let mut state = self.state.lock().await;
            state.update_producer_status(producer_id, ProcessStatus::Failed);
        }

        Ok(())
    }

    /// Send current system status to webserver
    async fn send_status_update(&self, request_id: u64) -> OrchestratorResult<()> {
        let (metrics, active_producers, current_topic, total_unique) = {
            let state = self.state.lock().await;
            let metrics = state.get_system_metrics();
            let active_producers = state.get_active_producer_count();
            let current_topic = state.context.topic.clone();
            let total_unique = state.get_unique_attribute_count();

            (metrics, active_producers, current_topic, total_unique)
        };

        let ack = OrchestratorUpdate::RequestAck {
            request_id,
            success: true,
            message: Some("Status update".to_string()),
        };

        let stats_update = OrchestratorUpdate::StatisticsUpdate {
            timestamp: chrono::Utc::now().timestamp() as u64,
            active_producers: active_producers as u32,
            current_topic,
            total_unique_attributes: total_unique,
            metrics,
        };

        // Send both acknowledgment and stats
        self.communicator.send_webserver_update(ack).await?;
        self.communicator.send_webserver_update(stats_update).await?;

        Ok(())
    }

    /// Update system configuration
    async fn update_config(
        &self,
        request_id: u64,
        optimization_mode: Option<OptimizationMode>,
        constraints: Option<GenerationConstraints>,
    ) -> OrchestratorResult<()> {
        {
            let mut state = self.state.lock().await;

            if let Some(mode) = optimization_mode {
                state.context.optimization_targets.optimization_mode = mode;
            }

            if let Some(constraints) = constraints {
                state.context.optimization_targets.max_cost_per_minute = constraints.max_cost_per_minute;
                state.context.optimization_targets.min_uam = constraints.target_uam;
            }
        }

        let ack = OrchestratorUpdate::RequestAck {
            request_id,
            success: true,
            message: Some("Configuration updated".to_string()),
        };

        self.communicator.send_webserver_update(ack).await?;

        process_debug!(ProcessId::current(), "⚙️ Configuration updated");
        Ok(())
    }

    /// Collect performance metrics and send to webserver
    async fn collect_and_send_metrics(&self) -> OrchestratorResult<()> {
        let (metrics, active_producers, current_topic, total_unique) = {
            let mut state = self.state.lock().await;

            // Update performance tracker
            state.performance.recalculate_stats();

            // Check if optimization is needed
            let optimizer = state.get_optimizer();
            let performance_stats = state.get_performance_stats();

            // Run optimization if needed
            if let Some(topic) = &state.context.topic {
                if let Ok(optimization) = optimizer.optimize_for_topic(
                    topic,
                    &performance_stats,
                    &state.context.optimization_targets.optimization_mode,
                ) {
                    if optimization.confidence > 0.8 {
                        process_debug!(
                            ProcessId::current(),
                            "🎯 Optimization opportunity found (confidence: {:.1}%): {}",
                            optimization.confidence * 100.0,
                            optimization.rationale
                        );

                        // TODO: Send updated configuration to producers
                        // This would involve sending UpdateConfig commands to active producers
                    }
                }
            }

            let metrics = state.get_system_metrics();
            let active_producers = state.get_active_producer_count();
            let current_topic = state.context.topic.clone();
            let total_unique = state.get_unique_attribute_count();

            (metrics, active_producers, current_topic, total_unique)
        };

        // Send statistics update to webserver only when topic is active
        if current_topic.is_some() && active_producers > 0 {
            process_debug!(ProcessId::current(), "📊 Sending StatisticsUpdate: UAM={:.2}, cost/min=${:.4} (topic active)", 
                          metrics.uam, metrics.cost_per_minute);
            
            let update = OrchestratorUpdate::StatisticsUpdate {
                timestamp: chrono::Utc::now().timestamp() as u64,
                active_producers: active_producers as u32,
                current_topic,
                total_unique_attributes: total_unique,
                metrics,
            };
            
            self.communicator.send_webserver_update(update).await?;
        } else {
            process_debug!(ProcessId::current(), "📭 Skipping StatisticsUpdate: no active topic (topic: {:?}, producers: {})", 
                          current_topic.as_deref().unwrap_or("None"), active_producers);
        }

        Ok(())
    }

    /// Perform periodic health checks and restart failed producers
    async fn perform_health_checks(&self) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🔍 Checking producer pool health");
        
        // Also check webserver connection health by sending a test statistics update
        self.test_webserver_connection().await;

        let health_infos = self.process_manager.check_process_health().await?;

        let mut failed_producers = Vec::new();
        let mut webserver_failed = false;
        
        for health_info in &health_infos {
            if health_info.status == ProcessStatus::Failed {
                if let Some(producer_id) = &health_info.producer_id {
                    process_error!(ProcessId::current(), "🔥 Producer {} has failed", producer_id);
                    failed_producers.push(producer_id.clone());

                    // Update state
                    {
                        let mut state = self.state.lock().await;
                        state.update_producer_status(producer_id.clone(), ProcessStatus::Failed);
                    }
                } else {
                    // This is the webserver
                    process_error!(ProcessId::current(), "🔥 WebServer has failed");
                    webserver_failed = true;
                }
            }
        }

        // Restart failed producers
        if !failed_producers.is_empty() {
            process_info!(
                ProcessId::current(),
                "🔄 Healing producer pool: restarting {} failed producers",
                failed_producers.len()
            );

            for failed_producer_id in &failed_producers {
                if let Err(e) = self.restart_failed_producer(failed_producer_id.clone()).await {
                    process_error!(
                        ProcessId::current(),
                        "❌ Failed to restart producer {}: {}",
                        failed_producer_id,
                        e
                    );
                } else {
                    process_info!(
                        ProcessId::current(),
                        "✅ Producer {} successfully restarted",
                        failed_producer_id
                    );
                }
            }

            // Send error notification to webserver (if it exists)
            if self.webserver_rx.is_some() {
                let status_update = OrchestratorUpdate::ErrorNotification(format!(
                    "{} producers failed and were restarted",
                    failed_producers.len()
                ));
                self.communicator.send_webserver_update(status_update).await?;
            }
        }

        // Restart failed webserver
        if webserver_failed {
            process_info!(
                ProcessId::current(),
                "🔄 Healing webserver: restarting failed webserver"
            );
            
            if let Err(e) = self.restart_failed_webserver().await {
                process_error!(
                    ProcessId::current(),
                    "❌ Failed to restart webserver: {}",
                    e
                );
            } else {
                process_info!(
                    ProcessId::current(),
                    "✅ WebServer successfully restarted"
                );
            }
        }

        Ok(())
    }

    /// Restart a single failed producer
    async fn restart_failed_producer(&self, failed_producer_id: ProcessId) -> OrchestratorResult<()> {
        // Get current topic and API keys for the new producer
        let (topic, api_keys) = {
            let state = self.state.lock().await;
            let topic = state.context.topic.clone();
            (topic, self.api_keys.get_api_keys().await?)
        };

        if let Some(topic) = topic {
            // Spawn a single replacement producer
            let producer_addr = self.producer_addr.expect("Producer address not initialized");
            let producer_infos = self
                .process_manager
                .spawn_producers(1, &topic, api_keys, producer_addr, None)
                .await?;

            if let Some(new_producer_info) = producer_infos.first() {
                // Register the new producer
                self.communicator
                    .register_producer(new_producer_info.id.clone(), new_producer_info.command_address)
                    .await?;

                // Update state: remove old producer and add new one
                {
                    let mut state = self.state.lock().await;
                    state.remove_producer(&failed_producer_id);
                    state.add_producer(
                        new_producer_info.id.clone(),
                        new_producer_info.process_id,
                        ProcessStatus::Running,
                    );

                    // Queue start command for the new producer
                    let optimizer = state.get_optimizer();
                    if let Ok(optimization) = optimizer.optimize_for_topic(
                        &topic,
                        &state.get_performance_stats(),
                        &state.context.optimization_targets.optimization_mode,
                    ) {
                        let command = OrchestratorCommand::Start {
                            command_id: chrono::Utc::now().timestamp_millis() as u64,
                            topic: topic.clone(),
                            prompt: optimization.optimized_prompt,
                            routing_strategy: optimization.routing_strategy,
                            generation_config: optimization.generation_config,
                        };

                        state.queue_start_command(new_producer_info.id.clone(), command);
                    }
                }

                process_debug!(
                    ProcessId::current(),
                    "🏭 Spawned replacement producer {} for failed producer {}",
                    new_producer_info.id,
                    failed_producer_id
                );
            }
        }

        Ok(())
    }

    /// Restart a failed webserver (self-healing)
    async fn restart_failed_webserver(&self) -> OrchestratorResult<()> {
        process_info!(ProcessId::current(), "🔄 Restarting failed webserver");

        // Get the webserver address for spawning
        let webserver_addr = self.webserver_addr.expect("WebServer address not initialized");
        
        // Spawn a replacement webserver
        let webserver_info = self
            .process_manager
            .spawn_webserver(8080, webserver_addr) // Use standard HTTP port
            .await?;

        // Update orchestrator to register the new webserver
        self.communicator
            .register_webserver(webserver_info.api_address)
            .await?;

        process_info!(
            ProcessId::current(),
            "🌐 Spawned replacement webserver (PID: {}) HTTP:{} IPC:{}",
            webserver_info.process_id,
            webserver_info.listen_address.port(),
            webserver_info.api_address.port()
        );

        Ok(())
    }

    /// Graceful shutdown
    async fn shutdown(&self) -> OrchestratorResult<()> {
        process_debug!(ProcessId::current(), "🛑 Starting graceful shutdown...");

        // Export performance data and log topic completion
        {
            let state = self.state.lock().await;
            if let Some(topic) = &state.context.topic {
                let current_iteration = state.get_current_iteration();
                process_info!(
                    ProcessId::current(),
                    "✅ Topic '{}' completed after {} iterations",
                    topic,
                    current_iteration
                );

                // Export cycle performance statistics to JSON
                if let Err(e) = state.export_cycle_performance(&self.file_system).await {
                    process_error!(ProcessId::current(), "⚠️ Failed to export cycle performance: {}", e);
                }

                // Export provider performance statistics to JSON
                if let Err(e) = state.export_provider_performance(&self.file_system).await {
                    process_error!(ProcessId::current(), "⚠️ Failed to export provider performance: {}", e);
                }
            }
        }

        // Stop all processes
        process_debug!(ProcessId::current(), "🛑 Stopped producer producer-40a5c980");
        self.process_manager.stop_all().await?;
        process_debug!(ProcessId::current(), "🛑 All processes stopped");

        // Shutdown communication
        self.communicator.shutdown().await?;
        process_debug!(ProcessId::current(), "🔌 Communication channels shut down");

        // Sync filesystem
        self.file_system.sync_to_disk().await?;
        process_debug!(ProcessId::current(), "💽 File system synced to disk");

        process_debug!(ProcessId::current(), "✅ Orchestrator shutdown complete");
        Ok(())
    }

    /// Get shutdown sender for external shutdown requests
    pub fn get_shutdown_sender(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Test webserver connection health and attempt to restore if needed
    async fn test_webserver_connection(&self) {
        // Only test connection if we have an active topic that would need to send updates
        let has_active_topic = {
            let state = self.state.lock().await;
            state.context.topic.is_some() && state.get_active_producer_count() > 0
        };
        
        if !has_active_topic {
            process_debug!(ProcessId::current(), "🔌 Skipping webserver connection test: no active topic");
            return;
        }
        
        // Try to send a minimal ping-style update to test the connection
        let ping_update = OrchestratorUpdate::ErrorNotification(
            "ping_test".to_string()
        );
        
        match self.communicator.send_webserver_update(ping_update).await {
            Ok(()) => {
                process_debug!(ProcessId::current(), "✅ Webserver connection healthy");
            }
            Err(e) => {
                process_error!(ProcessId::current(), "⚠️ Webserver connection test failed: {}", e);
                // The communicator will have marked webserver as not ready
                // Future connections will be re-established when webserver reconnects
            }
        }
    }
}
