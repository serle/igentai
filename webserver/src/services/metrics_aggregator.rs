//! Metrics aggregator service implementation
//!
//! This service aggregates system metrics and dashboard data.

use std::sync::Arc;
use std::time::Instant;
use shared::SystemMetrics;
use crate::traits::MetricsAggregator;
use crate::types::{SystemHealth, AttributeUpdate, DashboardData};
use crate::error::{WebServerResult};
use crate::state::WebServerState;

/// Real metrics aggregator implementation
#[derive(Clone)]
pub struct RealMetricsAggregator {
    state: Arc<WebServerState>,
}

impl RealMetricsAggregator {
    /// Create a new metrics service
    pub fn new(state: Arc<WebServerState>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl MetricsAggregator for RealMetricsAggregator {
    async fn get_current_metrics(&self) -> WebServerResult<SystemMetrics> {
        let metrics = self.state.current_metrics.read().await;
        Ok(metrics.clone())
    }
    
    async fn update_metrics(&self, metrics: SystemMetrics) -> WebServerResult<()> {
        let mut current_metrics = self.state.current_metrics.write().await;
        *current_metrics = metrics;
        
        // Update last orchestrator update time
        let mut last_update = self.state.last_orchestrator_update.write().await;
        *last_update = Instant::now();
        
        println!("📊 Updated system metrics");
        Ok(())
    }
    
    async fn get_system_health(&self) -> WebServerResult<SystemHealth> {
        let health = SystemHealth {
            orchestrator_connected: self.state.is_orchestrator_connected(),
            active_clients: self.state.get_connection_count(),
            last_update: Some(self.state.server_start_time.elapsed().as_secs()),
            server_uptime_seconds: self.state.get_uptime_seconds(),
        };
        Ok(health)
    }
    
    async fn add_attribute_update(&self, update: AttributeUpdate) -> WebServerResult<()> {
        let mut recent_attributes = self.state.recent_attributes.write().await;
        recent_attributes.push_back(update);
        
        // Keep only the last 1000 attributes
        while recent_attributes.len() > 1000 {
            recent_attributes.pop_front();
        }
        
        println!("📝 Added attribute update");
        Ok(())
    }
    
    async fn get_recent_attributes(&self, limit: usize) -> WebServerResult<Vec<AttributeUpdate>> {
        let recent_attributes = self.state.recent_attributes.read().await;
        let mut attributes: Vec<AttributeUpdate> = recent_attributes.iter().cloned().collect();
        attributes.truncate(limit);
        Ok(attributes)
    }
    
    async fn get_dashboard_data(&self) -> WebServerResult<DashboardData> {
        let metrics = self.get_current_metrics().await?;
        let recent_attributes = self.get_recent_attributes(100).await?;
        let system_health = self.get_system_health().await?;
        
        Ok(DashboardData {
            metrics,
            recent_attributes,
            system_health,
        })
    }
    
    async fn clear_metrics(&self) -> WebServerResult<()> {
        let mut current_metrics = self.state.current_metrics.write().await;
        *current_metrics = SystemMetrics::default();
        
        let mut recent_attributes = self.state.recent_attributes.write().await;
        recent_attributes.clear();
        
        println!("🧹 Cleared all metrics");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    
    #[tokio::test]
    async fn test_metrics_service() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let state = Arc::new(WebServerState::new(bind_addr, orch_addr));
        
        let service = RealMetricsAggregator::new(state);
        
        // Test get current metrics (should be default)
        let _metrics = service.get_current_metrics().await.unwrap();
        // SystemMetrics doesn't implement Eq/PartialEq, so just check it exists
        
        // Test update metrics
        let new_metrics = SystemMetrics::default();
        service.update_metrics(new_metrics.clone()).await.unwrap();
        
        let _updated_metrics = service.get_current_metrics().await.unwrap();
        // Just verify the update succeeded without comparing
        
        // Test system health
        let health = service.get_system_health().await.unwrap();
        assert_eq!(health.active_clients, 0);
        assert!(!health.orchestrator_connected);
        // server_uptime_seconds is u64, so it's always >= 0
    }
}