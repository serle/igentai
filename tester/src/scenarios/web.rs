//! Web/HTTP Interface Tests
//! 
//! Tests for webserver mode and HTTP interfaces

use std::time::Duration;
use crate::{
    OrchestratorConfig,
    ServiceConstellation,
    TracingCollector,
};

/// Test webserver mode startup and basic functionality
pub async fn server(
    _collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Server: webserver mode");
    
    let config = OrchestratorConfig::builder()
        .webserver_mode()
        .webserver_addr("127.0.0.1:8080")
        .producer_addr("127.0.0.1:6001")
        .build();
    
    constellation.start_orchestrator(config).await?;
    
    // Give it a moment to start up
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Test that webserver is responsive
    match reqwest::get("http://127.0.0.1:8080/").await {
        Ok(response) => {
            if response.status().is_success() {
                tracing::info!("✅ Server: PASSED - webserver responding");
                Ok(())
            } else {
                Err(format!("Webserver returned status: {}", response.status()).into())
            }
        }
        Err(e) => Err(format!("Failed to connect to webserver: {}", e).into()),
    }
}