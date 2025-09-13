# Orchestrator Component Design

The orchestrator is the central coordination component that manages producer processes, handles uniqueness checking, and maintains system state.

## Core Responsibilities

1. **Producer Lifecycle Management**: Start/stop producer processes based on topic requests
2. **Uniqueness Coordination**: Central authority for attribute uniqueness using HashSet
3. **Communication Hub**: Route messages between web server and producers  
4. **State Persistence**: Maintain system state in files for recovery
5. **Performance Monitoring**: Track and optimize producer/provider performance
6. **Dynamic Configuration**: Update producer settings based on performance metrics

## Data Structures

### Core State Management
```rust
pub struct OrchestratorState {
    // Configuration from command line
    producer_count: u32,        // From -n parameter
    webserver_port: u16,        // From -p parameter
    
    // Uniqueness tracking (Phase 1)
    unique_attributes: HashSet<String>,
    total_count: u64,
    
    // Child process management
    producer_processes: HashMap<ProducerId, Child>,
    producer_channels: HashMap<ProducerId, mpsc::Sender<ProducerMessage>>,
    webserver_process: Option<Child>,
    webserver_channel: Option<mpsc::Sender<WebServerMessage>>,
    
    // Statistics and performance tracking
    statistics_engine: StatisticsEngine,
    optimization_state: OptimizationState,
    
    // Current task context (user inputs from web UI)
    current_topic: Option<String>,
    prompt: String,                                    // User-provided initial prompt
    provider_weights: HashMap<String, f32>,            // User-provided initial weights
    
    // Control loop state
    is_running: Arc<AtomicBool>,
    last_optimization_cycle: Instant,
    optimization_interval: Duration,
    
    // File system state
    current_topic_folder: Option<String>,        // Current topic folder name
    output_file: Arc<Mutex<File>>,              // Main output.txt handle
    topic_file: Arc<Mutex<File>>,               // Topic description file
    metrics_file_path: PathBuf,                 // Current topic metrics file
    state_file_path: PathBuf,                   // Current topic state file
    last_state_save: Instant,
    last_file_sync: Instant,
    file_sync_interval: Duration,               // How often to sync files
    pending_file_writes: Vec<String>,
}
```

### Optimization State

```rust
/// Optimization state for weight and prompt refinement
pub struct OptimizationState {
    // Per-producer optimization (enables response partitioning)
    producer_weights: HashMap<ProducerId, HashMap<String, f32>>,    // Per-producer provider weights
    producer_prompts: HashMap<ProducerId, String>,                  // Per-producer custom prompts
    producer_performance: HashMap<ProducerId, ProducerPerformanceHistory>,
    
    // User-provided defaults (from web UI)
    initial_provider_weights: HashMap<String, f32>,    // Original user input
    initial_prompt: String,                             // Original user input
    
    // Weight optimization history
    weight_optimization_history: VecDeque<(Instant, HashMap<ProducerId, HashMap<String, f32>>)>,
    weight_change_threshold: f32,
    last_weight_update: Instant,
    
    // Prompt optimization history (per-producer)
    prompt_optimization_history: VecDeque<(Instant, HashMap<ProducerId, String>)>,
    prompt_performance_correlation: HashMap<ProducerId, PromptEffectivenessTracking>,
    last_prompt_update: Instant,
    
    // Partitioning effectiveness (inferred from prompts)
    response_partitioning_effectiveness: HashMap<ProducerId, PartitioningMetrics>,
    
    // Optimization effectiveness metrics
    optimization_effectiveness: OptimizationEffectiveness,
}
```

### Statistics Engine

```rust
/// Core event types for statistical analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProducerRequestStatistics {    // Renamed from AttributeGenerationEvent
    timestamp: Instant,
    producer_id: ProducerId,
    provider_used: String,
    prompt_hash: u64,
    candidates_generated: u32,
    unique_candidates: u32,
    response_time_ms: u64,
    tokens_used: u32,
    success: bool,
    partition_coherence: Option<f32>,     // How well response fits intended partition
}

/// Statistics engine for performance analysis and optimization
pub struct StatisticsEngine {
    // Time-series data for analysis
    attribute_generation_history: VecDeque<ProducerRequestStatistics>,  // Renamed
    provider_performance_history: HashMap<String, VecDeque<ProviderPerformanceEvent>>,
    producer_performance_history: HashMap<ProducerId, VecDeque<ProducerPerformanceEvent>>,
    
    // Current metrics
    current_metrics: SystemMetrics,
    performance_trends: PerformanceTrends,
    
    // Analysis windows
    short_term_window: Duration,    // 5 minutes for immediate optimization
    long_term_window: Duration,     // 30 minutes for trend analysis
}

impl StatisticsEngine {
    pub fn update_statistics(&mut self, events: Vec<ProducerRequestStatistics>) -> Result<(), StatisticsError> {
        let now = Instant::now();
        
        // Add new events to history
        for event in events {
            self.attribute_generation_history.push_back(event.clone());
            
            // Update provider-specific history
            let provider_event = ProviderPerformanceEvent {
                timestamp: event.timestamp,
                provider_name: event.provider_used.clone(),
                response_time_ms: event.response_time_ms,
                tokens_generated: event.tokens_used,
                success_rate: if event.success { 1.0 } else { 0.0 },
                unique_contribution_rate: event.unique_candidates as f32 / event.candidates_generated.max(1) as f32,
                cost_efficiency: self.calculate_cost_efficiency(&event),
                error_type: if !event.success { Some("generation_failed".to_string()) } else { None },
            };
            
            self.provider_performance_history
                .entry(event.provider_used.clone())
                .or_insert_with(VecDeque::new)
                .push_back(provider_event);
        }
        
        // Clean up old events outside analysis windows
        self.cleanup_old_events(now);
        
        // Update current metrics
        self.update_current_metrics();
        
        Ok(())
    }
}
```

### Producer Message Updates
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProducerMessage {
    // Orchestrator → Producer
    StartTopic {
        topic: String,
        prompt: String,                               // Producer-specific prompt
        provider_weights: HashMap<String, f32>,       // Producer-specific weights
        generation_config: GenerationConfig,
    },
    UpdateConfiguration {
        prompt: Option<String>,                       // Updated prompt
        provider_weights: Option<HashMap<String, f32>>, // Updated weights
        bloom_filter: Option<Vec<u8>>,                // Bloom filter (None in Phase 1)
    },
    Stop,
    
    // Producer → Orchestrator  
    AttributeBatch {
        producer_id: String,
        attributes: Vec<String>,
        provider_used: String,
        generation_metadata: GenerationMetadata,
        partition_analysis: Option<PartitionAnalysis>, // How well attributes fit intended partition
        bloom_stats: Option<BloomFilterStats>,         // Bloom filter statistics (None in Phase 1)
    },
}
```

## Topic-Based File Organization

### Folder Structure
```
igentai/
├── outputs/                           # All topic outputs
│   ├── paris_attractions/             # Sanitized topic name
│   │   ├── topic.txt                  # Topic description and metadata
│   │   ├── output.txt                 # Unique attributes for this topic
│   │   ├── metrics.json               # Performance metrics for this session
│   │   └── state.json                 # System state snapshot
│   ├── programming_languages/
│   │   ├── topic.txt
│   │   ├── output.txt
│   │   ├── metrics.json
│   │   └── state.json
│   └── machine_learning/
└── global_state.json                  # Global system state
```

### Topic Name Sanitization
```rust
fn sanitize_topic_name(topic: &str) -> String {
    topic
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("_")
}
```

### Example Transformations
```
"Paris attractions" → "paris_attractions/"
"Programming Languages!" → "programming_languages/" 
"Best Sci-Fi Movies (2020s)" → "best_sci_fi_movies_2020s/"
"Machine Learning" → "machine_learning/"
```

### File Organization Methods
```rust
impl OrchestratorState {
    /// Create topic folder and initialize files
    async fn create_topic_folder(&mut self, topic: &str) -> Result<(), OrchestratorError> {
        // Sanitize topic name for folder
        let folder_name = Self::sanitize_topic_name(topic);
        let folder_path = PathBuf::from("outputs").join(&folder_name);
        
        // Remove existing folder if it exists (overwrite behavior)
        if folder_path.exists() {
            tokio::fs::remove_dir_all(&folder_path).await?;
            println!("Overwriting existing topic folder: {}", folder_name);
        }
        
        // Create output directory
        tokio::fs::create_dir_all(&folder_path).await?;
        
        // Initialize topic.txt file
        let topic_content = format!(
            "Topic: {}\nStart Time: {} UTC\nProducer Count: {}\nPrompt: {}\nProvider Weights: {}\n",
            topic,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"),
            self.producer_count,
            self.prompt,
            serde_json::to_string(&self.provider_weights)?
        );
        
        tokio::fs::write(folder_path.join("topic.txt"), topic_content).await?;
        
        // Set up file paths
        let output_file_path = folder_path.join("output.txt");
        let metrics_file_path = folder_path.join("metrics.json");
        let state_file_path = folder_path.join("state.json");
        
        // Open output file handle
        let output_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_file_path)
            .await?;
        
        // Update orchestrator state
        self.current_topic_folder = Some(folder_name.clone());
        self.output_file = Arc::new(Mutex::new(output_file));
        self.metrics_file_path = metrics_file_path;
        self.state_file_path = state_file_path;
        
        println!("Created topic folder: outputs/{}/", folder_name);
        Ok(())
    }
    
    fn sanitize_topic_name(topic: &str) -> String {
        topic
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join("_")
    }
    
    /// Periodic file synchronization for current topic
    async fn periodic_file_sync(&mut self) -> Result<(), OrchestratorError> {
        let now = Instant::now();
        
        if now.duration_since(self.last_file_sync) < self.file_sync_interval {
            return Ok(());
        }
        
        // Flush pending writes to topic-specific output.txt
        if !self.pending_file_writes.is_empty() {
            let mut file = self.output_file.lock().await;
            for attribute in &self.pending_file_writes {
                writeln!(file, "{}", attribute).await?;
            }
            file.flush().await?;
            
            self.pending_file_writes.clear();
        }
        
        // Update topic-specific metrics.json
        self.save_topic_metrics().await?;
        
        // Update topic-specific state.json
        self.save_topic_state().await?;
        
        self.last_file_sync = now;
        Ok(())
    }
}
```

### Overwrite Behavior
- If folder already exists for a topic, it gets completely removed and recreated
- This ensures clean start for each topic run
- Previous results are overwritten, not versioned
- Simpler management without timestamp complexity

## Core Methods

### Producer Lifecycle Management
```rust
impl OrchestratorState {
    pub async fn start_producers(&mut self, topic: String) -> Result<(), OrchestratorError> {
        // Create topic-specific output folder first
        self.create_topic_folder(&topic).await?;
        
        // Stop existing producers
        self.stop_producers().await?;
        
        // Clear state for new topic
        self.reset_for_new_topic(topic.clone()).await?;
        
        // Initialize per-producer configurations for optimal partitioning
        self.initialize_producer_configurations(&topic).await?;
        
        // Spawn producer processes
        for i in 0..self.producer_count {
            let producer_id = ProducerId::new();
            let tcp_port = 8083 + i as u16;
            
            // Spawn process
            let child = Command::new("./igentai")
                .args(&[
                    "producer", 
                    "--orchestrator-addr", &format!("localhost:{}", tcp_port),
                    "--producer-id", &producer_id.to_string()
                ])
                .spawn()?;
            
            // Set up TCP communication channel
            let tcp_listener = TcpListener::bind(("localhost", tcp_port)).await?;
            let (tx, rx) = mpsc::channel(1000);
            
            // Store producer process and channel
            self.producer_processes.insert(producer_id.clone(), child);
            self.producer_channels.insert(producer_id.clone(), tx);
            
            // Start TCP connection handler for this producer
            self.start_producer_connection_handler(producer_id.clone(), tcp_listener, rx).await?;
            
            // Send personalized start message with per-producer configuration
            self.send_personalized_start_message(producer_id, topic.clone()).await?;
        }
        
        self.current_topic = Some(topic);
        Ok(())
    }
    
    /// Initialize per-producer configurations for optimal partitioning
    async fn initialize_producer_configurations(&mut self, topic: &str) -> Result<(), OrchestratorError> {
        let partition_strategy = self.determine_partition_strategy(topic);
        
        for i in 0..self.producer_count {
            let producer_id = ProducerId::new();
            
            // Generate specialized prompt for this producer's partition
            let specialized_prompt = self.generate_partition_specific_prompt(topic, i, &partition_strategy);
            
            // Initialize per-producer provider weights (can be specialized later)
            let initial_weights = self.provider_weights.clone();  // From user input
            
            // Store per-producer configuration
            self.optimization_state.producer_prompts.insert(producer_id.clone(), specialized_prompt);
            self.optimization_state.producer_weights.insert(producer_id, initial_weights);
        }
        
        Ok(())
    }
    
    /// Generate specialized prompts for response partitioning
    fn generate_partition_specific_prompt(&self, topic: &str, producer_index: u32, strategy: &PartitionStrategy) -> String {
        match strategy {
            PartitionStrategy::CategoryBased(categories) => {
                if let Some(category) = categories.get(producer_index as usize % categories.len()) {
                    format!("{} Focus specifically on {} related to {}. Generate unique examples in this category only.", 
                           self.prompt, category, topic)
                } else {
                    self.prompt.clone()
                }
            }
            PartitionStrategy::AlphabeticalRange(ranges) => {
                if let Some(range) = ranges.get(producer_index as usize % ranges.len()) {
                    format!("{} Generate unique examples starting with letters {}-{}.", 
                           self.prompt, range.start, range.end)
                } else {
                    self.prompt.clone()
                }
            }
            PartitionStrategy::SemanticClusters(clusters) => {
                if let Some(cluster) = clusters.get(producer_index as usize % clusters.len()) {
                    format!("{} Focus on {} aspects. Generate examples that specifically relate to this domain.", 
                           self.prompt, cluster)
                } else {
                    self.prompt.clone()
                }
            }
            PartitionStrategy::None => self.prompt.clone(),
        }
    }
    
    /// Send personalized start message with per-producer configuration
    async fn send_personalized_start_message(&mut self, producer_id: ProducerId, topic: String) -> Result<(), OrchestratorError> {
        let producer_prompt = self.optimization_state.producer_prompts
            .get(&producer_id)
            .cloned()
            .unwrap_or_else(|| self.prompt.clone());
            
        let producer_weights = self.optimization_state.producer_weights
            .get(&producer_id)
            .cloned()
            .unwrap_or_else(|| self.provider_weights.clone());
        
        let message = ProducerMessage::StartTopic {
            topic,
            prompt: producer_prompt,
            provider_weights: producer_weights,
            generation_config: self.get_generation_config(),
        };
        
        if let Some(sender) = self.producer_channels.get(&producer_id) {
            sender.send(message).await?;
        }
        
        Ok(())
    }
    
    pub async fn stop_producers(&mut self) -> Result<(), OrchestratorError> {
        // Send stop messages to all producers
        for (producer_id, sender) in &self.producer_channels {
            let _ = sender.send(ProducerMessage::Stop).await;
        }
        
        // Wait for graceful shutdown or force kill after timeout
        tokio::time::timeout(Duration::from_secs(10), async {
            for (producer_id, mut child) in self.producer_processes.drain() {
                let _ = child.wait().await;
            }
        }).await.unwrap_or_else(|_| {
            // Force kill remaining processes
            for (_, mut child) in self.producer_processes.drain() {
                let _ = child.kill();
            }
        });
        
        // Clear producer state
        self.producer_processes.clear();
        self.producer_channels.clear();
        
        Ok(())
    }
}
```

### Uniqueness Checking (Phase 1)
```rust
impl OrchestratorState {
    pub async fn process_attribute_batch(
        &mut self, 
        producer_id: ProducerId,
        candidates: Vec<String>,
        generation_metadata: GenerationMetadata
    ) -> Result<Vec<String>, OrchestratorError> {
        let mut newly_unique = Vec::new();
        let batch_timestamp = Instant::now();
        
        // Check each candidate against HashSet
        for candidate in candidates {
            let is_new = self.unique_attributes.insert(candidate.clone());
            if is_new {
                newly_unique.push(candidate.clone());
                self.total_count += 1;
                
                // Add to pending file writes
                self.pending_file_writes.push(candidate);
            }
        }
        
        // Create statistics event for performance tracking
        let stats_event = ProducerRequestStatistics {
            timestamp: batch_timestamp,
            producer_id: producer_id.clone(),
            provider_used: generation_metadata.provider_used.clone(),
            prompt_hash: generation_metadata.prompt_hash,
            candidates_generated: generation_metadata.candidates_generated,
            unique_candidates: newly_unique.len() as u32,
            response_time_ms: generation_metadata.response_time_ms,
            tokens_used: generation_metadata.tokens_used,
            success: generation_metadata.success,
            partition_coherence: generation_metadata.partition_coherence,
        };
        
        // Update statistics engine
        self.statistics_engine.update_statistics(vec![stats_event])?;
        
        // Trigger file writes if threshold reached
        if self.pending_file_writes.len() >= 10 {
            self.flush_pending_writes().await?;
        }
        
        // Send updates to web server if available
        if !newly_unique.is_empty() {
            if let Some(sender) = &self.webserver_channel {
                let attribute_updates: Vec<AttributeUpdate> = newly_unique.iter().map(|attr| {
                    AttributeUpdate {
                        content: attr.clone(),
                        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        producer_id: producer_id.to_string(),
                        llm_provider: generation_metadata.provider_used.clone(),
                        is_unique: true,
                    }
                }).collect();
                
                let message = WebServerMessage::NewAttributes(attribute_updates);
                let _ = sender.send(message).await;
            }
        }
        
        Ok(newly_unique)
    }
    
    /// Dynamic optimization based on performance metrics
    pub async fn optimize_producer_configurations(&mut self) -> Result<(), OrchestratorError> {
        let now = Instant::now();
        
        // Only optimize if enough time has passed
        if now.duration_since(self.last_optimization_cycle) < self.optimization_interval {
            return Ok(());
        }
        
        // Analyze current performance metrics
        let performance_analysis = self.statistics_engine.analyze_performance_trends()?;
        
        // Update per-producer weights based on provider performance
        for producer_id in self.producer_channels.keys() {
            if let Some(optimized_weights) = self.calculate_optimized_weights(producer_id, &performance_analysis) {
                self.optimization_state.producer_weights.insert(producer_id.clone(), optimized_weights.clone());
                
                // Send updated configuration to producer
                let update_message = ProducerMessage::UpdateConfiguration {
                    prompt: None,
                    provider_weights: Some(optimized_weights),
                    bloom_filter: None,
                };
                
                if let Some(sender) = self.producer_channels.get(producer_id) {
                    let _ = sender.send(update_message).await;
                }
            }
        }
        
        self.last_optimization_cycle = now;
        Ok(())
    }
    
    async fn orchestrator_cycle(&mut self) -> Result<(), OrchestratorError> {
        // 1. Process incoming messages from producers and web server
        self.process_incoming_messages().await?;
        
        // 2. Update statistics with recent data
        self.update_statistics().await?;
        
        // 3. Periodic file synchronization to topic folder
        self.periodic_file_sync().await?;
        
        // 4. Check if optimization cycle is due
        if self.should_run_optimization_cycle() {
            self.run_optimization_cycle().await?;
        }
        
        // 5. Send periodic updates to web server
        self.send_metrics_to_webserver().await?;
        
        Ok(())
    }
}
```

## File System Operations

### State Persistence
```rust
impl OrchestratorState {
    async fn flush_pending_writes(&mut self) -> Result<(), OrchestratorError> {
        if self.pending_file_writes.is_empty() {
            return Ok(());
        }
        
        // Write to output.txt (append mode)
        let mut file = self.output_file.lock().await;
        for attribute in &self.pending_file_writes {
            writeln!(file, "{}", attribute).await?;
        }
        file.flush().await?;
        
        self.pending_file_writes.clear();
        
        // Update metrics.json
        self.save_state_to_file().await?;
        
        Ok(())
    }
    
    async fn save_state_to_file(&self) -> Result<(), OrchestratorError> {
        let state = StateSnapshot {
            current_topic: self.current_topic.clone(),
            total_unique_count: self.total_count,
            active_producer_count: self.producer_processes.len() as u32,
            provider_weights: self.provider_weights.clone(),
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };
        
        let state_json = serde_json::to_string_pretty(&state)?;
        tokio::fs::write("state.json", state_json).await?;
        
        Ok(())
    }
}
```

## Testing Strategy

### Unit Tests
- Message serialization/deserialization
- Uniqueness checking logic
- Producer lifecycle management
- File I/O operations

### Integration Tests  
- Producer communication via TCP
- WebSocket communication with web server
- File persistence and recovery
- Performance optimization algorithms

This design provides a robust foundation for the orchestrator component while maintaining extensibility for Phase 2 bloom filter enhancements.