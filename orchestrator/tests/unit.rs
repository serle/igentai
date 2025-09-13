//! Unit tests for individual orchestrator components
//!
//! These tests verify specific functionality of individual methods and components
//! using clean, maintainable test patterns.

// Core imports for testing

mod common;
use common::{TestFixtures, TestHelpers, OrchestratorBuilder};

/// Test orchestrator state management and configuration
#[test]
fn test_orchestrator_state_tracking() {
    // Arrange & Act
    let orchestrator = OrchestratorBuilder::new()
        .with_producer_count(5)
        .with_port(3000)
        .build();

    // Assert - Test all state access methods
    TestHelpers::assert_state(&orchestrator, 5, 3000, 0, None);
}

/// Test uniqueness filtering logic with various scenarios
#[tokio::test]
async fn test_uniqueness_filtering() {
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

    // Assert - Should filter out duplicates correctly
    assert_eq!(unique_count, 4); // Paris, London, Tokyo, Berlin (4 unique)
    assert_eq!(orchestrator.total_count(), 4);
}

/// Test producer statistics tracking across multiple batches
#[tokio::test]
async fn test_producer_statistics() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();

    // Act - Process multiple batches from same producer
    let batch1 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let batch2 = vec!["D".to_string(), "E".to_string(), "A".to_string()]; // A is duplicate

    let count1 = TestHelpers::process_and_count(&mut orchestrator, producer_id.clone(), batch1).await;
    let count2 = TestHelpers::process_and_count(&mut orchestrator, producer_id, batch2).await;

    // Assert - Should track cumulative uniqueness
    assert_eq!(count1, 3); // First batch: all unique
    assert_eq!(count2, 2); // Second batch: 2 new unique (D, E)
    assert_eq!(orchestrator.total_count(), 5); // Total unique across both batches
}

/// Test concurrent message processing from multiple producers
#[tokio::test]
async fn test_concurrent_message_processing() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer1 = TestFixtures::producer_id_1();
    let producer2 = TestFixtures::producer_id_2();

    let batch1 = vec!["A".to_string(), "B".to_string()];
    let batch2 = vec!["C".to_string(), "A".to_string()]; // A is duplicate with batch1

    // Act - Simulate concurrent processing
    let count1 = TestHelpers::process_and_count(&mut orchestrator, producer1, batch1).await;
    let count2 = TestHelpers::process_and_count(&mut orchestrator, producer2, batch2).await;

    // Assert - Should handle cross-producer uniqueness
    assert_eq!(count1, 2); // A, B
    assert_eq!(count2, 1); // Only C is new (A already exists)
    assert_eq!(orchestrator.total_count(), 3); // A, B, C
}

/// Test state reset functionality
#[tokio::test]
async fn test_state_reset() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let batch = vec!["A".to_string(), "B".to_string()];

    // Add some data first
    TestHelpers::process_and_count(&mut orchestrator, producer_id, batch).await;
    assert_eq!(orchestrator.total_count(), 2);

    // Act - Reset for new topic
    let new_topic = "new_test_topic";
    orchestrator.reset_for_topic(new_topic.to_string()).await;

    // Assert - State should be reset
    TestHelpers::assert_state(&orchestrator, TestFixtures::DEFAULT_PRODUCER_COUNT, TestFixtures::DEFAULT_PORT, 0, Some(new_topic));
}

/// Test edge cases and boundary conditions
#[tokio::test]
async fn test_edge_cases() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();

    // Test empty batch
    let empty_count = TestHelpers::process_and_count(
        &mut orchestrator, 
        producer_id.clone(), 
        TestFixtures::empty_attributes()
    ).await;
    assert_eq!(empty_count, 0);

    // Test single item
    let single_count = TestHelpers::process_and_count(
        &mut orchestrator, 
        producer_id.clone(), 
        TestFixtures::single_attribute("solo")
    ).await;
    assert_eq!(single_count, 1);

    // Test all duplicates of existing item
    let all_duplicates = vec!["solo".to_string(), "solo".to_string(), "solo".to_string()];
    let duplicate_count = TestHelpers::process_and_count(&mut orchestrator, producer_id, all_duplicates).await;
    assert_eq!(duplicate_count, 0); // All are duplicates of existing "solo"

    // Final state check
    assert_eq!(orchestrator.total_count(), 1); // Only "solo" should remain
}

/// Test large data handling and performance characteristics
#[tokio::test]
async fn test_large_data_handling() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let large_batch = TestFixtures::large_dataset(10000, 1.0); // 10k unique items

    // Act - Measure performance
    let start_time = std::time::Instant::now();
    let count = TestHelpers::process_and_count(&mut orchestrator, producer_id, large_batch).await;
    let elapsed = start_time.elapsed();

    // Assert - Should handle large datasets efficiently
    assert_eq!(count, 10000);
    assert_eq!(orchestrator.total_count(), 10000);
    
    // Performance check - should process 10k items in reasonable time
    assert!(elapsed.as_millis() < 1000, "Processing took too long: {:?}", elapsed);
}

/// Test string deduplication with edge cases
#[tokio::test]
async fn test_string_deduplication_edge_cases() {
    // Arrange
    let mut orchestrator = TestHelpers::simple_orchestrator();
    let producer_id = TestFixtures::producer_id_1();
    let edge_cases = TestFixtures::edge_case_attributes();

    // Act
    let count = TestHelpers::process_and_count(&mut orchestrator, producer_id, edge_cases).await;

    // Assert - Should treat different string representations as distinct
    assert_eq!(count, 4); // "test", " test ", "TEST", "Test" (one "test" is duplicate)
    assert_eq!(orchestrator.total_count(), 4);
}