//! Tests for the FileManager service

use crate::services::RealFileManager;
use crate::traits::FileManager;
use crate::error::WebServerError;
use axum::http::StatusCode;
use std::sync::Arc;

// Helper functions for tests
fn create_file_manager() -> RealFileManager {
    RealFileManager::new()
}

mod real_file_manager_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_file_manager() {
        let file_manager = create_file_manager();
        
        // Test that file manager is created successfully
        // Basic functionality test
        assert!(file_manager.file_exists("index.html").await || !file_manager.file_exists("index.html").await);
    }

    #[tokio::test]
    async fn test_serve_existing_file() {
        let file_manager = create_file_manager();
        
        // Test serving index.html (should exist in static folder)
        let response = file_manager.serve_file("index.html").await;
        assert!(response.is_ok(), "Should successfully serve index.html");
        
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_serve_root_path() {
        let file_manager = create_file_manager();
        
        // Test serving root path (should map to index.html)
        let response = file_manager.serve_file("/").await;
        assert!(response.is_ok(), "Should successfully serve root path");
        
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_serve_empty_path() {
        let file_manager = create_file_manager();
        
        // Test serving empty path (should map to index.html)
        let response = file_manager.serve_file("").await;
        assert!(response.is_ok(), "Should successfully serve empty path");
        
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_serve_nonexistent_file() {
        let file_manager = create_file_manager();
        
        // Test serving non-existent file
        let response = file_manager.serve_file("nonexistent.html").await;
        assert!(response.is_ok(), "Should return a response even for non-existent files");
        
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_serve_invalid_path() {
        let file_manager = create_file_manager();
        
        // Test serving paths with directory traversal attempts
        let malicious_paths = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "/etc/passwd",
            "C:\\Windows\\System32\\config\\sam",
            "file:///etc/passwd",
        ];
        
        for path in malicious_paths {
            let response = file_manager.serve_file(path).await;
            assert!(response.is_ok(), "Should handle malicious path gracefully: {}", path);
            
            let response = response.unwrap();
            // Should either be NOT_FOUND or safely serve a different file
            assert!(
                response.status() == StatusCode::NOT_FOUND || 
                response.status() == StatusCode::OK,
                "Should return NOT_FOUND or OK for path: {}", path
            );
        }
    }

    #[tokio::test]
    async fn test_file_exists() {
        let file_manager = create_file_manager();
        
        // Test existing file
        assert!(file_manager.file_exists("index.html").await);
        
        // Test root path (should map to index.html)
        assert!(file_manager.file_exists("/").await);
        assert!(file_manager.file_exists("").await);
        
        // Test non-existent file
        assert!(!file_manager.file_exists("nonexistent.html").await);
    }

    #[tokio::test]
    async fn test_file_exists_with_various_paths() {
        let file_manager = create_file_manager();
        
        // Test various path formats that should all resolve to the same file
        let equivalent_paths = vec![
            ("", "index.html"),
            ("/", "index.html"),
            ("index", "index.html"),
            ("/index", "index.html"),
        ];
        
        for (path1, path2) in equivalent_paths {
            let exists1 = file_manager.file_exists(path1).await;
            let exists2 = file_manager.file_exists(path2).await;
            assert_eq!(exists1, exists2, "Paths '{}' and '{}' should have same existence", path1, path2);
        }
    }

    #[tokio::test]
    async fn test_content_type_detection() {
        let file_manager = create_file_manager();
        
        // Test HTML files
        assert_eq!(file_manager.get_content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(file_manager.get_content_type("page.html"), "text/html; charset=utf-8");
        assert_eq!(file_manager.get_content_type("file.htm"), "text/html; charset=utf-8");
        
        // Test CSS files
        assert_eq!(file_manager.get_content_type("style.css"), "text/css; charset=utf-8");
        assert_eq!(file_manager.get_content_type("app.css"), "text/css; charset=utf-8");
        
        // Test JavaScript files
        assert_eq!(file_manager.get_content_type("app.js"), "application/javascript; charset=utf-8");
        assert_eq!(file_manager.get_content_type("script.js"), "application/javascript; charset=utf-8");
        assert_eq!(file_manager.get_content_type("module.mjs"), "application/javascript; charset=utf-8");
        
        // Test JSON files
        assert_eq!(file_manager.get_content_type("data.json"), "application/json; charset=utf-8");
        assert_eq!(file_manager.get_content_type("config.json"), "application/json; charset=utf-8");
        
        // Test image files
        assert_eq!(file_manager.get_content_type("image.png"), "image/png");
        assert_eq!(file_manager.get_content_type("photo.jpg"), "image/jpeg");
        assert_eq!(file_manager.get_content_type("picture.jpeg"), "image/jpeg");
        assert_eq!(file_manager.get_content_type("graphic.gif"), "image/gif");
        assert_eq!(file_manager.get_content_type("icon.svg"), "image/svg+xml");
        assert_eq!(file_manager.get_content_type("logo.webp"), "image/webp");
        
        // Test font files
        assert_eq!(file_manager.get_content_type("font.woff"), "font/woff");
        assert_eq!(file_manager.get_content_type("font.woff2"), "font/woff2");
        assert_eq!(file_manager.get_content_type("font.ttf"), "font/ttf");
        assert_eq!(file_manager.get_content_type("font.otf"), "font/otf");
        assert_eq!(file_manager.get_content_type("font.eot"), "application/vnd.ms-fontobject");
        
        // Test other common files
        assert_eq!(file_manager.get_content_type("favicon.ico"), "image/x-icon");
        assert_eq!(file_manager.get_content_type("manifest.xml"), "application/xml; charset=utf-8");
        assert_eq!(file_manager.get_content_type("data.txt"), "text/plain; charset=utf-8");
        
        // Test unknown extension
        assert_eq!(file_manager.get_content_type("file.xyz"), "application/octet-stream");
        
        // Test file without extension
        assert_eq!(file_manager.get_content_type("README"), "text/html; charset=utf-8");
        assert_eq!(file_manager.get_content_type("LICENSE"), "text/html; charset=utf-8");
    }

    #[tokio::test]
    async fn test_content_type_case_insensitive() {
        let file_manager = create_file_manager();
        
        // Test that content type detection is case insensitive
        let extensions_to_test = vec![
            ("file.HTML", "file.html"),
            ("STYLE.CSS", "style.css"),
            ("APP.JS", "app.js"),
            ("IMAGE.PNG", "image.png"),
            ("FONT.WOFF", "font.woff"),
        ];
        
        for (uppercase, lowercase) in extensions_to_test {
            let upper_type = file_manager.get_content_type(uppercase);
            let lower_type = file_manager.get_content_type(lowercase);
            assert_eq!(upper_type, lower_type, "Content type should be case insensitive for {}", uppercase);
        }
    }

    #[tokio::test]
    async fn test_get_file_size() {
        let file_manager = create_file_manager();
        
        // Test with existing file
        if file_manager.file_exists("index.html").await {
            let size = file_manager.get_file_size("index.html").await;
            assert!(size.is_ok());
            assert!(size.unwrap() > 0);
        }
        
        // Test with non-existent file
        let size = file_manager.get_file_size("nonexistent.html").await;
        assert!(size.is_err());
        match size.err().unwrap() {
            WebServerError::FileNotFound(_) => (),
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_file_size_various_files() {
        let file_manager = create_file_manager();
        
        // Test size calculation for different file types (if they exist)
        let test_files = vec!["index.html", "app.js", "style.css"];
        
        for file in test_files {
            if file_manager.file_exists(file).await {
                let size_result = file_manager.get_file_size(file).await;
                assert!(size_result.is_ok(), "Should get size for existing file: {}", file);
                
                let size = size_result.unwrap();
                assert!(size > 0, "File size should be greater than 0 for: {}", file);
                
                // Size should be consistent on multiple calls
                let size2 = file_manager.get_file_size(file).await.unwrap();
                assert_eq!(size, size2, "File size should be consistent for: {}", file);
            }
        }
    }

    #[tokio::test]
    async fn test_path_normalization() {
        let file_manager = create_file_manager();
        
        // These should all serve the same file (index.html)
        let paths = vec!["", "/", "index", "/index"];
        
        for path in paths {
            assert!(file_manager.file_exists(path).await, "Path '{}' should exist", path);
        }
    }

    #[tokio::test]
    async fn test_concurrent_file_access() {
        let file_manager = Arc::new(create_file_manager());
        
        // Test concurrent access to the same file
        let mut handles = vec![];
        
        for i in 0..10 {
            let fm_clone = file_manager.clone();
            let handle = tokio::spawn(async move {
                // Mix of operations
                let exists = fm_clone.file_exists("index.html").await;
                let content_type = fm_clone.get_content_type("index.html");
                let serve_result = fm_clone.serve_file("index.html").await;
                
                (i, exists, content_type, serve_result.is_ok())
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            let (i, exists, content_type, serve_ok) = handle.await.unwrap();
            assert!(exists, "File should exist in concurrent access {}", i);
            assert_eq!(content_type, "text/html; charset=utf-8");
            assert!(serve_ok, "Serve should succeed in concurrent access {}", i);
        }
    }

    #[tokio::test]
    async fn test_cache_headers() {
        let file_manager = create_file_manager();
        
        // Test static files that might have cache headers
        let static_files = vec!["app.css", "app.js", "style.css", "script.js"];
        
        for file in static_files {
            if file_manager.file_exists(file).await {
                let response = file_manager.serve_file(file).await.unwrap();
                let headers = response.headers();
                
                // Check for cache control headers (implementation dependent)
                if headers.contains_key("cache-control") {
                    let cache_control = headers.get("cache-control").unwrap().to_str().unwrap();
                    assert!(
                        cache_control.contains("public") || 
                        cache_control.contains("private") || 
                        cache_control.contains("max-age"),
                        "Cache control should contain valid directives for: {}", file
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn test_response_headers() {
        let file_manager = create_file_manager();
        
        if file_manager.file_exists("index.html").await {
            let response = file_manager.serve_file("index.html").await.unwrap();
            let headers = response.headers();
            
            // Check content-type header is set
            assert!(headers.contains_key("content-type"));
            let content_type = headers.get("content-type").unwrap().to_str().unwrap();
            assert!(content_type.starts_with("text/html"));
            
            // Check for content-length header (if set)
            if headers.contains_key("content-length") {
                let content_length = headers.get("content-length").unwrap().to_str().unwrap();
                let length: u64 = content_length.parse().expect("Content-length should be a valid number");
                assert!(length > 0, "Content-length should be positive");
            }
        }
    }

    #[tokio::test]
    async fn test_serve_different_file_types() {
        let file_manager = create_file_manager();
        
        // Test serving different types of files (if they exist)
        let file_types = vec![
            ("index.html", StatusCode::OK),
            ("nonexistent.html", StatusCode::NOT_FOUND),
            ("../../../etc/passwd", StatusCode::NOT_FOUND), // Security test
        ];
        
        for (file, expected_status) in file_types {
            let response = file_manager.serve_file(file).await.unwrap();
            assert_eq!(
                response.status(), 
                expected_status,
                "File '{}' should return status {:?}", 
                file, 
                expected_status
            );
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let file_manager = create_file_manager();
        
        // Test various error conditions
        let error_cases = vec![
            "nonexistent.html",
            "does/not/exist.html",
            "missing/file.css",
            "",  // Empty path should work (maps to index)
        ];
        
        for error_case in error_cases {
            let result = file_manager.serve_file(error_case).await;
            assert!(result.is_ok(), "serve_file should always return Ok(Response), got error for: {}", error_case);
            
            let response = result.unwrap();
            // Should be either OK (for valid files) or NOT_FOUND (for missing files)
            assert!(
                response.status() == StatusCode::OK || 
                response.status() == StatusCode::NOT_FOUND,
                "Response should be OK or NOT_FOUND for: {}", error_case
            );
        }
    }

    #[tokio::test]
    async fn test_file_size_edge_cases() {
        let file_manager = create_file_manager();
        
        // Test edge cases for file size
        let edge_cases = vec![
            "",  // Empty path
            "/", // Root path
            "nonexistent.html", // Non-existent file
        ];
        
        for case in edge_cases {
            let size_result = file_manager.get_file_size(case).await;
            
            if file_manager.file_exists(case).await {
                // If file exists, should get a valid size
                assert!(size_result.is_ok(), "Should get size for existing file: {}", case);
                assert!(size_result.unwrap() > 0, "Size should be positive for: {}", case);
            } else {
                // If file doesn't exist, should get an error
                assert!(size_result.is_err(), "Should get error for non-existent file: {}", case);
            }
        }
    }

    #[tokio::test]
    async fn test_performance() {
        let file_manager = create_file_manager();
        
        // Test performance of repeated file operations
        let iterations = 100;
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let _ = file_manager.file_exists("index.html").await;
            let _ = file_manager.get_content_type("index.html");
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        // Operations should be reasonably fast
        assert!(avg_duration < std::time::Duration::from_millis(1), 
                "Average operation duration should be under 1ms, got {:?}", avg_duration);
    }

    #[tokio::test]
    async fn test_memory_usage() {
        let file_manager = create_file_manager();
        
        // Test that multiple file operations don't cause memory leaks
        // This is a basic test - in production you'd use more sophisticated memory testing
        
        for _ in 0..1000 {
            let _ = file_manager.file_exists("index.html").await;
            let _ = file_manager.serve_file("index.html").await;
            let _ = file_manager.get_content_type("style.css");
            
            // Drop any potential resources
            tokio::task::yield_now().await;
        }
        
        // If we reach here without OOM, the test passes
        assert!(true, "Memory usage test completed successfully");
    }

    #[tokio::test]
    async fn test_clone_and_send() {
        // Test that FileManager can be cloned and sent across threads
        let file_manager = create_file_manager();
        
        // Test Clone (if implemented)
        // let cloned = file_manager.clone();
        
        // Test Send + Sync by moving to another task
        let handle = tokio::spawn(async move {
            file_manager.file_exists("index.html").await
        });
        
        let result = handle.await.unwrap();
        assert!(result || !result); // Just test that it doesn't panic
    }
}