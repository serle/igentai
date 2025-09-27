//! Response processing and attribute extraction

use crate::error::ProducerResult;
use crate::types::ApiResponse;
use growable_bloom_filter::GrowableBloom;
use serde_json;
use shared::ProviderId;
use tracing::{debug, info};

/// Response processor for simple attribute extraction and deduplication
pub struct Processor {
    /// Bloom filter synced from orchestrator
    bloom_filter: GrowableBloom,

    /// Known seen values from orchestrator (authoritative source)
    seen_values: Vec<String>,

    /// Count of duplicate values encountered locally
    duplicate_count: usize,
}

impl Processor {
    /// Create new processor with default configuration
    pub fn new() -> Self {
        // Initialize empty bloom filter with reasonable defaults
        let bloom_filter = GrowableBloom::new(0.01, 10000);

        Self {
            bloom_filter,
            seen_values: Vec::new(),
            duplicate_count: 0,
        }
    }

    /// Create processor with custom configuration (for testing)
    pub fn with_config(false_positive_rate: f32, max_items: usize) -> Self {
        // Initialize empty bloom filter with reasonable defaults
        let bloom_filter = GrowableBloom::new(false_positive_rate as f64, max_items);

        Self {
            bloom_filter,
            seen_values: Vec::new(),
            duplicate_count: 0,
        }
    }

    /// Process API response and extract values, returning statistics
    pub fn process_response(&mut self, response: ApiResponse) -> ProducerResult<ProcessingStats> {
        if !response.success {
            debug!("Skipping processing for failed response from {:?}", response.provider);
            return Ok(ProcessingStats::empty());
        }

        // Split on newlines and commas, but preserve spaces within attribute names
        let extracted_items: Vec<String> = response.content
            .split(|c: char| c == '\n' || c == '\r' || c == ',')
            .map(|item| {
                // Clean up each item: trim, lowercase, preserve spaces and letters only (exclude numbers)
                let cleaned = item.trim()
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphabetic() || c.is_whitespace()) // Only letters and spaces, no numbers
                    .collect::<String>();
                
                // Normalize multiple spaces to single spaces and trim
                let normalized = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
                
                // Remove any leading/trailing numbers or number patterns that might remain
                normalized
                    .split_whitespace()
                    .filter(|word| !word.chars().all(|c| c.is_numeric())) // Remove pure number words
                    .filter(|word| !word.starts_with(char::is_numeric)) // Remove words starting with numbers
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|item| {
                !item.is_empty() 
                && item.len() > 2 
                && !item.chars().any(|c| c.is_numeric()) // Extra check: no numbers anywhere
                && item.split_whitespace().count() <= 6 // Reasonable length limit for noun phrases
            })
            .collect();

        // Process extracted items and check uniqueness (functional approach)
        let (new_values, duplicate_count) = self.filter_new_values(&extracted_items);

        // Update processor state with new values
        for value in &new_values {
            self.add_value(value);
        }

        let stats = ProcessingStats {
            total_extracted: extracted_items.len(),
            duplicate_count,
            provider: response.provider,
            new_values,
        };

        debug!(
            "Processed {:?} response: {} total, {} new, {} duplicates",
            response.provider,
            stats.total_extracted,
            stats.new_values.len(),
            stats.duplicate_count
        );

        Ok(stats)
    }

    /// Filter extracted values into new vs duplicate (functional approach)
    fn filter_new_values(&mut self, values: &[String]) -> (Vec<String>, usize) {
        let mut new_values = Vec::new();
        let mut duplicate_count = 0;

        for value in values {
            if self.bloom_filter.contains(value) {
                // Bloom filter says it might be a duplicate
                duplicate_count += 1;
            } else {
                // Definitely not seen before
                new_values.push(value.clone());
            }
        }

        // Update duplicate counter for statistics
        self.duplicate_count += duplicate_count;

        (new_values, duplicate_count)
    }

    /// Add a value to processor state (both bloom filter and seen values)
    fn add_value(&mut self, value: &str) {
        self.bloom_filter.insert(value.to_string());
        self.seen_values.push(value.to_string());
    }

    /// Get processor statistics
    pub fn get_stats(&self) -> ProcessorStats {
        ProcessorStats {
            total_processed: self.seen_values.len() + self.duplicate_count,
            total_unique_attributes: self.seen_values.len(),
            duplicate_count: self.duplicate_count,
            bloom_filter_enabled: true,             // Always enabled now
            bloom_filter_false_positive_rate: 0.01, // Default rate
        }
    }

    /// Reset processor state
    pub fn reset(&mut self) {
        // Create fresh empty bloom filter
        self.bloom_filter = GrowableBloom::new(0.01, 10000);
        self.seen_values.clear();
        self.duplicate_count = 0;
        info!("Processor state reset");
    }

    /// Update bloom filter with orchestrator data (receives serialized bloom filter)
    pub fn update_bloom_filter(&mut self, bloom_data: Option<Vec<u8>>, values: Vec<String>) {
        // Replace our seen values with orchestrator's authoritative list
        self.seen_values = values;

        // Use orchestrator's bloom filter if provided, otherwise fallback to rebuilding
        if let Some(data) = bloom_data {
            match Self::deserialize_bloom_filter(&data) {
                Ok(bloom_filter) => {
                    self.bloom_filter = bloom_filter;
                    info!("📡 Using orchestrator's serialized bloom filter ({} bytes)", data.len());
                }
                Err(e) => {
                    info!("⚠️ Failed to deserialize bloom filter, rebuilding: {}", e);
                    self.bloom_filter = Self::build_bloom_filter(&self.seen_values);
                }
            }
        } else {
            // No bloom filter provided - must rebuild (inefficient fallback)
            info!(
                "📋 No bloom filter provided, rebuilding from {} values",
                self.seen_values.len()
            );
            self.bloom_filter = Self::build_bloom_filter(&self.seen_values);
        }
    }

    /// Deserialize bloom filter from orchestrator
    fn deserialize_bloom_filter(data: &[u8]) -> Result<GrowableBloom, String> {
        serde_json::from_slice::<GrowableBloom>(data).map_err(|e| format!("Failed to deserialize bloom filter: {e}"))
    }

    /// Static/functional method to build a bloom filter from values (fallback only)
    fn build_bloom_filter(values: &[String]) -> GrowableBloom {
        let capacity = std::cmp::max(10000, values.len() * 2);
        let mut bloom_filter = GrowableBloom::new(0.01, capacity);

        // Add all values to the bloom filter
        for value in values {
            bloom_filter.insert(value);
        }

        bloom_filter
    }
}

/// Processor statistics
#[derive(Debug, Clone)]
pub struct ProcessorStats {
    pub total_processed: usize,
    pub total_unique_attributes: usize,
    pub duplicate_count: usize,
    pub bloom_filter_enabled: bool,
    pub bloom_filter_false_positive_rate: f64,
}

/// Statistics from processing a single API response
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub total_extracted: usize,
    pub duplicate_count: usize,
    pub provider: ProviderId,
    pub new_values: Vec<String>,
}

impl ProcessingStats {
    pub fn empty() -> Self {
        Self {
            total_extracted: 0,
            duplicate_count: 0,
            provider: ProviderId::Random, // Default provider
            new_values: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.total_extracted == 0
    }

    pub fn has_new_values(&self) -> bool {
        !self.new_values.is_empty()
    }
}

impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::{ProviderId, TokenUsage};
    use uuid::Uuid;

    fn create_test_response(provider: ProviderId, content: String) -> ApiResponse {
        ApiResponse {
            provider,
            request_id: Uuid::new_v4(),
            content,
            tokens_used: TokenUsage { input_tokens: 50, output_tokens: 50 },
            response_time_ms: 500,
            timestamp: Utc::now(),
            success: true,
            error_message: None,
        }
    }

    #[test]
    fn test_simple_word_extraction() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "red, blue, green, yellow, orange, purple".to_string());

        let stats = processor.process_response(response).unwrap();
        assert_eq!(stats.total_extracted, 6);
        assert_eq!(stats.new_values.len(), 6);
        assert_eq!(stats.duplicate_count, 0);

        // Check that new values are in processing stats
        assert_eq!(stats.new_values.len(), 6);
        assert!(stats.new_values.contains(&"red".to_string()));
        assert!(stats.new_values.contains(&"blue".to_string()));
        assert!(stats.new_values.contains(&"green".to_string()));
    }

    #[test]
    fn test_newline_separated_list() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "apple\nbanana\ncherry\ndate".to_string());

        let stats = processor.process_response(response).unwrap();
        assert_eq!(stats.total_extracted, 4);
        assert_eq!(stats.new_values.len(), 4);

        assert!(stats.new_values.contains(&"apple".to_string()));
        assert!(stats.new_values.contains(&"banana".to_string()));
    }

    #[test]
    fn test_comma_separated_list() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "dog, cat, bird, fish".to_string());

        let stats = processor.process_response(response).unwrap();
        assert_eq!(stats.total_extracted, 4);
        assert_eq!(stats.new_values.len(), 4);

        assert!(stats.new_values.contains(&"dog".to_string()));
        assert!(stats.new_values.contains(&"cat".to_string()));
    }

    #[test]
    fn test_uniqueness_tracking() {
        let mut processor = Processor::new();

        // First occurrence should be unique
        let response1 = create_test_response(ProviderId::OpenAI, "unique, word".to_string());
        let stats1 = processor.process_response(response1).unwrap();
        assert_eq!(stats1.total_extracted, 2);
        assert_eq!(stats1.new_values.len(), 2);
        assert_eq!(stats1.duplicate_count, 0);

        // Second occurrence should be duplicates
        let response2 = create_test_response(ProviderId::OpenAI, "unique, word".to_string());
        let stats2 = processor.process_response(response2).unwrap();
        assert_eq!(stats2.total_extracted, 2);
        assert_eq!(stats2.new_values.len(), 0);
        assert_eq!(stats2.duplicate_count, 2);
    }

    #[test]
    fn test_bloom_filter_update() {
        let mut processor = Processor::new();

        processor.update_bloom_filter(None, vec!["test".to_string(), "example".to_string()]);

        let response = create_test_response(ProviderId::OpenAI, "test, new, example, fresh".to_string());
        let stats = processor.process_response(response).unwrap();

        // "test" and "example" should be duplicates (already in bloom filter)
        // "new" and "fresh" should be unique
        assert_eq!(stats.total_extracted, 4);
        assert_eq!(stats.new_values.len(), 2);
        assert_eq!(stats.duplicate_count, 2);
    }

    #[test]
    fn test_processor_stats() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "one, two, three".to_string());

        let processing_stats = processor.process_response(response).unwrap();
        assert_eq!(processing_stats.new_values.len(), 3);

        let stats = processor.get_stats();
        assert_eq!(stats.total_processed, 3);
    }

    #[test]
    fn test_processor_reset() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "test, data".to_string());

        processor.process_response(response).unwrap();
        assert_eq!(processor.get_stats().total_processed, 2);

        processor.reset();
        assert_eq!(processor.get_stats().total_processed, 0);
    }

    #[test]
    fn test_failed_response_handling() {
        let mut processor = Processor::new();
        let mut response = create_test_response(ProviderId::OpenAI, "should be ignored".to_string());
        response.success = false;

        let stats = processor.process_response(response).unwrap();
        assert_eq!(stats.total_extracted, 0);
        assert_eq!(stats.new_values.len(), 0);
    }

    #[test]
    fn test_new_values_tracking() {
        let mut processor = Processor::new();
        let response = create_test_response(ProviderId::OpenAI, "test".to_string());

        let stats = processor.process_response(response).unwrap();
        assert_eq!(stats.total_extracted, 1);
        assert_eq!(stats.new_values.len(), 1);

        assert_eq!(stats.new_values.len(), 1);
        assert_eq!(stats.new_values[0], "test");
    }
}
