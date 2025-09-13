//! Response processing implementation with integrated bloom filter

use async_trait::async_trait;
use bloom::{ASMS, BloomFilter};
use regex::Regex;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::ProducerResult;
use crate::traits::ResponseProcessor;

/// Real response processor with regex-based extraction and bloom filter deduplication
pub struct RealResponseProcessor {
    extraction_regex: Regex,
    bloom_filter: Arc<RwLock<Option<BloomFilter>>>,
}

impl RealResponseProcessor {
    /// Create new response processor
    pub fn new() -> Self {
        // Regex to extract quoted strings or comma-separated values
        let extraction_regex = Regex::new(r#""([^"]+)"|([^,\n\s]+)"#).unwrap();
        
        Self {
            extraction_regex,
            bloom_filter: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new bloom filter from binary data
    fn deserialize_bloom_filter(data: &[u8]) -> Option<BloomFilter> {
        if data.is_empty() {
            return None;
        }
        
        // Try to deserialize the bloom filter data
        // For now, create a new bloom filter with reasonable defaults
        // In production, this would properly deserialize the received data
        let mut filter = BloomFilter::with_rate(0.01, 10000);
        
        // For demonstration, add the binary data as elements to simulate
        // a populated filter (real implementation would deserialize properly)
        for chunk in data.chunks(8) {
            if chunk.len() >= 4 {
                let value = u32::from_le_bytes([
                    chunk.get(0).copied().unwrap_or(0),
                    chunk.get(1).copied().unwrap_or(0),
                    chunk.get(2).copied().unwrap_or(0),
                    chunk.get(3).copied().unwrap_or(0),
                ]);
                filter.insert(&value);
            }
        }
        
        Some(filter)
    }

    /// Get bloom filter statistics for the last filtering operation
    pub async fn get_last_filter_stats(&self, total_candidates: usize, filtered_count: usize) -> shared::messages::metrics::BloomFilterStats {
        let filter_guard = self.bloom_filter.read().await;
        let _has_filter = filter_guard.is_some();
        
        shared::messages::metrics::BloomFilterStats {
            total_candidates: total_candidates as u64,
            filtered_candidates: filtered_count as u64,
            filter_effectiveness: if total_candidates > 0 {
                (total_candidates - filtered_count) as f64 / total_candidates as f64
            } else {
                0.0
            },
            last_filter_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[async_trait]
impl ResponseProcessor for RealResponseProcessor {
    async fn set_bloom_filter(&self, filter_data: Vec<u8>) -> ProducerResult<()> {
        let mut filter = self.bloom_filter.write().await;
        *filter = Self::deserialize_bloom_filter(&filter_data);
        println!("Updated bloom filter with {} bytes", filter_data.len());
        Ok(())
    }
    
    async fn process_response(&self, response: &str) -> ProducerResult<Vec<String>> {
        // Extract attributes from response
        let attributes = self.extract_attributes(response).await?;
        
        // Filter for duplicates using bloom filter
        let unique_attributes = self.filter_duplicates(&attributes).await?;
        
        Ok(unique_attributes)
    }
    async fn extract_attributes(&self, response: &str) -> ProducerResult<Vec<String>> {
        let mut attributes = Vec::new();
        
        for cap in self.extraction_regex.captures_iter(response) {
            if let Some(quoted) = cap.get(1) {
                let attr = quoted.as_str().trim().to_string();
                if !attr.is_empty() && attr.len() > 2 {
                    attributes.push(attr);
                }
            } else if let Some(unquoted) = cap.get(2) {
                let attr = unquoted.as_str().trim().to_string();
                if !attr.is_empty() && attr.len() > 2 && !attr.contains("assistant") {
                    attributes.push(attr);
                }
            }
        }
        
        // Deduplicate and limit to reasonable size
        attributes.sort();
        attributes.dedup();
        attributes.truncate(10);
        
        Ok(attributes)
    }

    async fn filter_duplicates(&self, attributes: &[String]) -> ProducerResult<Vec<String>> {
        let filter_guard = self.bloom_filter.read().await;
        
        match &*filter_guard {
            None => {
                // No bloom filter set, return all attributes
                Ok(attributes.to_vec())
            }
            Some(filter) => {
                // Use real bloom filter to check for duplicates
                let mut unique_attributes = Vec::new();
                for attr in attributes {
                    if !filter.contains(attr) {
                        unique_attributes.push(attr.clone());
                    }
                }
                Ok(unique_attributes)
            }
        }
    }
}