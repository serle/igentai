//! REST API Client for WebServer
//! 
//! Provides HTTP client functionality to interact with the webserver REST API

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// REST API client for communicating with the webserver
#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Serialize, Debug)]
pub struct StartRequest {
    pub topic: String,
    pub producer_count: u32,
    pub iterations: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct StartResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct StopResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct StatusResponse {
    pub server_status: String,
    pub connected_clients: u32,
    pub orchestrator_connected: bool,
}

#[derive(Deserialize, Debug)]
pub struct DashboardResponse {
    // Add fields as needed - the actual structure depends on the webserver implementation
    pub status: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(webserver_addr: &str) -> Self {
        let base_url = if webserver_addr.starts_with("http") {
            webserver_addr.to_string()
        } else {
            format!("http://{}", webserver_addr)
        };
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { base_url, client }
    }
    
    /// Start topic generation via REST API
    pub async fn start_topic(&self, request: StartRequest) -> Result<StartResponse, Box<dyn std::error::Error>> {
        tracing::info!("🚀 Starting topic '{}' via REST API with {} producers", request.topic, request.producer_count);
        
        let url = format!("{}/api/start", self.base_url);
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(format!("API request failed with status: {}", response.status()).into());
        }
        
        let result: StartResponse = response.json().await?;
        tracing::info!("✅ Start API response: {}", result.message);
        Ok(result)
    }
    
    /// Stop topic generation via REST API
    pub async fn stop_topic(&self) -> Result<StopResponse, Box<dyn std::error::Error>> {
        tracing::info!("🛑 Stopping topic generation via REST API");
        
        let url = format!("{}/api/stop", self.base_url);
        let response = self.client
            .post(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(format!("Stop API request failed with status: {}", response.status()).into());
        }
        
        let result: StopResponse = response.json().await?;
        tracing::info!("✅ Stop API response: {}", result.message);
        Ok(result)
    }
    
    /// Get system status via REST API
    pub async fn get_status(&self) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/status", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(format!("Status API request failed with status: {}", response.status()).into());
        }
        
        let result: StatusResponse = response.json().await?;
        Ok(result)
    }
    
    /// Get dashboard data via REST API  
    pub async fn get_dashboard(&self) -> Result<DashboardResponse, Box<dyn std::error::Error>> {
        let url = format!("{}/api/dashboard", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(format!("Dashboard API request failed with status: {}", response.status()).into());
        }
        
        let result: DashboardResponse = response.json().await?;
        Ok(result)
    }
    
    /// Check if webserver is responsive
    pub async fn health_check(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let url = format!("{}/test", self.base_url);
        let response = self.client
            .get(&url)
            .send()
            .await?;
        
        Ok(response.status().is_success())
    }
    
    /// Wait for webserver to be ready
    pub async fn wait_for_ready(&self, timeout: Duration) -> Result<(), Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            if self.health_check().await.unwrap_or(false) {
                tracing::info!("✅ WebServer is ready and responding");
                return Ok(());
            }
            
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        Err("WebServer failed to become ready within timeout".into())
    }
}