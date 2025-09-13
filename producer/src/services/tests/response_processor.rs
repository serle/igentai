//! Tests for ResponseProcessor service

use crate::services::response_processor::RealResponseProcessor;
use crate::traits::ResponseProcessor;

#[tokio::test]
async fn test_response_processor_creation() {
    let processor = RealResponseProcessor::new();
    
    // Should be able to process empty response
    let result = processor.process_response("").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_extract_quoted_attributes() {
    let processor = RealResponseProcessor::new();
    
    let response = r#"Here are some attributes: "creative", "innovative", "bold", "unique""#;
    let attributes = processor.extract_attributes(response).await.unwrap();
    
    assert!(!attributes.is_empty());
    assert!(attributes.contains(&"creative".to_string()));
    assert!(attributes.contains(&"innovative".to_string()));
    assert!(attributes.contains(&"bold".to_string()));
    assert!(attributes.contains(&"unique".to_string()));
}

#[tokio::test]
async fn test_extract_comma_separated_attributes() {
    let processor = RealResponseProcessor::new();
    
    let response = "innovative, creative, bold, sustainable, modern";
    let attributes = processor.extract_attributes(response).await.unwrap();
    
    assert!(!attributes.is_empty());
    assert!(attributes.len() <= 5); // May be limited by truncation
}

#[tokio::test]
async fn test_filter_short_attributes() {
    let processor = RealResponseProcessor::new();
    
    let response = r#""a", "ab", "abc", "good attribute""#;
    let attributes = processor.extract_attributes(response).await.unwrap();
    
    // Should only contain attributes longer than 2 characters
    for attr in &attributes {
        assert!(attr.len() > 2);
    }
    assert!(attributes.contains(&"abc".to_string()));
    assert!(attributes.contains(&"good attribute".to_string()));
}

#[tokio::test]
async fn test_filter_assistant_mentions() {
    let processor = RealResponseProcessor::new();
    
    let response = "creative, assistant, innovative, assistant response, bold";
    let attributes = processor.extract_attributes(response).await.unwrap();
    
    // Should not contain "assistant" or "assistant response"
    for attr in &attributes {
        assert!(!attr.to_lowercase().contains("assistant"));
    }
}

#[tokio::test]
async fn test_deduplication() {
    let processor = RealResponseProcessor::new();
    
    let response = r#""creative", "bold", "creative", "innovative", "bold""#;
    let attributes = processor.extract_attributes(response).await.unwrap();
    
    // Should be deduplicated
    let creative_count = attributes.iter().filter(|&attr| attr == "creative").count();
    let bold_count = attributes.iter().filter(|&attr| attr == "bold").count();
    
    assert_eq!(creative_count, 1);
    assert_eq!(bold_count, 1);
}

#[tokio::test]
async fn test_attribute_limit() {
    let processor = RealResponseProcessor::new();
    
    // Create response with more than 10 attributes
    let mut long_response = String::new();
    for i in 0..15 {
        if i > 0 {
            long_response.push_str(", ");
        }
        long_response.push_str(&format!(r#""attribute{}""#, i));
    }
    
    let attributes = processor.extract_attributes(&long_response).await.unwrap();
    
    // Should be limited to 10 attributes
    assert!(attributes.len() <= 10);
}

#[tokio::test]
async fn test_bloom_filter_empty() {
    let processor = RealResponseProcessor::new();
    
    let test_attributes = vec!["creative".to_string(), "innovative".to_string()];
    let filtered = processor.filter_duplicates(&test_attributes).await.unwrap();
    
    // With no bloom filter set, should return all attributes
    assert_eq!(filtered.len(), test_attributes.len());
    assert_eq!(filtered, test_attributes);
}

#[tokio::test]
async fn test_set_bloom_filter() {
    let processor = RealResponseProcessor::new();
    
    // Set a bloom filter with some data
    let filter_data = vec![1, 2, 3, 4, 5, 6, 7, 8];
    let result = processor.set_bloom_filter(filter_data.clone()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_bloom_filter_with_data() {
    let processor = RealResponseProcessor::new();
    
    // Set a bloom filter
    let filter_data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    processor.set_bloom_filter(filter_data).await.unwrap();
    
    let test_attributes = vec!["creative".to_string(), "innovative".to_string(), "bold".to_string()];
    let filtered = processor.filter_duplicates(&test_attributes).await.unwrap();
    
    // With bloom filter, some attributes might be filtered out
    assert!(filtered.len() <= test_attributes.len());
}

#[tokio::test]
async fn test_complete_response_processing() {
    let processor = RealResponseProcessor::new();
    
    let response = r#"
    Here are creative attributes for your topic:
    "innovative", "sustainable", "bold", "creative", "modern", "elegant"
    These represent unique qualities that stand out.
    "#;
    
    let result = processor.process_response(response).await.unwrap();
    
    assert!(!result.is_empty());
    // All results should be valid (non-empty, longer than 2 chars)
    for attr in &result {
        assert!(attr.len() > 2);
        assert!(!attr.trim().is_empty());
    }
}

#[tokio::test]
async fn test_get_filter_stats() {
    let processor = RealResponseProcessor::new();
    
    let stats = processor.get_last_filter_stats(10, 7).await;
    
    assert_eq!(stats.total_candidates, 10);
    assert_eq!(stats.filtered_candidates, 7);
    assert_eq!(stats.filter_effectiveness, 0.3); // (10-7)/10 = 0.3
    assert!(stats.last_filter_update > 0);
}

#[tokio::test]
async fn test_complex_response_formats() {
    let processor = RealResponseProcessor::new();
    
    // Test different response formats
    let test_cases = vec![
        r#"1. "creative" 2. "innovative" 3. "bold""#,
        "- creative\n- innovative\n- bold",
        "creative | innovative | bold",
        "Attributes: creative, innovative, bold.",
    ];
    
    for response in test_cases {
        let result = processor.process_response(response).await.unwrap();
        assert!(!result.is_empty(), "Failed to extract from: {}", response);
    }
}