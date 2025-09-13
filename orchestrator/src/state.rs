//! Orchestrator state management
//!
//! Pure state management for uniqueness tracking and producer coordination
//! that can be tested independently without external dependencies.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use shared::{ProducerId, ProducerStatus, SystemMetrics};
use crate::error::{OrchestratorError, OrchestratorResult};

/// Core orchestrator state management
pub struct OrchestratorState {
    // Uniqueness tracking
    unique_attributes: HashSet<String>,
    total_count: u64,
    
    // Producer tracking
    active_producers: HashMap<ProducerId, ProducerInfo>,
    
    // Current task context
    current_topic: Option<String>,
    prompt: String,
    provider_weights: HashMap<String, f32>,
    
    // Performance tracking
    recent_additions: VecDeque<(Instant, String, ProducerId)>,
    last_stats_update: Instant,
}

#[derive(Clone, Debug)]
pub struct ProducerInfo {
    pub id: ProducerId,
    pub total_contributed: u64,
    pub recent_contributions: VecDeque<Instant>,
    pub current_rate: f64,
    pub status: ProducerStatus,
}



impl OrchestratorState {
    pub fn new() -> Self {
        Self {
            unique_attributes: HashSet::new(),
            total_count: 0,
            active_producers: HashMap::new(),
            current_topic: None,
            prompt: "Generate unique examples of {topic}. Be specific and avoid generic responses.".to_string(),
            provider_weights: HashMap::from([
                ("openai".to_string(), 0.6),
                ("anthropic".to_string(), 0.4),
            ]),
            recent_additions: VecDeque::new(),
            last_stats_update: Instant::now(),
        }
    }

    /// Process attribute batch for uniqueness checking
    pub fn process_result(&mut self, producer_id: ProducerId, candidates: Vec<String>) -> OrchestratorResult<Vec<String>> {
        let mut newly_unique = Vec::new();
        let batch_timestamp = Instant::now();
        
        for candidate in candidates {
            if self.unique_attributes.insert(candidate.clone()) {
                newly_unique.push(candidate.clone());
                self.total_count += 1;
                self.recent_additions.push_back((batch_timestamp, candidate, producer_id.clone()));
            }
        }
        
        // Update producer statistics
        if let Some(producer_info) = self.active_producers.get_mut(&producer_id) {
            producer_info.total_contributed += newly_unique.len() as u64;
            for _ in &newly_unique {
                producer_info.recent_contributions.push_back(batch_timestamp);
            }
        }
        
        Ok(newly_unique)
    }

    /// Initialize producer tracking for topic
    pub fn init_producer_config(&mut self, topic: &str, producer_count: u32) -> OrchestratorResult<()> {
        if topic.trim().is_empty() {
            return Err(OrchestratorError::ConfigurationError { 
                field: "topic cannot be empty".to_string() 
            });
        }
        
        self.current_topic = Some(topic.to_string());
        self.active_producers.clear();
        
        // Initialize producer tracking
        for _i in 0..producer_count {
            let producer_id = ProducerId::new();
            let producer_info = ProducerInfo {
                id: producer_id.clone(),
                total_contributed: 0,
                recent_contributions: VecDeque::new(),
                current_rate: 0.0,
                status: ProducerStatus::Starting,
            };
            
            self.active_producers.insert(producer_id, producer_info);
        }
        
        Ok(())
    }

    /// Get current system metrics using shared type
    pub fn get_metrics(&self, start_time: Instant) -> SystemMetrics {
        let uptime = start_time.elapsed().as_secs();
        let entries_per_minute = if uptime > 0 {
            (self.total_count as f64 * 60.0) / uptime as f64
        } else {
            0.0
        };
        
        SystemMetrics {
            total_unique_entries: self.total_count,
            entries_per_minute,
            per_llm_performance: HashMap::new(), // Would be populated with real performance data
            current_topic: self.current_topic.clone(),
            active_producers: self.active_producers.len() as u32,
            uptime_seconds: uptime,
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }
    }

    /// Reset state for new topic
    pub fn reset_for_topic(&mut self, topic: String) {
        self.unique_attributes.clear();
        self.total_count = 0;
        self.current_topic = Some(topic);
        self.active_producers.clear();
        self.recent_additions.clear();
    }
    
    // Accessors for testing
    pub fn total_count(&self) -> u64 {
        self.total_count
    }
    
    pub fn current_topic(&self) -> Option<&str> {
        self.current_topic.as_deref()
    }
    
    pub fn unique_attributes(&self) -> &HashSet<String> {
        &self.unique_attributes
    }
    
    pub fn active_producers(&self) -> &HashMap<ProducerId, ProducerInfo> {
        &self.active_producers
    }
}