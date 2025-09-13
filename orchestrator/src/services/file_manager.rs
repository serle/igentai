//! File system service implementation
//!
//! This module contains the production file system implementation that handles
//! topic folder creation, attribute writing, and file synchronization operations.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use crate::error::{OrchestratorError, OrchestratorResult};
use crate::traits::FileSystem;

/// Real file system implementation for production
/// 
/// Provides actual file I/O operations with topic folder management,
/// batched attribute writing, and comprehensive metadata tracking.
pub struct RealFileSystem {
    current_topic_folder: Arc<Mutex<Option<PathBuf>>>,
    pending_writes: Arc<Mutex<Vec<String>>>,
}

impl RealFileSystem {
    /// Create a new file system service instance
    pub fn new() -> Self {
        Self {
            current_topic_folder: Arc::new(Mutex::new(None)),
            pending_writes: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Sanitize topic name for use as folder name
    /// 
    /// Removes special characters and converts to lowercase with underscores
    fn sanitize_topic_name(topic: &str) -> String {
        topic
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join("_")
    }
}

#[async_trait::async_trait]
impl FileSystem for RealFileSystem {
    async fn create_topic_folder(&self, topic: &str, producer_count: u32) -> OrchestratorResult<()> {
        let folder_name = Self::sanitize_topic_name(topic);
        let folder_path = PathBuf::from("outputs").join(&folder_name);
        
        // Remove existing folder if it exists (overwrite behavior)
        if folder_path.exists() {
            fs::remove_dir_all(&folder_path).await
                .map_err(|_| OrchestratorError::TopicFolderError { topic: topic.to_string() })?;
            println!("Overwriting existing topic folder: {}", folder_name);
        }
        
        // Create output directory
        fs::create_dir_all(&folder_path).await
            .map_err(|_| OrchestratorError::TopicFolderError { topic: topic.to_string() })?;
        
        // Initialize topic.txt with metadata
        let topic_content = format!(
            "Topic: {}\nStart Time: {} UTC\nProducer Count: {}\nFolder: {}\nCreated: {}\n",
            topic,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            producer_count,
            folder_name,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        );
        
        fs::write(folder_path.join("topic.txt"), topic_content).await
            .map_err(|_| OrchestratorError::FileSystemError {
                operation: "write_topic_file".to_string(),
                path: folder_path.join("topic.txt").display().to_string(),
            })?;
        
        // Initialize empty output.txt for attribute writing
        fs::write(folder_path.join("output.txt"), "").await
            .map_err(|_| OrchestratorError::FileSystemError {
                operation: "create_output_file".to_string(),
                path: folder_path.join("output.txt").display().to_string(),
            })?;
        
        // Initialize empty metrics.json and state.json
        fs::write(folder_path.join("metrics.json"), "{}").await?;
        fs::write(folder_path.join("state.json"), "{}").await?;
        
        // Update current folder tracking
        *self.current_topic_folder.lock().await = Some(folder_path.clone());
        
        println!("Created topic folder with all files: outputs/{}/", folder_name);
        Ok(())
    }
    
    async fn write_attributes(&self, attributes: &[String]) -> OrchestratorResult<()> {
        // Add to pending writes queue for batch processing
        let mut pending = self.pending_writes.lock().await;
        pending.extend(attributes.iter().cloned());
        
        println!("Queued {} attributes for writing (total pending: {})", 
                attributes.len(), pending.len());
        Ok(())
    }
    
    async fn sync_files(&self) -> OrchestratorResult<()> {
        let mut pending = self.pending_writes.lock().await;
        if pending.is_empty() {
            return Ok(());
        }
        
        let folder_path = self.current_topic_folder.lock().await.clone()
            .ok_or(OrchestratorError::FileSystemError {
                operation: "sync_files".to_string(),
                path: "no topic folder initialized".to_string(),
            })?;
        
        // Append all pending attributes to output.txt
        let output_file_path = folder_path.join("output.txt");
        let mut output_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output_file_path)
            .await
            .map_err(|_| OrchestratorError::FileSystemError {
                operation: "open_output_file".to_string(),
                path: output_file_path.display().to_string(),
            })?;
        
        let write_count = pending.len();
        for attribute in pending.iter() {
            output_file.write_all(format!("{}\n", attribute).as_bytes()).await?;
        }
        output_file.flush().await?;
        pending.clear();
        
        // Update metrics.json with synchronization info
        let metrics_content = serde_json::json!({
            "last_sync_utc": chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            "last_sync_timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "attributes_written": write_count,
            "total_syncs": 1
        });
        
        fs::write(
            folder_path.join("metrics.json"), 
            serde_json::to_string_pretty(&metrics_content)?
        ).await?;
        
        // Update state.json with current state
        let state_content = serde_json::json!({
            "current_folder": folder_path.file_name().unwrap().to_str().unwrap(),
            "last_updated": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "attributes_synced_this_session": write_count,
            "status": "active"
        });
        
        fs::write(
            folder_path.join("state.json"),
            serde_json::to_string_pretty(&state_content)?
        ).await?;
        
        println!("File sync completed: {} attributes written to outputs/{}/", 
                write_count, folder_path.file_name().unwrap().to_str().unwrap());
        Ok(())
    }
}

