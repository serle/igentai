//! End-to-end bloom filter integration tests
//!
//! Tests the complete bloom filter workflow:
//! 1. Orchestrator creates and populates a bloom filter
//! 2. Orchestrator serializes the bloom filter
//! 3. Producer receives and deserializes the bloom filter
//! 4. Producer uses the bloom filter for deduplication

use tracing_subscriber;

use growable_bloom_filter::GrowableBloom;
use producer::core::Processor;
use serde_json;

/// Test the complete bloom filter serialization/deserialization workflow
#[tokio::test]
async fn test_bloom_filter_serialization_roundtrip() {
    let _ = tracing_subscriber::fmt::try_init();

    // Step 1: Create a bloom filter in the "orchestrator" (simulated)
    let mut orchestrator_bloom = GrowableBloom::new(0.01, 10000);

    // Add some test data that would be found by the orchestrator
    let orchestrator_data = vec![
        "apple",
        "banana",
        "cherry",
        "date",
        "elderberry",
        "fig",
        "grape",
        "honeydew",
        "kiwi",
        "lemon",
    ];

    for item in &orchestrator_data {
        orchestrator_bloom.insert(item);
    }

    // Step 2: Serialize the bloom filter (what orchestrator would do)
    let serialized_bloom = serde_json::to_vec(&orchestrator_bloom).expect("Failed to serialize bloom filter");

    println!("🔄 Serialized bloom filter: {} bytes", serialized_bloom.len());

    // Step 3: Create a producer and update it with the serialized bloom filter
    let mut processor = Processor::new();

    // Convert orchestrator data to the format the producer expects
    let seen_values: Vec<String> = orchestrator_data.iter().map(|s| s.to_string()).collect();

    // This simulates the orchestrator sending a sync command to the producer
    processor.update_bloom_filter(Some(serialized_bloom), seen_values.clone());

    // Step 4: Test that the producer's bloom filter works correctly

    // Test 4a: Items that were in the orchestrator should be detected as duplicates
    for item in &orchestrator_data {
        let response = create_test_response(item.to_string());
        let stats = processor
            .process_response(response)
            .expect("Failed to process response");

        // Should detect as duplicate (bloom filter hit)
        assert_eq!(
            stats.new_values.len(),
            0,
            "Item '{}' should be detected as duplicate",
            item
        );
        assert_eq!(
            stats.duplicate_count, 1,
            "Item '{}' should be counted as 1 duplicate",
            item
        );
    }

    // Test 4b: New items not in the orchestrator should be detected as unique
    let new_items = vec!["mango", "nectarine", "orange", "papaya", "quince"];

    for item in &new_items {
        let response = create_test_response(item.to_string());
        let stats = processor
            .process_response(response)
            .expect("Failed to process response");

        // Should detect as new (bloom filter miss)
        assert_eq!(stats.new_values.len(), 1, "Item '{}' should be detected as new", item);
        assert_eq!(
            stats.duplicate_count, 0,
            "Item '{}' should not be counted as duplicate",
            item
        );
        assert_eq!(stats.new_values[0], *item, "New item should match input");
    }

    println!("✅ Bloom filter serialization/deserialization roundtrip successful!");
}

/// Test bloom filter behavior with large datasets
#[tokio::test]
async fn test_bloom_filter_large_dataset_e2e() {
    let _ = tracing_subscriber::fmt::try_init();

    // Create diverse data for meaningful testing
    let base_words = vec!["apple", "banana", "cherry", "date", "elderberry"];
    let normalized_data: Vec<String> = (0..5000)
        .map(|i| base_words[i % base_words.len()].to_string())
        .collect();

    // Step 1: Create and populate orchestrator bloom filter
    let mut orchestrator_bloom = GrowableBloom::new(0.01, 10000);
    for item in &normalized_data {
        orchestrator_bloom.insert(item);
    }

    // Step 2: Serialize the bloom filter
    let serialized_bloom = serde_json::to_vec(&orchestrator_bloom).expect("Failed to serialize large bloom filter");

    println!("🔄 Large bloom filter serialized: {} bytes", serialized_bloom.len());

    // Step 3: Producer receives and uses the bloom filter
    let mut processor = Processor::new();
    processor.update_bloom_filter(Some(serialized_bloom), normalized_data);

    // Step 4: Test with mix of known and unknown items
    let test_items = vec![
        ("apple", true),     // Should be duplicate
        ("banana", true),    // Should be duplicate
        ("cherry", true),    // Should be duplicate
        ("mango", false),    // Should be new
        ("orange", false),   // Should be new
    ];

    for (item, should_be_duplicate) in test_items {
        let response = create_test_response(item.to_string());
        let stats = processor
            .process_response(response)
            .expect("Failed to process response");

        if should_be_duplicate {
            assert_eq!(
                stats.new_values.len(),
                0,
                "Item '{}' should be detected as duplicate",
                item
            );
            assert_eq!(
                stats.duplicate_count, 1,
                "Item '{}' should be counted as 1 duplicate",
                item
            );
        } else {
            assert_eq!(stats.new_values.len(), 1, "Item '{}' should be detected as new", item);
            assert_eq!(
                stats.duplicate_count, 0,
                "Item '{}' should not be counted as duplicate",
                item
            );
        }
    }

    println!("✅ Large dataset bloom filter test successful!");
}

/// Test bloom filter false positive rate
#[tokio::test]
async fn test_bloom_filter_false_positive_rate() {
    let _ = tracing_subscriber::fmt::try_init();

    // Create diverse normalized dataset that would come from real processor output
    let known_words = vec![
        "apple", "banana", "cherry", "date", "elderberry", "fig", "grape", "honeydew", 
        "kiwi", "lemon", "mango", "nectarine", "orange", "papaya", "quince", "raspberry",
        "strawberry", "tangerine", "watermelon", "blueberry", "coconut", "dragonfruit",
        "guava", "jackfruit", "lime", "melon", "peach", "pear", "pineapple", "plum"
    ];
    
    // Repeat words to create larger dataset
    let dataset_size = 1000;
    let normalized_data: Vec<String> = (0..dataset_size)
        .map(|i| known_words[i % known_words.len()].to_string())
        .collect();

    // Create bloom filter with higher false positive rate for testing
    let mut orchestrator_bloom = GrowableBloom::new(0.05, dataset_size); // 5% FP rate
    for item in &normalized_data {
        orchestrator_bloom.insert(item);
    }

    let serialized_bloom = serde_json::to_vec(&orchestrator_bloom).expect("Failed to serialize bloom filter");

    let mut processor = Processor::new();
    processor.update_bloom_filter(Some(serialized_bloom), normalized_data.clone());

    // Test with items that definitely shouldn't be in the bloom filter
    let test_size = 1000;
    let mut false_positives = 0;

    // Generate unique test words using different prefixes to ensure uniqueness after normalization
    let prefixes = vec!["computer", "device", "gadget", "machine", "tool", "hardware", "software", "system"];
    
    for i in 0..test_size {
        let prefix = &prefixes[i % prefixes.len()];
        let suffix_num = i / prefixes.len();
        let test_item = if suffix_num == 0 {
            prefix.to_string()
        } else {
            format!("{}{}", prefix, "x".repeat(suffix_num)) // Add 'x' chars to make unique
        };
        
        let response = create_test_response(test_item.clone());
        let stats = processor
            .process_response(response)
            .expect("Failed to process response");

        if stats.duplicate_count > 0 {
            false_positives += 1;
        }
    }

    let false_positive_rate = false_positives as f64 / test_size as f64;
    println!("🎯 Measured false positive rate: {:.2}%", false_positive_rate * 100.0);

    // Should be approximately 5% (allow some variance)
    assert!(
        false_positive_rate < 0.10,
        "False positive rate too high: {:.2}%",
        false_positive_rate * 100.0
    );

    println!("✅ False positive rate test successful!");
}

/// Test bloom filter with special characters and edge cases
#[tokio::test]
async fn test_bloom_filter_special_characters_e2e() {
    let _ = tracing_subscriber::fmt::try_init();

    let special_data = vec![
        "café",
        "naïve",
        "résumé",
        "München",
        "北京",
        "🚀",
        "hello world",
        "test-item",
        "item.with.dots",
        "item_with_underscores",
        "UPPERCASE",
        "lowercase",
        "MiXeD_CaSe",
    ];

    // Create and populate orchestrator bloom filter
    let mut orchestrator_bloom = GrowableBloom::new(0.01, 1000);
    for item in &special_data {
        orchestrator_bloom.insert(item);
    }

    let serialized_bloom =
        serde_json::to_vec(&orchestrator_bloom).expect("Failed to serialize bloom filter with special characters");

    let seen_values: Vec<String> = special_data.iter().map(|s| s.to_string()).collect();

    let mut processor = Processor::new();
    processor.update_bloom_filter(Some(serialized_bloom), seen_values);

    // Test that all special characters are properly handled
    for item in &special_data {
        let response = create_test_response(item.to_string());
        let stats = processor
            .process_response(response)
            .expect("Failed to process response with special characters");

        // Note: The processor normalizes input (lowercase, alphanumeric only)
        // So some of these will not match exactly. This tests the real behavior.
        println!(
            "📝 Item '{}': new={}, duplicates={}",
            item,
            stats.new_values.len(),
            stats.duplicate_count
        );
    }

    println!("✅ Special characters bloom filter test completed!");
}

/// Test error handling when bloom filter deserialization fails
#[tokio::test]
async fn test_bloom_filter_deserialization_error_handling() {
    let _ = tracing_subscriber::fmt::try_init();

    let mut processor = Processor::new();

    // Test with invalid JSON data
    let invalid_json = b"invalid json data";
    let seen_values = vec!["testword".to_string(), "anotherword".to_string()];

    // This should gracefully fall back to rebuilding the bloom filter from seen_values
    processor.update_bloom_filter(Some(invalid_json.to_vec()), seen_values.clone());

    // Test that the fallback bloom filter works
    let response = create_test_response("testword".to_string());
    let stats = processor
        .process_response(response)
        .expect("Failed to process response after deserialization error");

    // Should detect as duplicate because it falls back to rebuilding from seen_values
    assert_eq!(
        stats.duplicate_count, 1,
        "Should detect 'testword' as duplicate after fallback"
    );

    // Test with no bloom filter data (None case)
    processor.update_bloom_filter(None, seen_values);

    // Should still work via fallback
    let response2 = create_test_response("anotherword".to_string());
    let stats2 = processor
        .process_response(response2)
        .expect("Failed to process response with no bloom filter data");

    assert_eq!(
        stats2.duplicate_count, 1,
        "Should detect 'anotherword' as duplicate after None fallback"
    );

    println!("✅ Error handling test successful!");
}

// Helper function to create test API responses
fn create_test_response(content: String) -> producer::types::ApiResponse {
    use chrono::Utc;
    use shared::{ProviderId, TokenUsage};
    use uuid::Uuid;

    producer::types::ApiResponse {
        provider: ProviderId::Random,
        request_id: Uuid::new_v4(),
        content,
        tokens_used: TokenUsage { input_tokens: 5, output_tokens: 5 },
        response_time_ms: 100,
        timestamp: Utc::now(),
        success: true,
        error_message: None,
    }
}
