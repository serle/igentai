//! Comprehensive integration tests for the orchestrator
//!
//! These tests verify end-to-end functionality using mockall-generated mocks
//! with clean, maintainable test patterns.

// Core imports for testing
use orchestrator::OrchestratorError;

mod common;
use common::{TestFixtures, TestHelpers, OrchestratorBuilder};

/// Test basic orchestrator instantiation and configuration
#[tokio::test]
async fn test_orchestrator_instantiation() {
    // Arrange & Act
    let orchestrator = OrchestratorBuilder::new()
        .with_producer_count(3)
        .with_port(9000)
        .build();

    // Assert
    TestHelpers::assert_state(&orchestrator, 3, 9000, 0, None);
}

/// Test API key validation success
#[tokio::test]
async fn test_api_key_validation_success() {
    // Arrange
    let mut orchestrator = TestHelpers::api_validation_orchestrator();

    // Act
    let result = orchestrator.next().await;

    // Assert
    assert!(result.is_ok());
}

/// Test API key validation failure
#[tokio::test]
async fn test_api_key_validation_failure() {
    // Arrange
    let mut orchestrator = TestHelpers::failing_api_orchestrator();

    // Act - Test API validation directly with timeout to prevent infinite loop
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        orchestrator.run()
    ).await;

    // Assert - Should fail due to API key validation error
    match result {
        Ok(run_result) => {
            assert!(run_result.is_err(), "Expected API validation to fail but run() succeeded");
            if let Err(OrchestratorError::ConfigurationError { field }) = run_result {
                assert!(field.contains("Missing required API keys: OPENAI_API_KEY"));
            }
        }
        Err(_timeout) => panic!("Test should not timeout - API validation should fail immediately")
    }
}

/// Test uniqueness processing logic
#[tokio::test]
async fn test_uniqueness_processing() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let duplicate_attributes = TestFixtures::duplicate_attributes();

    // Act
    let unique_count = TestHelpers::process_and_count(
        &mut orchestrator, 
        producer_id, 
        duplicate_attributes
    ).await;

    // Assert
    assert_eq!(unique_count, 4); // Paris, London, Tokyo, Berlin (4 unique from 6 total)
    assert_eq!(orchestrator.total_count(), 4);
}

/// Test multiple producer processing
#[tokio::test]
async fn test_multiple_producer_processing() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer1 = TestFixtures::producer_id_1();
    let producer2 = TestFixtures::producer_id_2();

    let batch1 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let batch2 = vec!["C".to_string(), "D".to_string(), "E".to_string()]; // C overlaps

    // Act
    let count1 = TestHelpers::process_and_count(&mut orchestrator, producer1, batch1).await;
    let count2 = TestHelpers::process_and_count(&mut orchestrator, producer2, batch2).await;

    // Assert
    assert_eq!(count1, 3); // A, B, C
    assert_eq!(count2, 2); // D, E (C already exists)
    assert_eq!(orchestrator.total_count(), 5); // A, B, C, D, E total
}

/// Test topic reset functionality
#[tokio::test]
async fn test_topic_reset() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let attributes = vec!["A".to_string(), "B".to_string()];

    // Add some data
    TestHelpers::process_and_count(&mut orchestrator, producer_id, attributes).await;
    assert_eq!(orchestrator.total_count(), 2);

    // Act
    orchestrator.reset_for_topic(TestFixtures::TEST_TOPIC.to_string()).await;

    // Assert
    assert_eq!(orchestrator.total_count(), 0);
    assert_eq!(orchestrator.current_topic(), Some(TestFixtures::TEST_TOPIC));
}

/// Test producer statistics tracking
#[tokio::test]
async fn test_producer_statistics() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();

    // Act - Process multiple batches from same producer
    let batch1 = vec!["Item1".to_string(), "Item2".to_string()];
    let batch2 = vec!["Item3".to_string(), "Item1".to_string()]; // Item1 is duplicate
    let batch3 = vec!["Item4".to_string()];

    let count1 = TestHelpers::process_and_count(&mut orchestrator, producer_id.clone(), batch1).await;
    let count2 = TestHelpers::process_and_count(&mut orchestrator, producer_id.clone(), batch2).await;
    let count3 = TestHelpers::process_and_count(&mut orchestrator, producer_id, batch3).await;

    // Assert
    assert_eq!(count1, 2); // Item1, Item2
    assert_eq!(count2, 1); // Only Item3 is new
    assert_eq!(count3, 1); // Item4
    assert_eq!(orchestrator.total_count(), 4); // Item1, Item2, Item3, Item4
}

/// Test empty batch handling
#[tokio::test]
async fn test_empty_batch_handling() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();

    // Act
    let count = TestHelpers::process_and_count(
        &mut orchestrator, 
        producer_id, 
        TestFixtures::empty_attributes()
    ).await;

    // Assert
    assert_eq!(count, 0);
    assert_eq!(orchestrator.total_count(), 0);
}

/// Test large batch processing
#[tokio::test]
async fn test_large_batch_processing() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let large_batch = TestFixtures::large_dataset(1000, 0.8); // 800 unique items

    // Act
    let count = TestHelpers::process_and_count(&mut orchestrator, producer_id, large_batch).await;

    // Assert
    assert_eq!(count, 800); // Should detect 800 unique items
    assert_eq!(orchestrator.total_count(), 800);
}

/// Test edge case string handling
#[tokio::test]
async fn test_edge_case_string_handling() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let edge_cases = TestFixtures::edge_case_attributes();

    // Act
    let count = TestHelpers::process_and_count(&mut orchestrator, producer_id, edge_cases).await;

    // Assert - All variations should be treated as different (exact string matching)
    assert_eq!(count, 4); // "test", " test ", "TEST", "Test" (one "test" is duplicate)
    assert_eq!(orchestrator.total_count(), 4);
}