//! Webserver state management
//!
//! This module contains the core state structures used throughout the webserver.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use shared::SystemMetrics;
use crate::types::{BrowserMessage, AttributeUpdate, SystemHealth, ClientId, ClientConnection};

/// Core webserver state
#[derive(Debug)]
pub struct WebServerState {
    // Server configuration
    pub bind_address: SocketAddr,
    
    // Browser connection management  
    pub client_connections: Arc<RwLock<HashMap<ClientId, Arc<ClientConnection>>>>,
    pub client_broadcast: broadcast::Sender<BrowserMessage>,
    
    // Orchestrator communication
    pub orchestrator_address: SocketAddr,
    pub orchestrator_reconnect_interval: Duration,
    pub orchestrator_connected: Arc<AtomicBool>,
    
    // System state mirror (read-only view from orchestrator)
    pub current_metrics: Arc<RwLock<SystemMetrics>>,
    pub recent_attributes: Arc<RwLock<VecDeque<AttributeUpdate>>>,
    pub system_health: Arc<RwLock<SystemHealth>>,
    
    // Server state
    pub is_running: Arc<AtomicBool>,
    pub connection_count: Arc<AtomicU32>,
    pub last_orchestrator_update: Arc<RwLock<Instant>>,
    pub server_start_time: Instant,
}

impl WebServerState {
    /// Create a new webserver state
    pub fn new(bind_address: SocketAddr, orchestrator_address: SocketAddr) -> Self {
        let (client_broadcast, _) = broadcast::channel(1000);
        let now = Instant::now();
        
        Self {
            bind_address,
            client_connections: Arc::new(RwLock::new(HashMap::new())),
            client_broadcast,
            orchestrator_address,
            orchestrator_reconnect_interval: Duration::from_secs(5),
            orchestrator_connected: Arc::new(AtomicBool::new(false)),
            current_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            recent_attributes: Arc::new(RwLock::new(VecDeque::new())),
            system_health: Arc::new(RwLock::new(SystemHealth {
                orchestrator_connected: false,
                active_clients: 0,
                last_update: None,
                server_uptime_seconds: 0,
            })),
            is_running: Arc::new(AtomicBool::new(true)),
            connection_count: Arc::new(AtomicU32::new(0)),
            last_orchestrator_update: Arc::new(RwLock::new(now)),
            server_start_time: now,
        }
    }
    
    /// Check if the server is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
    
    /// Set running state
    pub fn set_running(&self, running: bool) {
        self.is_running.store(running, Ordering::Relaxed);
    }
    
    /// Get server uptime in seconds
    pub fn get_uptime_seconds(&self) -> u64 {
        self.server_start_time.elapsed().as_secs()
    }
    
    /// Get client connection count
    pub fn get_connection_count(&self) -> u32 {
        self.connection_count.load(Ordering::Relaxed)
    }
    
    /// Increment connection count
    pub fn increment_connection_count(&self) -> u32 {
        self.connection_count.fetch_add(1, Ordering::Relaxed) + 1
    }
    
    /// Decrement connection count  
    pub fn decrement_connection_count(&self) -> u32 {
        self.connection_count.fetch_sub(1, Ordering::Relaxed).saturating_sub(1)
    }
    
    /// Check if orchestrator is connected
    pub fn is_orchestrator_connected(&self) -> bool {
        self.orchestrator_connected.load(Ordering::Relaxed)
    }
    
    /// Set orchestrator connection status
    pub fn set_orchestrator_connected(&self, connected: bool) {
        self.orchestrator_connected.store(connected, Ordering::Relaxed);
    }
}

// Default implementation for SystemMetrics moved to shared crate or implemented there

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_webserver_state_creation() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        
        let state = WebServerState::new(bind_addr, orch_addr);
        
        assert_eq!(state.bind_address, bind_addr);
        assert_eq!(state.orchestrator_address, orch_addr);
        assert!(state.is_running());
        assert_eq!(state.get_connection_count(), 0);
        assert!(!state.is_orchestrator_connected());
    }
    
    #[tokio::test]
    async fn test_connection_count_management() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        
        let state = WebServerState::new(bind_addr, orch_addr);
        
        assert_eq!(state.get_connection_count(), 0);
        
        let count1 = state.increment_connection_count();
        assert_eq!(count1, 1);
        assert_eq!(state.get_connection_count(), 1);
        
        let count2 = state.increment_connection_count();
        assert_eq!(count2, 2);
        
        let count3 = state.decrement_connection_count();
        assert_eq!(count3, 1);
        assert_eq!(state.get_connection_count(), 1);
    }
    
    #[tokio::test]
    async fn test_orchestrator_connection_status() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
        let orch_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        
        let state = WebServerState::new(bind_addr, orch_addr);
        
        assert!(!state.is_orchestrator_connected());
        
        state.set_orchestrator_connected(true);
        assert!(state.is_orchestrator_connected());
        
        state.set_orchestrator_connected(false);
        assert!(!state.is_orchestrator_connected());
    }
}