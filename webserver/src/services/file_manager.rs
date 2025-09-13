//! File manager service implementation
//!
//! This service handles serving static web assets like HTML, CSS, and JavaScript files.

use std::collections::HashMap;
use axum::response::{Response, Html};
use axum::http::{StatusCode, header};
use rust_embed::RustEmbed;
use crate::traits::FileManager;
use crate::error::{WebServerError, WebServerResult};

/// Embedded static assets
#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticAssets;

/// Real file manager service implementation
#[derive(Clone)]
pub struct RealFileManager {
    content_types: HashMap<&'static str, &'static str>,
}

impl RealFileManager {
    /// Create a new static file service
    pub fn new() -> Self {
        let mut content_types = HashMap::new();
        content_types.insert("html", "text/html; charset=utf-8");
        content_types.insert("css", "text/css; charset=utf-8");
        content_types.insert("js", "application/javascript; charset=utf-8");
        content_types.insert("json", "application/json; charset=utf-8");
        content_types.insert("png", "image/png");
        content_types.insert("jpg", "image/jpeg");
        content_types.insert("jpeg", "image/jpeg");
        content_types.insert("gif", "image/gif");
        content_types.insert("svg", "image/svg+xml");
        content_types.insert("ico", "image/x-icon");
        content_types.insert("woff", "font/woff");
        content_types.insert("woff2", "font/woff2");
        content_types.insert("ttf", "font/ttf");

        Self { content_types }
    }

    /// Get file extension from path
    fn get_extension<'a>(&self, path: &'a str) -> Option<&'a str> {
        if path.contains('.') {
            path.split('.').last()
        } else {
            None
        }
    }

    /// Normalize file path
    fn normalize_path(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        
        // Handle root path
        if path.is_empty() || path == "index" {
            return "index.html".to_string();
        }
        
        // Handle paths without extensions - assume HTML
        if !path.contains('.') {
            return format!("{}.html", path);
        }
        
        path.to_string()
    }
}

#[async_trait::async_trait]
impl FileManager for RealFileManager {
    async fn serve_file(&self, path: &str) -> WebServerResult<Response> {
        let normalized_path = self.normalize_path(path);
        
        match StaticAssets::get(&normalized_path) {
            Some(content) => {
                let content_type = self.get_content_type(&normalized_path);
                
                let response = Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CACHE_CONTROL, "public, max-age=3600") // 1 hour cache
                    .body(axum::body::Body::from(content.data))
                    .map_err(|e| WebServerError::ResponseError(e.to_string()))?;
                
                Ok(response)
            }
            None => {
                // Try to serve 404 page if available, otherwise return basic 404
                match StaticAssets::get("404.html") {
                    Some(content) => {
                        let response = Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                            .body(axum::body::Body::from(content.data))
                            .map_err(|e| WebServerError::ResponseError(e.to_string()))?;
                        
                        Ok(response)
                    }
                    None => {
                        let html = Html(format!(
                            r#"
                            <!DOCTYPE html>
                            <html>
                            <head><title>Not Found</title></head>
                            <body>
                                <h1>404 - File Not Found</h1>
                                <p>The requested file '{}' was not found.</p>
                                <p><a href="/">Return to Dashboard</a></p>
                            </body>
                            </html>
                            "#,
                            path
                        ));
                        
                        let response = Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                            .body(axum::body::Body::from(html.0))
                            .map_err(|e| WebServerError::ResponseError(e.to_string()))?;
                        
                        Ok(response)
                    }
                }
            }
        }
    }

    async fn file_exists(&self, path: &str) -> bool {
        let normalized_path = self.normalize_path(path);
        StaticAssets::get(&normalized_path).is_some()
    }

    fn get_content_type(&self, path: &str) -> String {
        if let Some(extension) = self.get_extension(path) {
            self.content_types.get(extension).copied().unwrap_or("application/octet-stream").to_string()
        } else {
            "text/html; charset=utf-8".to_string() // Default for files without extension
        }
    }

    async fn get_file_size(&self, path: &str) -> WebServerResult<u64> {
        let normalized_path = self.normalize_path(path);
        
        match StaticAssets::get(&normalized_path) {
            Some(content) => Ok(content.data.len() as u64),
            None => Err(WebServerError::FileNotFound(path.to_string())),
        }
    }
}

impl Default for RealFileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_exists() {
        let service = RealFileManager::new();
        
        // Test existing files (assuming they exist in static/ folder)
        assert!(service.file_exists("index.html").await);
        assert!(service.file_exists("").await); // Should map to index.html
        assert!(service.file_exists("/").await); // Should map to index.html
        
        // Test non-existing file
        assert!(!service.file_exists("nonexistent.html").await);
    }

    #[test]
    fn test_normalize_path() {
        let service = RealFileManager::new();
        
        assert_eq!(service.normalize_path(""), "index.html");
        assert_eq!(service.normalize_path("/"), "index.html");
        assert_eq!(service.normalize_path("index"), "index.html");
        assert_eq!(service.normalize_path("/static/app.css"), "static/app.css");
        assert_eq!(service.normalize_path("app.js"), "app.js");
        assert_eq!(service.normalize_path("about"), "about.html");
    }

    #[test]
    fn test_content_types() {
        let service = RealFileManager::new();
        
        assert_eq!(service.get_content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(service.get_content_type("app.css"), "text/css; charset=utf-8");
        assert_eq!(service.get_content_type("app.js"), "application/javascript; charset=utf-8");
        assert_eq!(service.get_content_type("data.json"), "application/json; charset=utf-8");
        assert_eq!(service.get_content_type("image.png"), "image/png");
        assert_eq!(service.get_content_type("unknown.xyz"), "application/octet-stream");
        assert_eq!(service.get_content_type("noextension"), "text/html; charset=utf-8");
    }

    #[tokio::test]
    async fn test_get_file_size() {
        let service = RealFileManager::new();
        
        // Test with existing file
        if service.file_exists("index.html").await {
            let size = service.get_file_size("index.html").await;
            assert!(size.is_ok());
            assert!(size.unwrap() > 0);
        }
        
        // Test with non-existing file
        let size = service.get_file_size("nonexistent.html").await;
        assert!(size.is_err());
        matches!(size.unwrap_err(), WebServerError::FileNotFound(_));
    }
}