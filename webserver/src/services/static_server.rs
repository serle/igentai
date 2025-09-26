//! Static file serving service
//!
//! Serves frontend assets with proper caching and content types

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{WebServerError, WebServerResult};
use crate::traits::{StaticFileResponse, StaticFileServer};

/// Real static file server implementation
#[derive(Clone)]
pub struct RealStaticFileServer {
    /// Base directory for static files
    base_dir: PathBuf,

    /// MIME type mappings
    mime_types: HashMap<String, String>,
}

impl RealStaticFileServer {
    /// Create new static file server
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let mut mime_types = HashMap::new();
        mime_types.insert("html".to_string(), "text/html; charset=utf-8".to_string());
        mime_types.insert("css".to_string(), "text/css".to_string());
        mime_types.insert("js".to_string(), "application/javascript".to_string());
        mime_types.insert("json".to_string(), "application/json".to_string());
        mime_types.insert("png".to_string(), "image/png".to_string());
        mime_types.insert("jpg".to_string(), "image/jpeg".to_string());
        mime_types.insert("jpeg".to_string(), "image/jpeg".to_string());
        mime_types.insert("gif".to_string(), "image/gif".to_string());
        mime_types.insert("svg".to_string(), "image/svg+xml".to_string());
        mime_types.insert("ico".to_string(), "image/x-icon".to_string());
        mime_types.insert("woff".to_string(), "font/woff".to_string());
        mime_types.insert("woff2".to_string(), "font/woff2".to_string());

        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            mime_types,
        }
    }

    /// Get MIME type from file extension
    fn get_mime_type(&self, path: &str) -> String {
        if let Some(extension) = Path::new(path).extension().and_then(|e| e.to_str()) {
            self.mime_types
                .get(&extension.to_lowercase())
                .unwrap_or(&"application/octet-stream".to_string())
                .clone()
        } else {
            "application/octet-stream".to_string()
        }
    }

    /// Get cache control header based on file type
    fn get_cache_control(&self, path: &str) -> Option<String> {
        if let Some(extension) = Path::new(path).extension().and_then(|e| e.to_str()) {
            match extension.to_lowercase().as_str() {
                "html" => Some("no-cache".to_string()),
                "js" | "css" => Some("public, max-age=3600".to_string()), // 1 hour
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" => Some("public, max-age=86400".to_string()), // 1 day
                "woff" | "woff2" => Some("public, max-age=604800".to_string()), // 1 week
                _ => None,
            }
        } else {
            None
        }
    }

    /// Resolve file path and prevent directory traversal
    fn resolve_path(&self, request_path: &str) -> WebServerResult<PathBuf> {
        // Remove leading slash and normalize path
        let clean_path = request_path.trim_start_matches('/');

        // Handle empty path or root - serve index.html
        let file_path = if clean_path.is_empty() || clean_path == "index.html" {
            "index.html"
        } else {
            clean_path
        };

        // Build full path
        let full_path = self.base_dir.join(file_path);

        // Canonicalize to prevent directory traversal
        let canonical_path = match full_path.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                return Err(WebServerError::http(format!("File not found: {}", request_path)));
            }
        };

        let canonical_base = match self.base_dir.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                shared::process_error!(shared::ProcessId::current(), "Failed to canonicalize base directory: {}", e);
                return Err(WebServerError::internal(
                    "Static file base directory not accessible".to_string(),
                ));
            }
        };

        // Ensure the resolved path is within the base directory
        if !canonical_path.starts_with(&canonical_base) {
            return Err(WebServerError::http("Access denied".to_string()));
        }

        Ok(canonical_path)
    }
}

#[async_trait]
impl StaticFileServer for RealStaticFileServer {
    async fn serve_file(&self, path: &str) -> WebServerResult<StaticFileResponse> {
        let file_path = self.resolve_path(path)?;

        // Check if it's a directory
        if file_path.is_dir() {
            // Try to serve index.html from the directory
            let index_path = file_path.join("index.html");
            if index_path.exists() {
                return self.serve_file("index.html").await;
            } else {
                return Err(WebServerError::http("Directory listing not allowed".to_string()));
            }
        }

        // Read file content
        match fs::read(&file_path).await {
            Ok(content) => {
                let content_type = self.get_mime_type(path);
                let cache_control = self.get_cache_control(path);

                shared::process_info!(shared::ProcessId::current(), "📄 Served static file: {} ({} bytes)", path, content.len());

                let mut response = StaticFileResponse::new(content, content_type);
                if let Some(cache) = cache_control {
                    response = response.with_cache_control(cache);
                }

                Ok(response)
            }
            Err(e) => {
                shared::process_warn!(shared::ProcessId::current(), "❌ Failed to read static file {}: {}", path, e);
                Err(WebServerError::http(format!("File not found: {}", path)))
            }
        }
    }

    async fn file_exists(&self, path: &str) -> bool {
        match self.resolve_path(path) {
            Ok(file_path) => file_path.exists(),
            Err(_) => false,
        }
    }

    async fn content_type(&self, path: &str) -> WebServerResult<String> {
        Ok(self.get_mime_type(path))
    }

    async fn list_files(&self) -> WebServerResult<Vec<String>> {
        let mut files = Vec::new();

        fn collect_files(dir: &Path, base: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
            let entries = std::fs::read_dir(dir)?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Ok(relative_path) = path.strip_prefix(base) {
                        if let Some(path_str) = relative_path.to_str() {
                            files.push(path_str.to_string());
                        }
                    }
                } else if path.is_dir() {
                    collect_files(&path, base, files)?;
                }
            }

            Ok(())
        }

        match collect_files(&self.base_dir, &self.base_dir, &mut files) {
            Ok(_) => {
                files.sort();
                Ok(files)
            }
            Err(e) => {
                shared::process_error!(shared::ProcessId::current(), "Failed to list files: {}", e);
                Err(WebServerError::internal(format!("Failed to list files: {}", e)))
            }
        }
    }
}

impl Default for RealStaticFileServer {
    fn default() -> Self {
        Self::new("./static")
    }
}
