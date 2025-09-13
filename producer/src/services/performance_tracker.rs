//! Performance tracking implementation

use std::collections::HashMap;
use async_trait::async_trait;
use tokio::sync::RwLock;

use shared::ApiFailure;
use crate::error::ProducerResult;
use crate::traits::PerformanceTracker;
use crate::types::ProviderStats;

/// Real performance tracker with in-memory statistics
#[derive(Clone)]
pub struct RealPerformanceTracker {
    stats: std::sync::Arc<RwLock<HashMap<String, ProviderStats>>>,
}

impl RealPerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            stats: std::sync::Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PerformanceTracker for RealPerformanceTracker {
    async fn record_success(
        &self, 
        provider: &str, 
        response_time: std::time::Duration,
        tokens: u32
    ) -> ProducerResult<()> {
        let mut stats = self.stats.write().await;
        let provider_stats = stats.entry(provider.to_string()).or_insert_with(ProviderStats::default);
        
        provider_stats.total_requests += 1;
        provider_stats.successful_requests += 1;
        provider_stats.total_response_time_ms += response_time.as_millis() as u64;
        provider_stats.last_used = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        
        println!("Recorded success for {}: {}ms, {} tokens", provider, response_time.as_millis(), tokens);
        Ok(())
    }

    async fn record_failure(&self, provider: &str, failure: ApiFailure) -> ProducerResult<()> {
        let mut stats = self.stats.write().await;
        let provider_stats = stats.entry(provider.to_string()).or_insert_with(ProviderStats::default);
        
        provider_stats.total_requests += 1;
        provider_stats.failed_requests += 1;
        
        println!("Recorded failure for {}: {:?}", provider, failure);
        Ok(())
    }

    async fn get_stats(&self) -> ProducerResult<HashMap<String, ProviderStats>> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
}