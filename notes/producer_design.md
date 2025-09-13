# Producer Component Design

The producer is an autonomous process that generates attribute candidates using multiple LLM providers and communicates with the orchestrator for uniqueness coordination.

## Core Responsibilities

1. **Multi-Provider LLM Management**: Connect to and manage multiple LLM providers (OpenAI, Anthropic, etc.)
2. **Intelligent Provider Selection**: Choose providers based on weights, health, and performance
3. **Continuous Generation Loop**: Autonomously generate attribute candidates for the given topic
4. **Communication Management**: Handle TCP communication with orchestrator
5. **Performance Tracking**: Monitor provider performance and adapt accordingly
6. **Phase 2 Extensibility**: Support bloom filter pre-filtering when available

## Data Structures

### Core State Management
```rust
pub struct ProducerState {
    // Identity and configuration
    id: ProducerId,
    orchestrator_addr: SocketAddr,
    
    // Direct LLM client implementations (no trait objects)
    openai_client: Option<openai::Client>,
    anthropic_client: Option<anthropic::Client>,
    openai_model: String,
    anthropic_model: String,
    
    // Multi-provider management
    provider_weights: HashMap<String, f32>,
    provider_health: HashMap<String, ProviderHealth>,
    last_provider_used: Option<String>,
    
    // Current task context
    current_topic: Option<String>,
    prompt: String,
    generation_config: GenerationConfig,
    
    // Communication
    tcp_connection: Option<TcpStream>,
    message_sender: mpsc::Sender<ProducerMessage>,
    message_receiver: mpsc::Receiver<ProducerMessage>,
    
    // Performance tracking
    generation_history: VecDeque<GenerationAttempt>,
    provider_statistics: HashMap<String, ProviderStats>,
    
    // Bloom filter state (None in Phase 1, prepared for Phase 2)
    #[cfg(feature = "bloom-filter")]  
    bloom_filter: Option<BloomFilter>,
    #[cfg(feature = "bloom-filter")]
    bloom_stats: BloomFilterStats,
    
    // Loop control
    is_running: Arc<AtomicBool>,
    should_stop: Arc<AtomicBool>,
}

pub struct ProviderHealth {
    consecutive_failures: u32,
    last_success: Option<Instant>,
    last_failure: Option<Instant>,
    average_response_time: Duration,
    current_status: ProviderStatus,
}

pub struct ProviderStats {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    total_response_time: Duration,
    unique_contributions: u64,
    last_used: Option<Instant>,
}

pub struct GenerationAttempt {
    timestamp: Instant,
    provider_used: String,
    prompt_sent: String,
    raw_response: String,
    parsed_candidates: Vec<String>,
    response_time: Duration,
    tokens_used: u32,
    success: bool,
}

pub struct LLMResponse {
    content: String,
    tokens_used: u32,
    prompt_tokens: u32,
    completion_tokens: u32,
    model_used: String,
    response_time: Duration,
}

pub struct ProducerConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openai_model: Option<String>,
    pub anthropic_model: Option<String>,
}
```

### Message Protocol
```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProducerMessage {
    // Orchestrator → Producer
    StartTopic {
        topic: String,
        prompt: String,
        provider_weights: HashMap<String, f32>,
        generation_config: GenerationConfig,
    },
    UpdateConfiguration {
        prompt: Option<String>,
        provider_weights: Option<HashMap<String, f32>>,
        bloom_filter: Option<Vec<u8>>,
    },
    Stop,
    
    // Producer → Orchestrator  
    AttributeBatch {
        producer_id: String,
        attributes: Vec<String>,
        provider_used: String,
        generation_metadata: GenerationMetadata,
        bloom_stats: Option<BloomFilterStats>,
    },
    ProducerStatus {
        producer_id: String,
        status: ProducerStatusType,
        provider_health: HashMap<String, ProviderHealth>,
    },
}
```

## LLM Provider Methods

Direct implementation without trait objects:

```rust
impl ProducerState {
    /// Generate content using OpenAI API
    async fn generate_openai(&self, prompt: &str, config: &GenerationConfig) -> Result<LLMResponse, ProducerError> {
        let openai_client = self.openai_client.as_ref()
            .ok_or(ProducerError::ProviderNotConfigured("openai".to_string()))?;
        
        let start_time = Instant::now();
        
        let request = openai::ChatCompletionRequest {
            model: self.openai_model.clone(),
            messages: vec![
                openai::ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                }
            ],
            temperature: Some(config.temperature),
            max_tokens: Some(config.max_tokens),
            ..Default::default()
        };
        
        let response = openai_client.chat().create(request).await
            .map_err(|e| ProducerError::ApiError(format!("OpenAI API error: {}", e)))?;
            
        let response_time = start_time.elapsed();
        
        if let Some(choice) = response.choices.first() {
            Ok(LLMResponse {
                content: choice.message.content.clone(),
                tokens_used: response.usage.total_tokens,
                prompt_tokens: response.usage.prompt_tokens,
                completion_tokens: response.usage.completion_tokens,
                model_used: response.model.clone(),
                response_time,
            })
        } else {
            Err(ProducerError::EmptyResponse)
        }
    }
    
    /// Generate content using Anthropic API
    async fn generate_anthropic(&self, prompt: &str, config: &GenerationConfig) -> Result<LLMResponse, ProducerError> {
        let anthropic_client = self.anthropic_client.as_ref()
            .ok_or(ProducerError::ProviderNotConfigured("anthropic".to_string()))?;
        
        let start_time = Instant::now();
        
        let request = anthropic::CreateMessageRequest {
            model: self.anthropic_model.clone(),
            messages: vec![
                anthropic::Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                }
            ],
            temperature: config.temperature,
            max_tokens: config.max_tokens,
            ..Default::default()
        };
        
        let response = anthropic_client.messages().create(request).await
            .map_err(|e| ProducerError::ApiError(format!("Anthropic API error: {}", e)))?;
            
        let response_time = start_time.elapsed();
        
        Ok(LLMResponse {
            content: response.content.text.clone(),
            tokens_used: response.usage.input_tokens + response.usage.output_tokens,
            prompt_tokens: response.usage.input_tokens,
            completion_tokens: response.usage.output_tokens,
            model_used: response.model.clone(),
            response_time,
        })
    }
    
    /// Generate content using any configured provider
    async fn generate_with_provider(&self, provider: &str, prompt: &str, config: &GenerationConfig) -> Result<LLMResponse, ProducerError> {
        match provider {
            "openai" => self.generate_openai(prompt, config).await,
            "anthropic" => self.generate_anthropic(prompt, config).await,
            "mock" => self.generate_mock(prompt, config).await,  // For testing
            _ => Err(ProducerError::UnknownProvider(provider.to_string())),
        }
    }
    
    /// Check if a provider is configured
    fn is_provider_configured(&self, provider: &str) -> bool {
        match provider {
            "openai" => self.openai_client.is_some(),
            "anthropic" => self.anthropic_client.is_some(),
            "mock" => true, // Mock always available for testing
            _ => false,
        }
    }
    
    /// Health check for OpenAI
    async fn health_check_openai(&self) -> Result<(), ProducerError> {
        let openai_client = self.openai_client.as_ref()
            .ok_or(ProducerError::ProviderNotConfigured("openai".to_string()))?;
        
        let test_request = openai::ChatCompletionRequest {
            model: self.openai_model.clone(),
            messages: vec![
                openai::ChatMessage {
                    role: "user".to_string(),
                    content: "Test".to_string(),
                }
            ],
            max_tokens: Some(1),
            ..Default::default()
        };
        
        openai_client.chat().create(test_request).await
            .map_err(|e| ProducerError::ApiError(format!("OpenAI health check failed: {}", e)))?;
        
        Ok(())
    }
    
    /// Health check for Anthropic
    async fn health_check_anthropic(&self) -> Result<(), ProducerError> {
        let anthropic_client = self.anthropic_client.as_ref()
            .ok_or(ProducerError::ProviderNotConfigured("anthropic".to_string()))?;
        
        let test_request = anthropic::CreateMessageRequest {
            model: self.anthropic_model.clone(),
            messages: vec![
                anthropic::Message {
                    role: "user".to_string(),
                    content: "Test".to_string(),
                }
            ],
            max_tokens: 1,
            ..Default::default()
        };
        
        anthropic_client.messages().create(test_request).await
            .map_err(|e| ProducerError::ApiError(format!("Anthropic health check failed: {}", e)))?;
        
        Ok(())
    }
}
```

## Producer Initialization

```rust
impl ProducerState {
    pub fn new(id: ProducerId, orchestrator_addr: SocketAddr, config: ProducerConfig) -> Result<Self, ProducerError> {
        // Initialize OpenAI client if API key is provided
        let openai_client = if let Some(api_key) = config.openai_api_key {
            Some(openai::Client::new(api_key))
        } else {
            None
        };
        
        // Initialize Anthropic client if API key is provided
        let anthropic_client = if let Some(api_key) = config.anthropic_api_key {
            Some(anthropic::Client::new(api_key))
        } else {
            None
        };
        
        // Ensure at least one provider is configured
        if openai_client.is_none() && anthropic_client.is_none() {
            return Err(ProducerError::NoProvidersConfigured);
        }
        
        let (message_sender, message_receiver) = mpsc::channel(1000);
        
        Ok(Self {
            id,
            orchestrator_addr,
            openai_client,
            anthropic_client,
            openai_model: config.openai_model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
            anthropic_model: config.anthropic_model.unwrap_or_else(|| "claude-3-5-sonnet".to_string()),
            provider_weights: HashMap::new(),
            provider_health: HashMap::new(),
            last_provider_used: None,
            current_topic: None,
            prompt: String::new(),
            generation_config: GenerationConfig::default(),
            tcp_connection: None,
            message_sender,
            message_receiver,
            generation_history: VecDeque::new(),
            provider_statistics: HashMap::new(),
            #[cfg(feature = "bloom-filter")]
            bloom_filter: None,
            #[cfg(feature = "bloom-filter")]
            bloom_stats: BloomFilterStats::default(),
            is_running: Arc::new(AtomicBool::new(false)),
            should_stop: Arc::new(AtomicBool::new(false)),
        })
    }
    
    /// Create a mock producer for testing
    pub fn new_mock(id: ProducerId, orchestrator_addr: SocketAddr) -> Self {
        let (message_sender, message_receiver) = mpsc::channel(1000);
        
        Self {
            id,
            orchestrator_addr,
            openai_client: None,  // No real clients for testing
            anthropic_client: None,
            openai_model: "mock-gpt".to_string(),
            anthropic_model: "mock-claude".to_string(),
            provider_weights: HashMap::from([
                ("mock".to_string(), 1.0),
            ]),
            provider_health: HashMap::new(),
            last_provider_used: None,
            current_topic: None,
            prompt: String::new(),
            generation_config: GenerationConfig::default(),
            tcp_connection: None,
            message_sender,
            message_receiver,
            generation_history: VecDeque::new(),
            provider_statistics: HashMap::new(),
            #[cfg(feature = "bloom-filter")]
            bloom_filter: None,
            #[cfg(feature = "bloom-filter")]
            bloom_stats: BloomFilterStats::default(),
            is_running: Arc::new(AtomicBool::new(false)),
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Generate mock response for testing
    async fn generate_mock(&self, _prompt: &str, _config: &GenerationConfig) -> Result<LLMResponse, ProducerError> {
        Ok(LLMResponse {
            content: "Mock response: Item1\nItem2\nItem3".to_string(),
            tokens_used: 10,
            prompt_tokens: 5,
            completion_tokens: 5,
            model_used: "mock-model".to_string(),
            response_time: Duration::from_millis(100),
        })
    }
}
```

## Core Producer Loop

### Main Generation Loop
```rust
impl ProducerState {
    pub async fn run_generation_loop(&mut self) -> Result<(), ProducerError> {
        println!("Producer {} starting generation loop", self.id);
        
        while !self.should_stop.load(Ordering::Relaxed) {
            match self.generation_cycle().await {
                Ok(_) => {
                    // Brief pause between generations to avoid overwhelming APIs
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    eprintln!("Generation cycle error: {}", e);
                    // Longer pause on error to avoid rapid retry loops
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            
            // Check for orchestrator messages (configuration updates, stop signals)
            if let Ok(message) = self.message_receiver.try_recv() {
                self.handle_orchestrator_message(message).await?;
            }
        }
        
        println!("Producer {} stopping generation loop", self.id);
        Ok(())
    }
    
    async fn generation_cycle(&mut self) -> Result<(), ProducerError> {
        // 1. Connection Decision - Select provider based on weights and health
        let selected_provider = self.select_provider_intelligently().await?;
        
        // 2. Build context-aware prompt
        let prompt = self.build_context_aware_prompt();
        
        // 3. Make request to selected provider
        let generation_start = Instant::now();
        let llm_response = self.generate_with_provider(&selected_provider, &prompt, &self.generation_config).await?;
        
        // 4. Parse response into candidate attributes
        let candidates = self.parse_llm_response(&llm_response.content)?;
        
        // 5. Bloom filter pre-filtering (None in Phase 1)
        #[cfg(feature = "bloom-filter")]
        let filtered_candidates = self.apply_bloom_filter_if_available(candidates);
        #[cfg(not(feature = "bloom-filter"))]
        let filtered_candidates = candidates;
        
        // 6. Send candidates to orchestrator
        self.send_candidates_to_orchestrator(
            filtered_candidates.clone(),
            selected_provider.clone(),
            &llm_response
        ).await?;
        
        // 7. Update provider performance statistics
        self.update_provider_stats(
            &selected_provider,
            &llm_response,
            generation_start.elapsed(),
            true
        );
        
        // 8. Record generation attempt for analysis
        let attempt = GenerationAttempt {
            timestamp: generation_start,
            provider_used: selected_provider.clone(),
            prompt_sent: prompt,
            raw_response: llm_response.content.clone(),
            parsed_candidates: filtered_candidates,
            response_time: llm_response.response_time,
            tokens_used: llm_response.tokens_used,
            success: true,
        };
        
        self.generation_history.push_back(attempt);
        
        // Keep history bounded
        while self.generation_history.len() > 100 {
            self.generation_history.pop_front();
        }
        
        Ok(())
    }
}
```

### Intelligent Provider Selection
```rust
impl ProducerState {
    async fn select_provider_intelligently(&self) -> Result<String, ProducerError> {
        // Filter out unhealthy and unconfigured providers
        let healthy_providers: Vec<String> = self.provider_weights
            .keys()
            .filter(|provider| {
                // Check if provider is configured
                if !self.is_provider_configured(provider) {
                    return false;
                }
                
                // Check health status
                if let Some(health) = self.provider_health.get(*provider) {
                    health.consecutive_failures < 3 && 
                    health.current_status == ProviderStatus::Healthy
                } else {
                    true // Unknown providers assumed healthy initially
                }
            })
            .cloned()
            .collect();
            
        if healthy_providers.is_empty() {
            return Err(ProducerError::NoHealthyProviders);
        }
        
        // Weighted random selection from healthy providers
        self.weighted_random_selection(&healthy_providers)
    }
    
    fn weighted_random_selection(&self, healthy_providers: &[String]) -> Result<String, ProducerError> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Calculate total weight for healthy providers
        let total_weight: f32 = healthy_providers
            .iter()
            .map(|provider| self.provider_weights.get(provider).unwrap_or(&0.0))
            .sum();
            
        if total_weight <= 0.0 {
            // Fallback to uniform random selection
            let index = rng.gen_range(0..healthy_providers.len());
            return Ok(healthy_providers[index].clone());
        }
        
        // Weighted selection
        let mut random_value = rng.gen::<f32>() * total_weight;
        
        for provider in healthy_providers {
            let weight = self.provider_weights.get(provider).unwrap_or(&0.0);
            random_value -= weight;
            if random_value <= 0.0 {
                return Ok(provider.clone());
            }
        }
        
        // Fallback to last provider if calculation fails
        Ok(healthy_providers.last().unwrap().clone())
    }
}
```

## Communication with Orchestrator

```rust
impl ProducerState {
    async fn send_candidates_to_orchestrator(
        &mut self,
        candidates: Vec<String>,
        provider_used: String,
        llm_response: &LLMResponse
    ) -> Result<(), ProducerError> {
        let metadata = GenerationMetadata {
            response_time_ms: llm_response.response_time.as_millis() as u64,
            tokens_used: llm_response.tokens_used,
            prompt_tokens: llm_response.prompt_tokens,
            completion_tokens: llm_response.completion_tokens,
            provider_status: self.provider_health
                .get(&provider_used)
                .map(|h| h.current_status.clone())
                .unwrap_or(ProviderStatus::Unknown),
        };
        
        let message = ProducerMessage::AttributeBatch {
            producer_id: self.id.to_string(),
            attributes: candidates,
            provider_used,
            generation_metadata: metadata,
            #[cfg(feature = "bloom-filter")]
            bloom_stats: Some(self.bloom_stats.clone()),
            #[cfg(not(feature = "bloom-filter"))]
            bloom_stats: None,
        };
        
        // Serialize message using bincode for efficiency
        let serialized = bincode::serialize(&message)
            .map_err(|e| ProducerError::SerializationError(e.to_string()))?;
            
        // Send length-prefixed message over TCP
        if let Some(ref mut connection) = self.tcp_connection {
            let length = serialized.len() as u32;
            connection.write_all(&length.to_be_bytes()).await?;
            connection.write_all(&serialized).await?;
            connection.flush().await?;
        } else {
            return Err(ProducerError::NoConnection);
        }
        
        Ok(())
    }
    
    async fn handle_orchestrator_message(&mut self, message: ProducerMessage) -> Result<(), ProducerError> {
        match message {
            ProducerMessage::StartTopic { 
                topic, 
                prompt, 
                provider_weights, 
                generation_config 
            } => {
                self.current_topic = Some(topic);
                self.prompt = prompt;
                self.provider_weights = provider_weights;
                self.generation_config = generation_config;
                self.is_running.store(true, Ordering::Relaxed);
            }
            
            ProducerMessage::UpdateConfiguration { 
                prompt, 
                provider_weights,
                bloom_filter  
            } => {
                if let Some(new_prompt) = prompt {
                    self.prompt = new_prompt;
                }
                
                if let Some(weights) = provider_weights {
                    self.provider_weights = weights;
                }
                
                #[cfg(feature = "bloom-filter")]
                if let Some(bloom_data) = bloom_filter {
                    self.bloom_filter = BloomFilter::deserialize(bloom_data).ok();
                }
            }
            
            ProducerMessage::Stop => {
                self.should_stop.store(true, Ordering::Relaxed);
            }
            
            _ => {
                // Handle other message types as needed
            }
        }
        
        Ok(())
    }
}
```

### Prompt Engineering
```rust
impl ProducerState {
    fn build_context_aware_prompt(&self) -> String {
        let topic = self.current_topic.as_ref().unwrap_or(&"unknown topic".to_string());
        
        // Use prompt with topic substitution
        let base_prompt = self.prompt
            .replace("{topic}", topic)
            .replace("{batch_size}", &self.generation_config.batch_size.to_string());
        
        // Add context about generation approach
        let enhanced_prompt = format!("{}\n\nGenerate {} unique, specific examples. Focus on variety and avoid generic responses. Each entry should be on its own line.", 
            base_prompt, 
            self.generation_config.batch_size
        );
        
        enhanced_prompt
    }
    
    fn parse_llm_response(&self, response: &str) -> Result<Vec<String>, ProducerError> {
        let mut candidates = Vec::new();
        
        // Split by newlines and clean up
        for line in response.lines() {
            let trimmed = line.trim();
            
            // Skip empty lines, numbered lists, bullet points
            if trimmed.is_empty() || 
               trimmed.starts_with("- ") || 
               trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) && trimmed.contains('.') {
                continue;
            }
            
            // Remove common prefixes and clean up
            let cleaned = trimmed
                .trim_start_matches("- ")
                .trim_start_matches("* ")
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .trim_start_matches('.')
                .trim_start_matches(')') 
                .trim();
                
            if !cleaned.is_empty() && cleaned.len() > 2 {
                candidates.push(cleaned.to_string());
            }
        }
        
        // Limit to requested batch size
        candidates.truncate(self.generation_config.batch_size as usize);
        
        Ok(candidates)
    }
}
```

## Phase 2 Extensions (Bloom Filter Support)

### Bloom Filter Pre-filtering
```rust
#[cfg(feature = "bloom-filter")]
impl ProducerState {
    fn apply_bloom_filter_if_available(&mut self, candidates: Vec<String>) -> Vec<String> {
        if let Some(ref bloom_filter) = self.bloom_filter {
            let original_count = candidates.len();
            
            let filtered: Vec<String> = candidates
                .into_iter()
                .filter(|candidate| !bloom_filter.contains(candidate))
                .collect();
                
            let filtered_count = filtered.len();
            
            // Update bloom filter statistics
            self.bloom_stats.total_candidates += original_count as u64;
            self.bloom_stats.filtered_candidates += (original_count - filtered_count) as u64;
            self.bloom_stats.filter_effectiveness = 
                self.bloom_stats.filtered_candidates as f64 / self.bloom_stats.total_candidates as f64;
                
            filtered
        } else {
            candidates
        }
    }
}

#[cfg(feature = "bloom-filter")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomFilterStats {
    total_candidates: u64,
    filtered_candidates: u64,
    filter_effectiveness: f64,
    last_filter_update: u64, // timestamp
}
```

## Error Handling and Recovery

### Provider Health Management
```rust
impl ProducerState {
    fn update_provider_stats(&mut self, provider: &str, response: &LLMResponse, elapsed: Duration, success: bool) {
        let stats = self.provider_statistics
            .entry(provider.to_string())
            .or_insert(ProviderStats {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                total_response_time: Duration::ZERO,
                unique_contributions: 0,
                last_used: None,
            });
            
        stats.total_requests += 1;
        stats.last_used = Some(Instant::now());
        
        if success {
            stats.successful_requests += 1;
            stats.total_response_time += response.response_time;
        } else {
            stats.failed_requests += 1;
        }
        
        // Update health tracking
        let health = self.provider_health
            .entry(provider.to_string())
            .or_insert(ProviderHealth {
                consecutive_failures: 0,
                last_success: None,
                last_failure: None,
                average_response_time: Duration::ZERO,
                current_status: ProviderStatus::Unknown,
            });
            
        if success {
            health.consecutive_failures = 0;
            health.last_success = Some(Instant::now());
            health.current_status = ProviderStatus::Healthy;
            health.average_response_time = stats.total_response_time / stats.successful_requests.max(1) as u32;
        } else {
            health.consecutive_failures += 1;
            health.last_failure = Some(Instant::now());
            
            if health.consecutive_failures >= 3 {
                health.current_status = ProviderStatus::Unhealthy;
            }
        }
    }
}
```

## Testing Strategy

### Unit Tests
- Provider selection algorithms
- Response parsing logic  
- Message serialization/deserialization
- Bloom filter integration (Phase 2)

### Integration Tests
- LLM API connectivity (with mocked responses)
- TCP communication with orchestrator
- Configuration update handling
- Error recovery scenarios

This simplified producer design eliminates trait objects while maintaining multi-provider support through direct method implementations. The architecture is cleaner and easier to understand while preserving all the intelligent provider selection and performance tracking capabilities.