//! Static file serving handlers
//! 
//! Serve frontend assets with proper caching and content types

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, Response},
};
use std::sync::Arc;

use crate::traits::StaticFileServer;

/// Serve index.html for root path
pub async fn serve_index<S>(
    State(static_server): State<Arc<S>>,
) -> Result<Html<String>, StatusCode>
where
    S: StaticFileServer,
{
    match static_server.serve_file("index.html").await {
        Ok(response) => {
            let content = String::from_utf8_lossy(&response.content).to_string();
            Ok(Html(content))
        },
        Err(_) => {
            // Return a basic HTML page if index.html doesn't exist
            let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Orchestrator Dashboard</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            text-align: center; 
            margin: 50px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            flex-direction: column;
        }
        .container {
            background: rgba(255, 255, 255, 0.1);
            padding: 40px;
            border-radius: 10px;
            backdrop-filter: blur(10px);
        }
        h1 { font-size: 2.5em; margin-bottom: 20px; }
        p { font-size: 1.2em; opacity: 0.9; }
        .status { 
            margin: 20px 0;
            padding: 10px;
            background: rgba(255, 255, 255, 0.2);
            border-radius: 5px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🎯 Orchestrator Dashboard</h1>
        <p>WebServer is running and ready to serve dashboard content.</p>
        <div class="status">
            <strong>Status:</strong> Active
        </div>
        <p><em>Frontend assets will be served from this endpoint once available.</em></p>
    </div>
    <script>
        // Basic WebSocket connection test
        const ws = new WebSocket('ws://localhost:8080/ws');
        ws.onopen = () => console.log('WebSocket connected');
        ws.onmessage = (event) => console.log('WebSocket message:', event.data);
        ws.onerror = (error) => console.log('WebSocket error:', error);
    </script>
</body>
</html>"#;
            Ok(Html(html.to_string()))
        }
    }
}

/// Serve static files
pub async fn serve_static<S>(
    Path(path): Path<String>,
    State(static_server): State<Arc<S>>,
) -> Result<Response, StatusCode>
where
    S: StaticFileServer,
{
    match static_server.serve_file(&path).await {
        Ok(file_response) => {
            let mut response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, file_response.content_type);
            
            // Add cache control if specified
            if let Some(cache_control) = file_response.cache_control {
                response = response.header(header::CACHE_CONTROL, cache_control);
            }
            
            let body = file_response.content;
            response.body(body.into())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        },
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}