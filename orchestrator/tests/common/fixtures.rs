//! Test fixtures and data for orchestrator tests
//!
//! This module provides consistent test data and fixtures used across all test suites.

use shared::ProducerId;

/// Standard test data and fixtures
pub struct TestFixtures;

impl TestFixtures {
    /// Standard test producer IDs using proper UUID format
    pub const PRODUCER_1: &'static str = "550e8400-e29b-41d4-a716-446655440001";
    pub const PRODUCER_2: &'static str = "550e8400-e29b-41d4-a716-446655440002";
    pub const PRODUCER_3: &'static str = "550e8400-e29b-41d4-a716-446655440003";
    
    /// Standard configuration values
    pub const DEFAULT_PRODUCER_COUNT: u32 = 2;
    pub const DEFAULT_PORT: u16 = 8080;
    pub const TEST_TOPIC: &'static str = "test_topic";
    
    /// Test API keys
    pub const OPENAI_KEY: &'static str = "test-openai-key";
    pub const ANTHROPIC_KEY: &'static str = "test-anthropic-key";
    
    /// Create a valid ProducerId from standard test ID
    pub fn producer_id_1() -> ProducerId {
        ProducerId::from_string(Self::PRODUCER_1).unwrap()
    }
    
    pub fn producer_id_2() -> ProducerId {
        ProducerId::from_string(Self::PRODUCER_2).unwrap()
    }
    
    pub fn producer_id_3() -> ProducerId {
        ProducerId::from_string(Self::PRODUCER_3).unwrap()
    }
    
    /// Sample attributes for uniqueness testing
    pub fn sample_attributes() -> Vec<String> {
        vec![
            "Eiffel Tower".to_string(),
            "Louvre Museum".to_string(),
            "Notre-Dame Cathedral".to_string(),
            "Arc de Triomphe".to_string(),
            "Champs-Élysées".to_string(),
        ]
    }
    
    /// Attributes with duplicates for testing uniqueness filtering
    pub fn duplicate_attributes() -> Vec<String> {
        vec![
            "Paris".to_string(),
            "London".to_string(),
            "Paris".to_string(), // Duplicate
            "Tokyo".to_string(),
            "London".to_string(), // Duplicate
            "Berlin".to_string(),
        ]
    }
    
    /// Large dataset for performance testing
    pub fn large_dataset(size: usize, unique_ratio: f64) -> Vec<String> {
        let unique_count = (size as f64 * unique_ratio) as usize;
        let mut data = Vec::with_capacity(size);
        
        for i in 0..size {
            let item_id = i % unique_count;
            data.push(format!("Item_{}", item_id));
        }
        
        data
    }
    
    /// Empty dataset for edge case testing
    pub fn empty_attributes() -> Vec<String> {
        Vec::new()
    }
    
    /// Single item for edge case testing
    pub fn single_attribute(item: &str) -> Vec<String> {
        vec![item.to_string()]
    }
    
    /// Dataset with whitespace and case variations
    pub fn edge_case_attributes() -> Vec<String> {
        vec![
            "test".to_string(),
            " test ".to_string(),    // With spaces
            "TEST".to_string(),      // Different case
            "test".to_string(),      // Exact duplicate
            "Test".to_string(),      // Different case
        ]
    }
}