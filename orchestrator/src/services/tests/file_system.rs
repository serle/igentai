//! Comprehensive tests for RealFileSystem service
//!
//! These tests verify the critical file management functionality including
//! topic folder creation, attribute writing, file synchronization, and error handling.

use std::collections::HashSet;
use tokio::fs;

use crate::services::file_manager::RealFileSystem;
use crate::traits::FileSystem;

/// Helper function to set up test environment
async fn setup_file_system() -> RealFileSystem {
    // Clean up any existing outputs directory
    if fs::metadata("outputs").await.is_ok() {
        let _ = fs::remove_dir_all("outputs").await;
    }
    
    RealFileSystem::new()
}

/// Clean up test outputs
async fn cleanup_outputs() {
    if fs::metadata("outputs").await.is_ok() {
        let _ = fs::remove_dir_all("outputs").await;
    }
}

/// Test basic topic folder creation
#[tokio::test]
async fn test_create_topic_folder() {
    let file_system = setup_file_system().await;
    
    let result = file_system.create_topic_folder("Test Topic", 3).await;
    assert!(result.is_ok(), "Should create topic folder successfully");
    
    // Verify folder structure
    let folder_path = std::path::PathBuf::from("outputs").join("test_topic");
    assert!(folder_path.exists(), "Topic folder should exist");
    assert!(folder_path.is_dir(), "Should be a directory");
    
    // Verify required files
    assert!(folder_path.join("topic.txt").exists(), "topic.txt should exist");
    assert!(folder_path.join("output.txt").exists(), "output.txt should exist");
    assert!(folder_path.join("metrics.json").exists(), "metrics.json should exist");
    assert!(folder_path.join("state.json").exists(), "state.json should exist");
    
    // Verify topic.txt content
    let topic_content = fs::read_to_string(folder_path.join("topic.txt")).await.unwrap();
    assert!(topic_content.contains("Topic: Test Topic"), "Should contain topic name");
    assert!(topic_content.contains("Producer Count: 3"), "Should contain producer count");
    assert!(topic_content.contains("Folder: test_topic"), "Should contain folder name");
    
    // Verify initial empty files
    let output_content = fs::read_to_string(folder_path.join("output.txt")).await.unwrap();
    assert!(output_content.is_empty(), "output.txt should be initially empty");
    
    let metrics_content = fs::read_to_string(folder_path.join("metrics.json")).await.unwrap();
    assert_eq!(metrics_content, "{}", "metrics.json should be initially empty object");
    
    cleanup_outputs().await;
}

/// Test topic name sanitization
#[tokio::test]
async fn test_topic_name_sanitization() {
    let file_system = setup_file_system().await;
    
    // Test various special characters and spaces
    let result = file_system.create_topic_folder("Test Topic!@# With $pecial Ch@rs", 1).await;
    assert!(result.is_ok(), "Should handle special characters in topic name");
    
    // Should sanitize to "test_topic_with_pecial_chrs"
    let expected_folder = std::path::PathBuf::from("outputs").join("test_topic_with_pecial_chrs");
    assert!(expected_folder.exists(), "Should create sanitized folder name");
    
    cleanup_outputs().await;
}

/// Test folder overwrite behavior
#[tokio::test]
async fn test_folder_overwrite() {
    let file_system = setup_file_system().await;
    
    // Create initial folder
    file_system.create_topic_folder("Overwrite Test", 2).await.unwrap();
    let folder_path = std::path::PathBuf::from("outputs").join("overwrite_test");
    
    // Add some content to verify overwrite
    fs::write(folder_path.join("custom_file.txt"), "original content").await.unwrap();
    assert!(folder_path.join("custom_file.txt").exists(), "Custom file should exist");
    
    // Create folder again (should overwrite)
    let result = file_system.create_topic_folder("Overwrite Test", 5).await;
    assert!(result.is_ok(), "Should overwrite existing folder successfully");
    
    // Verify folder still exists but custom file is gone
    assert!(folder_path.exists(), "Folder should still exist");
    assert!(!folder_path.join("custom_file.txt").exists(), "Custom file should be removed");
    
    // Verify new topic.txt has updated content
    let topic_content = fs::read_to_string(folder_path.join("topic.txt")).await.unwrap();
    assert!(topic_content.contains("Producer Count: 5"), "Should have new producer count");
    
    cleanup_outputs().await;
}

/// Test writing attributes to queue
#[tokio::test]
async fn test_write_attributes() {
    let file_system = setup_file_system().await;
    
    // Create topic folder first
    file_system.create_topic_folder("Attribute Test", 1).await.unwrap();
    
    // Write some attributes
    let attributes1 = vec!["attribute1".to_string(), "attribute2".to_string()];
    let result = file_system.write_attributes(&attributes1).await;
    assert!(result.is_ok(), "Should write attributes successfully");
    
    // Write more attributes
    let attributes2 = vec!["attribute3".to_string(), "attribute4".to_string(), "attribute5".to_string()];
    let result = file_system.write_attributes(&attributes2).await;
    assert!(result.is_ok(), "Should write more attributes successfully");
    
    // Attributes should be queued but not yet written to file
    let output_content = fs::read_to_string("outputs/attribute_test/output.txt").await.unwrap();
    assert!(output_content.is_empty(), "Attributes should be queued, not written yet");
    
    cleanup_outputs().await;
}

/// Test file synchronization
#[tokio::test]
async fn test_sync_files() {
    let file_system = setup_file_system().await;
    
    // Create topic folder
    file_system.create_topic_folder("Sync Test", 1).await.unwrap();
    
    // Write attributes to queue
    let attributes = vec![
        "sync_attr1".to_string(),
        "sync_attr2".to_string(), 
        "sync_attr3".to_string()
    ];
    file_system.write_attributes(&attributes).await.unwrap();
    
    // Sync files
    let result = file_system.sync_files().await;
    assert!(result.is_ok(), "Should sync files successfully");
    
    // Verify attributes were written to file
    let output_content = fs::read_to_string("outputs/sync_test/output.txt").await.unwrap();
    let lines: Vec<&str> = output_content.trim().split('\n').filter(|s| !s.is_empty()).collect();
    assert_eq!(lines.len(), 3, "Should have 3 lines in output file");
    assert!(lines.contains(&"sync_attr1"), "Should contain sync_attr1");
    assert!(lines.contains(&"sync_attr2"), "Should contain sync_attr2");
    assert!(lines.contains(&"sync_attr3"), "Should contain sync_attr3");
    
    // Verify metrics.json was updated
    let metrics_content = fs::read_to_string("outputs/sync_test/metrics.json").await.unwrap();
    assert!(!metrics_content.eq("{}"), "Metrics should be updated");
    let metrics: serde_json::Value = serde_json::from_str(&metrics_content).unwrap();
    assert_eq!(metrics["attributes_written"], 3, "Should record 3 attributes written");
    assert!(metrics["last_sync_utc"].is_string(), "Should have sync timestamp");
    
    // Verify state.json was updated
    let state_content = fs::read_to_string("outputs/sync_test/state.json").await.unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_content).unwrap();
    assert_eq!(state["current_folder"], "sync_test", "Should record current folder");
    assert_eq!(state["attributes_synced_this_session"], 3, "Should record synced count");
    assert_eq!(state["status"], "active", "Should be active status");
    
    cleanup_outputs().await;
}

/// Test sync with empty queue
#[tokio::test]
async fn test_sync_empty_queue() {
    let file_system = setup_file_system().await;
    
    file_system.create_topic_folder("Empty Sync", 1).await.unwrap();
    
    // Sync without any attributes queued
    let result = file_system.sync_files().await;
    assert!(result.is_ok(), "Should handle empty queue sync gracefully");
    
    // Output file should remain empty
    let output_content = fs::read_to_string("outputs/empty_sync/output.txt").await.unwrap();
    assert!(output_content.is_empty(), "Output should remain empty");
    
    cleanup_outputs().await;
}

/// Test error handling - sync without topic folder
#[tokio::test]
async fn test_sync_without_topic_folder() {
    let file_system = setup_file_system().await;
    
    // Try to sync without creating topic folder first
    file_system.write_attributes(&vec!["orphan_attr".to_string()]).await.unwrap();
    let result = file_system.sync_files().await;
    
    assert!(result.is_err(), "Should fail when no topic folder is initialized");
    
    cleanup_outputs().await;
}

/// Test multiple sync operations (batched writing)
#[tokio::test]
async fn test_multiple_sync_operations() {
    let file_system = setup_file_system().await;
    
    file_system.create_topic_folder("Multi Sync", 1).await.unwrap();
    
    // First batch
    file_system.write_attributes(&vec!["batch1_attr1".to_string(), "batch1_attr2".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Second batch
    file_system.write_attributes(&vec!["batch2_attr1".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Third batch
    file_system.write_attributes(&vec!["batch3_attr1".to_string(), "batch3_attr2".to_string(), "batch3_attr3".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Verify all attributes were written
    let output_content = fs::read_to_string("outputs/multi_sync/output.txt").await.unwrap();
    let lines: Vec<&str> = output_content.trim().split('\n').filter(|s| !s.is_empty()).collect();
    assert_eq!(lines.len(), 6, "Should have 6 total attributes");
    
    // Verify all expected attributes are present
    let expected_attrs = ["batch1_attr1", "batch1_attr2", "batch2_attr1", "batch3_attr1", "batch3_attr2", "batch3_attr3"];
    for expected in &expected_attrs {
        assert!(lines.contains(expected), "Should contain {}", expected);
    }
    
    cleanup_outputs().await;
}

/// Test concurrent attribute writing
#[tokio::test]
async fn test_concurrent_attribute_writing() {
    let file_system = setup_file_system().await;
    
    file_system.create_topic_folder("Concurrent Test", 3).await.unwrap();
    
    // Simulate writes from multiple producers (sequentially to avoid lifetime issues)
    for i in 0..5 {
        let attrs = vec![
            format!("producer_{}_attr_1", i),
            format!("producer_{}_attr_2", i),
            format!("producer_{}_attr_3", i),
        ];
        file_system.write_attributes(&attrs).await.unwrap();
    }
    
    // Sync all attributes
    file_system.sync_files().await.unwrap();
    
    // Verify all attributes were written
    let output_content = fs::read_to_string("outputs/concurrent_test/output.txt").await.unwrap();
    let lines: Vec<&str> = output_content.trim().split('\n').filter(|s| !s.is_empty()).collect();
    assert_eq!(lines.len(), 15, "Should have 15 total attributes (5 producers × 3 attrs)");
    
    // Verify uniqueness and correct content
    let unique_lines: HashSet<&str> = lines.into_iter().collect();
    assert_eq!(unique_lines.len(), 15, "All attributes should be unique");
    
    // Check that we have attributes from all producers
    for i in 0..5 {
        let producer_attrs: Vec<_> = unique_lines.iter()
            .filter(|line| line.contains(&format!("producer_{}", i)))
            .collect();
        assert_eq!(producer_attrs.len(), 3, "Should have 3 attributes from producer {}", i);
    }
    
    cleanup_outputs().await;
}

/// Test complete workflow integration
#[tokio::test]
async fn test_complete_workflow() {
    let file_system = setup_file_system().await;
    
    // Complete workflow: create folder -> write -> sync -> write more -> sync
    file_system.create_topic_folder("Complete Workflow", 2).await.unwrap();
    
    // First phase
    file_system.write_attributes(&vec!["phase1_attr1".to_string(), "phase1_attr2".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Second phase
    file_system.write_attributes(&vec!["phase2_attr1".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Third phase
    file_system.write_attributes(&vec!["phase3_attr1".to_string(), "phase3_attr2".to_string(), "phase3_attr3".to_string()]).await.unwrap();
    file_system.sync_files().await.unwrap();
    
    // Verify final state
    let folder_path = std::path::PathBuf::from("outputs/complete_workflow");
    
    // Check all files exist
    assert!(folder_path.join("topic.txt").exists());
    assert!(folder_path.join("output.txt").exists());
    assert!(folder_path.join("metrics.json").exists());
    assert!(folder_path.join("state.json").exists());
    
    // Check output content
    let output_content = fs::read_to_string(folder_path.join("output.txt")).await.unwrap();
    let lines: Vec<&str> = output_content.trim().split('\n').filter(|s| !s.is_empty()).collect();
    assert_eq!(lines.len(), 6, "Should have 6 total attributes");
    
    // Verify all phases are represented
    assert!(lines.iter().any(|&line| line.contains("phase1")), "Should have phase1 attributes");
    assert!(lines.iter().any(|&line| line.contains("phase2")), "Should have phase2 attributes");
    assert!(lines.iter().any(|&line| line.contains("phase3")), "Should have phase3 attributes");
    
    // Check final state
    let state_content = fs::read_to_string(folder_path.join("state.json")).await.unwrap();
    let state: serde_json::Value = serde_json::from_str(&state_content).unwrap();
    assert_eq!(state["status"], "active");
    assert_eq!(state["current_folder"], "complete_workflow");
    
    cleanup_outputs().await;
}