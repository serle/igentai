//! Metrics and statistics message types

use serde::{Deserialize, Serialize};

/// Bloom filter statistics for tracking deduplication performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BloomFilterStats {
    pub total_items: usize,
    pub unique_items: usize,
    pub duplicate_items: usize,
    pub filtered_count: usize,
    pub filter_size_bytes: usize,
    pub false_positive_rate: f64,
}

impl BloomFilterStats {
    pub fn new(total_items: usize, unique_items: usize, duplicate_items: usize, filtered_count: usize) -> Self {
        Self {
            total_items,
            unique_items,
            duplicate_items,
            filtered_count,
            filter_size_bytes: 0,
            false_positive_rate: 0.0,
        }
    }
}
