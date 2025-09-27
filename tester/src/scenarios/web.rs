//! Web/HTTP Interface Tests
//!
//! Tests for webserver mode and HTTP interfaces

use crate::{OrchestratorConfig, ServiceConstellation, TracingCollector};
use std::time::Duration;

/// Test webserver mode startup and basic functionality
pub async fn server(
    _collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Server: webserver mode");

    let config = OrchestratorConfig::builder()
        .webserver_mode()
        .webserver_addr("127.0.0.1:6003") // IPC address for orchestrator-webserver communication
        .producer_addr("127.0.0.1:6001")  // IPC address for orchestrator-producer communication
        .build();

    constellation.start_orchestrator(config).await?;

    // Give it a moment to start up
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Test that webserver is responsive on the HTTP port (8080)
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
