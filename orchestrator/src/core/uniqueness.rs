//! Uniqueness tracking with bloom filter support
//! 
//! This module handles deduplication of attributes using both
//! exact matching (HashSet) and probabilistic filtering (Bloom filter).

use std::collections::HashSet;
use growable_bloom_filter::GrowableBloom;
use serde_json;
use crate::error::OrchestratorResult;

/// Manages uniqueness checking and bloom filter distribution
pub struct UniquenessTracker {
    /// Exact set of unique items (source of truth)
    unique_items: HashSet<String>,
    
    /// Unique items found in current iteration
    current_iteration_items: Vec<String>,
    
    /// Bloom filter for fast probabilistic checking
    bloom_filter: GrowableBloom,
    
    /// Current bloom filter version (increments on updates)
    bloom_version: u64,
    
    /// Statistics for optimization
    stats: UniquenessStats,
}

/// Statistics about uniqueness checking performance
#[derive(Debug, Clone, Default)]
pub struct UniquenessStats {
    pub total_items_processed: u64,
    pub unique_items_found: u64,
    pub bloom_filter_hits: u64,
    pub bloom_filter_misses: u64,
    pub false_positive_rate: f64,
}

impl UniquenessTracker {
    /// Create new uniqueness tracker
    pub fn new() -> Self {
        Self {
            unique_items: HashSet::new(),
            current_iteration_items: Vec::new(),
            bloom_filter: Self::create_bloom_filter(100_000), // 100K expected items
            bloom_version: 0,
            stats: UniquenessStats::default(),
        }
    }
    
    /// Reset state for new topic
    pub fn reset(&mut self) {
        self.unique_items.clear();
        self.current_iteration_items.clear();
        self.bloom_filter = Self::create_bloom_filter(100_000);
        self.bloom_version = 0;
        self.stats = UniquenessStats::default();
    }
    
    /// Filter out non-unique items from a batch
    pub fn filter_unique(&mut self, items: Vec<String>) -> OrchestratorResult<Vec<String>> {
        let mut unique_items = Vec::new();
        let mut bloom_updated = false;
        
        for item in items {
            self.stats.total_items_processed += 1;
            
            // First check bloom filter for quick rejection
            if self.bloom_filter.contains(&item) {
                self.stats.bloom_filter_hits += 1;
                
                // Bloom filter says it might exist, check exact set
                if !self.unique_items.contains(&item) {
                    // False positive - item is actually unique
                    self.add_unique_item(item.clone())?;
                    unique_items.push(item);
                    bloom_updated = true;
                }
                // else: item is truly duplicate, skip it
            } else {
                // Bloom filter says it's definitely unique
                self.stats.bloom_filter_misses += 1;
                self.add_unique_item(item.clone())?;
                unique_items.push(item);
                bloom_updated = true;
            }
        }
        
        // Update bloom filter version if we added new items
        if bloom_updated {
            self.bloom_version += 1;
            self.update_false_positive_rate();
        }
        
        Ok(unique_items)
    }
    
    /// Add a unique item to both storage and bloom filter
    fn add_unique_item(&mut self, item: String) -> OrchestratorResult<()> {
        if self.unique_items.insert(item.clone()) {
            self.bloom_filter.insert(&item);
            self.stats.unique_items_found += 1;
            
            // Track for current iteration
            self.current_iteration_items.push(item.clone());
            
            // Check if we need to rebuild bloom filter for efficiency
            if self.should_rebuild_bloom_filter() {
                self.rebuild_bloom_filter()?;
            }
        }
        Ok(())
    }
    
    /// Get serialized bloom filter data for distribution
    pub fn get_bloom_filter_data(&self) -> Option<Vec<u8>> {
        // Serialize the actual bloom filter using serde_json
        match serde_json::to_vec(&self.bloom_filter) {
            Ok(serialized) => Some(serialized),
            Err(e) => {
                shared::process_error!(shared::ProcessId::current(), "Failed to serialize bloom filter: {}", e);
                None
            }
        }
    }
    
    /// Get current bloom filter version
    pub fn get_bloom_version(&self) -> u64 {
        self.bloom_version
    }
    
    /// Check if bloom filter should be distributed (e.g., after significant updates)
    pub fn should_distribute_bloom_filter(&self) -> bool {
        // Distribute every 100 new unique items or every version increment
        self.stats.unique_items_found % 100 == 0 && self.stats.unique_items_found > 0
    }
    
    /// Get total count of unique items
    pub fn total_unique_count(&self) -> u64 {
        self.unique_items.len() as u64
    }
    
    /// Get current statistics
    pub fn get_stats(&self) -> &UniquenessStats {
        &self.stats
    }
    
    /// Get unique attributes found in current iteration
    pub fn get_current_iteration_items(&self) -> &[String] {
        &self.current_iteration_items
    }
    
    /// Clear current iteration items and start next iteration
    pub fn start_next_iteration(&mut self) {
        self.current_iteration_items.clear();
    }
    
    /// Create a new bloom filter with given capacity
    fn create_bloom_filter(expected_items: usize) -> GrowableBloom {
        // 1% false positive rate, with capacity for expected items
        GrowableBloom::new(0.01, expected_items)
    }
    
    /// Check if bloom filter should be rebuilt for efficiency
    fn should_rebuild_bloom_filter(&self) -> bool {
        let current_size = self.unique_items.len();
        
        // Rebuild if:
        // 1. We have more than 2x the items we planned for
        // 2. False positive rate is too high (>5%)
        current_size > 200_000 || self.stats.false_positive_rate > 0.05
    }
    
    /// Rebuild bloom filter with optimal size
    fn rebuild_bloom_filter(&mut self) -> OrchestratorResult<()> {
        let current_size = self.unique_items.len();
        let new_capacity = std::cmp::max(current_size * 2, 100_000);
        
        // Create new bloom filter with better capacity
        self.bloom_filter = Self::create_bloom_filter(new_capacity);
        
        // Re-insert all unique items
        for item in &self.unique_items {
            self.bloom_filter.insert(item);
        }
        
        // Update version and reset stats
        self.bloom_version += 1;
        self.stats.bloom_filter_hits = 0;
        self.stats.bloom_filter_misses = 0;
        self.stats.false_positive_rate = 0.0;
        
        Ok(())
    }
    
    /// Update false positive rate calculation
    fn update_false_positive_rate(&mut self) {
        let total_bloom_checks = self.stats.bloom_filter_hits + self.stats.bloom_filter_misses;
        if total_bloom_checks > 0 {
            // This is a simplified calculation - in practice we'd need more sophisticated tracking
            self.stats.false_positive_rate = self.stats.bloom_filter_hits as f64 / total_bloom_checks as f64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_uniqueness_tracking() {
        let mut tracker = UniquenessTracker::new();
        
        // First batch - all should be unique
        let items1 = vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()];
        let unique1 = tracker.filter_unique(items1).unwrap();
        assert_eq!(unique1.len(), 3);
        assert_eq!(tracker.total_unique_count(), 3);
        
        // Second batch - some duplicates
        let items2 = vec!["apple".to_string(), "date".to_string(), "banana".to_string()];
        let unique2 = tracker.filter_unique(items2).unwrap();
        assert_eq!(unique2.len(), 1); // Only "date" is new
        assert_eq!(tracker.total_unique_count(), 4);
        
        assert!(unique2.contains(&"date".to_string()));
        assert!(!unique2.contains(&"apple".to_string()));
    }
    
    #[test]
    fn test_bloom_filter_versioning() {
        let mut tracker = UniquenessTracker::new();
        let initial_version = tracker.get_bloom_version();
        
        // Add some items
        let items = vec!["item1".to_string(), "item2".to_string()];
        tracker.filter_unique(items).unwrap();
        
        // Version should have incremented
        assert!(tracker.get_bloom_version() > initial_version);
    }
    
    #[test]
    fn test_bloom_filter_data_serialization() {
        let mut tracker = UniquenessTracker::new();
        
        // Add some items
        let items = vec!["test1".to_string(), "test2".to_string()];
        tracker.filter_unique(items).unwrap();
        
        // Should be able to get serialized data
        let bloom_data = tracker.get_bloom_filter_data();
        assert!(bloom_data.is_some());
        assert!(!bloom_data.unwrap().is_empty());
    }
    
    #[test]
    fn test_reset_functionality() {
        let mut tracker = UniquenessTracker::new();
        
        // Add some items
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        tracker.filter_unique(items).unwrap();
        assert_eq!(tracker.total_unique_count(), 3);
        
        // Reset should clear everything
        tracker.reset();
        assert_eq!(tracker.total_unique_count(), 0);
        assert_eq!(tracker.get_bloom_version(), 0);
    }
}