//! Real file system service implementation
//! 
//! Handles file I/O operations for storing unique attributes and managing
//! topic directories with atomic writes and proper error handling.

use std::path::PathBuf;
use async_trait::async_trait;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::{OrchestratorError, OrchestratorResult};
use crate::traits::FileSystem;
use shared::process_debug;

/// Real file system implementation
pub struct RealFileSystem {
    /// Base directory for all data
    base_dir: PathBuf,
}

impl RealFileSystem {
    /// Create new file system service (outputs to ./output folder)
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from("./output"),
        }
    }
    
    /// Create with custom base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
    
    /// Get topic directory path
    fn topic_dir_path(&self, topic: &str) -> PathBuf {
        // For webserver mode, use a "default" topic name
        if topic.is_empty() {
            self.base_dir.join("default")
        } else {
            self.base_dir.join(topic)
        }
    }
    
    /// Get attributes file path for a topic
    fn attributes_file_path(&self, topic: &str) -> PathBuf {
        self.topic_dir_path(topic).join("output.json")
    }
    
    /// Get metadata file path for a topic
    fn metadata_file_path(&self, topic: &str) -> PathBuf {
        self.topic_dir_path(topic).join("metadata.json")
    }
    
    /// Get output.txt file path for a topic
    fn output_file_path(&self, topic: &str) -> PathBuf {
        self.topic_dir_path(topic).join("output.txt")
    }
}

#[async_trait]
impl FileSystem for RealFileSystem {
    async fn create_topic_directory(&self, topic: &str) -> OrchestratorResult<()> {
        let topic_dir = self.topic_dir_path(topic);
        
        // Clean up any existing topic directory first
        if topic_dir.exists() {
            self.cleanup_topic(topic).await?;
        }
        
        // Create directory structure
        fs::create_dir_all(&topic_dir).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        // Create metadata file
        let metadata = serde_json::json!({
            "topic": topic,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "version": "1.0",
            "total_attributes": 0
        });
        
        let metadata_path = self.metadata_file_path(topic);
        let metadata_content = serde_json::to_string_pretty(&metadata)
            .map_err(|e| OrchestratorError::JsonError { source: e })?;
        
        fs::write(&metadata_path, metadata_content).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        process_debug!(shared::ProcessId::current(), "📁 Created topic directory: {}", topic_dir.display());
        Ok(())
    }
    
    async fn write_unique_attributes(&self, topic: &str, attributes: &[String]) -> OrchestratorResult<()> {
        if attributes.is_empty() {
            return Ok(());
        }
        
        let attributes_path = self.attributes_file_path(topic);
        
        // Append attributes to JSONL file (one JSON object per line)
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&attributes_path)
            .await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        let timestamp = chrono::Utc::now().to_rfc3339();
        for attribute in attributes {
            let entry = serde_json::json!({
                "attribute": attribute,
                "timestamp": timestamp
            });
            
            let line = serde_json::to_string(&entry)
                .map_err(|e| OrchestratorError::JsonError { source: e })?;
            
            file.write_all(format!("{}\n", line).as_bytes()).await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        }
        
        file.flush().await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        // Update metadata
        self.update_metadata_count(topic, attributes.len()).await?;
        
        process_debug!(shared::ProcessId::current(), "💾 Wrote {} unique attributes for topic '{}'", attributes.len(), topic);
        Ok(())
    }
    
    async fn write_unique_attributes_with_metadata(&self, topic: &str, attributes: &[String], provider_metadata: &shared::types::ProviderMetadata) -> OrchestratorResult<()> {
        if attributes.is_empty() {
            return Ok(());
        }
        
        let attributes_path = self.attributes_file_path(topic);
        
        // Append attributes to JSON file (one JSON object per line)
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&attributes_path)
            .await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        let timestamp = chrono::Utc::now().to_rfc3339();
        for attribute in attributes {
            let entry = serde_json::json!({
                "attribute": attribute,
                "model": provider_metadata.model,
                "provider": provider_metadata.provider_id.to_string().to_lowercase(),
                "timestamp": timestamp
            });
            
            let line = serde_json::to_string(&entry)
                .map_err(|e| OrchestratorError::JsonError { source: e })?;
            
            file.write_all(format!("{}\n", line).as_bytes()).await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        }
        
        file.flush().await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        // Update metadata
        self.update_metadata_count(topic, attributes.len()).await?;
        
        process_debug!(shared::ProcessId::current(), "💾 Wrote {} unique attributes with provider metadata for topic '{}'", attributes.len(), topic);
        Ok(())
    }
    
    async fn read_topic_attributes(&self, topic: &str) -> OrchestratorResult<Vec<String>> {
        let attributes_path = self.attributes_file_path(topic);
        
        // Check if file exists
        if !attributes_path.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&attributes_path).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        let mut attributes = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            
            let entry: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| OrchestratorError::JsonError { source: e })?;
            
            if let Some(attribute) = entry.get("attribute").and_then(|v| v.as_str()) {
                attributes.push(attribute.to_string());
            }
        }
        
        Ok(attributes)
    }
    
    async fn cleanup_topic(&self, topic: &str) -> OrchestratorResult<()> {
        let topic_dir = self.topic_dir_path(topic);
        
        if topic_dir.exists() {
            fs::remove_dir_all(&topic_dir).await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
            
            process_debug!(shared::ProcessId::current(), "🗑️ Cleaned up topic directory: {}", topic_dir.display());
        }
        
        Ok(())
    }
    
    async fn sync_to_disk(&self) -> OrchestratorResult<()> {
        // On most systems, fsync is handled by the OS, but we can ensure
        // any buffered writes are flushed
        
        // Create a temporary file and sync it to force filesystem flush
        let sync_file = self.base_dir.join(".sync_marker");
        fs::write(&sync_file, "sync").await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        // Remove the sync marker
        let _ = fs::remove_file(&sync_file).await;
        
        process_debug!(shared::ProcessId::current(), "💽 File system synced to disk");
        Ok(())
    }
    
    async fn append_to_output(&self, topic: &str, new_attributes: &[String]) -> OrchestratorResult<()> {
        if new_attributes.is_empty() {
            return Ok(());
        }
        
        let output_path = self.output_file_path(topic);
        
        // Append new attributes to output.txt (one per line)
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_path)
            .await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        for attribute in new_attributes {
            file.write_all(attribute.as_bytes()).await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
            file.write_all(b"\n").await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        }
        
        file.flush().await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        process_debug!(shared::ProcessId::current(), "📝 Appended {} new attributes to {}", new_attributes.len(), output_path.display());
        Ok(())
    }
    
    async fn write_file(&self, filename: &str, content: &[u8]) -> OrchestratorResult<()> {
        // Ensure base directory exists
        fs::create_dir_all(&self.base_dir).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
            
        let file_path = self.base_dir.join(filename);
        
        fs::write(&file_path, content).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        process_debug!(shared::ProcessId::current(), "📝 Wrote file: {}", file_path.display());
        Ok(())
    }
}

impl RealFileSystem {
    /// Update the metadata file with new attribute count
    async fn update_metadata_count(&self, topic: &str, additional_count: usize) -> OrchestratorResult<()> {
        let metadata_path = self.metadata_file_path(topic);
        
        // Read current metadata
        let mut metadata = if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path).await
                .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| OrchestratorError::JsonError { source: e })?
        } else {
            serde_json::json!({
                "topic": topic,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "version": "1.0",
                "total_attributes": 0
            })
        };
        
        // Update count
        let current_count = metadata["total_attributes"].as_u64().unwrap_or(0);
        metadata["total_attributes"] = serde_json::Value::from(current_count + additional_count as u64);
        metadata["updated_at"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
        
        // Write back
        let updated_content = serde_json::to_string_pretty(&metadata)
            .map_err(|e| OrchestratorError::JsonError { source: e })?;
        
        fs::write(&metadata_path, updated_content).await
            .map_err(|e| OrchestratorError::FileSystemError { source: e })?;
        
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn create_test_fs() -> (RealFileSystem, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let fs = RealFileSystem::with_base_dir(temp_dir.path().to_path_buf());
        (fs, temp_dir)
    }
    
    #[tokio::test]
    async fn test_create_topic_directory() {
        let (fs, _temp) = create_test_fs().await;
        
        let result = fs.create_topic_directory("test_topic").await;
        assert!(result.is_ok());
        
        // Check directory exists
        let topic_dir = fs.topic_dir_path("test_topic");
        assert!(topic_dir.exists());
        
        // Check metadata file exists
        let metadata_path = fs.metadata_file_path("test_topic");
        assert!(metadata_path.exists());
    }
    
    #[tokio::test]
    async fn test_write_and_read_attributes() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create topic directory first
        fs.create_topic_directory("test_topic").await.unwrap();
        
        // Write attributes
        let attributes = vec![
            "attribute1".to_string(),
            "attribute2".to_string(),
            "attribute3".to_string(),
        ];
        
        let result = fs.write_unique_attributes("test_topic", &attributes).await;
        assert!(result.is_ok());
        
        // Read attributes back
        let read_attributes = fs.read_topic_attributes("test_topic").await.unwrap();
        assert_eq!(read_attributes.len(), 3);
        assert!(read_attributes.contains(&"attribute1".to_string()));
        assert!(read_attributes.contains(&"attribute2".to_string()));
        assert!(read_attributes.contains(&"attribute3".to_string()));
    }
    
    #[tokio::test]
    async fn test_cleanup_topic() {
        let (fs, _temp) = create_test_fs().await;
        
        // Create topic directory
        fs.create_topic_directory("cleanup_test").await.unwrap();
        
        // Verify it exists
        let topic_dir = fs.topic_dir_path("cleanup_test");
        assert!(topic_dir.exists());
        
        // Cleanup
        let result = fs.cleanup_topic("cleanup_test").await;
        assert!(result.is_ok());
        
        // Verify it's gone
        assert!(!topic_dir.exists());
    }
    
    
    #[tokio::test]
    async fn test_sync_to_disk() {
        let (fs, _temp) = create_test_fs().await;
        
        // Should not fail
        let result = fs.sync_to_disk().await;
        assert!(result.is_ok());
    }
}