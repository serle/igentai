//! WebServer-specific types and messages

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use shared::messages::webserver::CompletionReason;
use shared::{OrchestratorUpdate, ProcessId, ProviderMetadata, SystemMetrics};

/// WebServer configuration
#[derive(Debug, Clone)]
pub struct WebServerConfig {
    pub process_id: ProcessId,
    pub http_port: u16,
    pub api_port: u16,
    pub static_dir: String,
    pub orchestrator_addr: std::net::SocketAddr,
    pub standalone_mode: bool,
}

impl WebServerConfig {
    pub fn new(http_port: u16, api_port: u16, orchestrator_addr: std::net::SocketAddr) -> Self {
        Self {
            process_id: ProcessId::WebServer, // Will be overridden by singleton
            http_port,
            api_port,
            static_dir: "./static".to_string(),
            orchestrator_addr,
            standalone_mode: false,
        }
    }
}

/// Client session information
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub id: Uuid,
    pub connected_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub client_info: ClientInfo,
}

/// Client connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub user_agent: Option<String>,
    pub ip_address: String,
}

/// WebSocket messages sent to browser clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    /// Dashboard data update
    #[serde(rename = "dashboard_update")]
    DashboardUpdate {
        timestamp: u64,
        metrics: SystemMetrics,
        insights: Vec<OptimizationInsight>,
    },

    /// Real-time attribute feed
    #[serde(rename = "attribute_update")]
    AttributeUpdate {
        attributes: Vec<String>,
        producer_id: ProcessId,
        metadata: ProviderMetadata,
        uniqueness_ratio: f64,
    },

    /// System status change
    #[serde(rename = "status_update")]
    StatusUpdate {
        orchestrator_connected: bool,
        active_producers: u32,
        current_topic: Option<String>,
        system_health: SystemHealth,
    },

    /// Alert notification
    #[serde(rename = "alert")]
    Alert {
        level: AlertLevel,
        title: String,
        message: String,
        timestamp: u64,
        dismissible: bool,
    },

    /// Connection acknowledgment
    #[serde(rename = "connection_ack")]
    ConnectionAck { session_id: Uuid, server_time: u64 },

    /// Real-time statistics update
    #[serde(rename = "statistics_update")]
    StatisticsUpdate {
        timestamp: u64,
        active_producers: u32,
        current_topic: Option<String>,
        total_unique_attributes: usize,
        metrics: SystemMetrics,
    },

    /// Generation completed notification
    #[serde(rename = "generation_complete")]
    GenerationComplete {
        timestamp: u64,
        topic: String,
        total_iterations: u32,
        final_unique_count: usize,
        completion_reason: CompletionReason,
    },
}

/// Browser-to-server WebSocket messages (now only for read operations and pings)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientRequest {
    /// Request dashboard data
    #[serde(rename = "get_dashboard")]
    GetDashboard,

    /// Ping for connection health
    #[serde(rename = "ping")]
    Ping,
}

/// Optimization insights for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationInsight {
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub confidence: f64,
    pub impact_score: f64,
    pub recommendation: Option<String>,
    pub timestamp: u64,
}

/// Types of optimization insights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    #[serde(rename = "cost_efficiency")]
    CostEfficiency,
    #[serde(rename = "performance_optimization")]
    PerformanceOptimization,
    #[serde(rename = "provider_health")]
    ProviderHealth,
    #[serde(rename = "routing_strategy")]
    RoutingStrategy,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertLevel {
    #[serde(rename = "info")]
    Info,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "success")]
    Success,
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemHealth {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "unhealthy")]
    Unhealthy,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Dashboard analytics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub performance_metrics: PerformanceMetrics,
    pub provider_breakdown: HashMap<shared::ProviderId, ProviderStats>,
    pub producer_breakdown: HashMap<ProcessId, ProducerStats>,
    pub recent_activity: Vec<ActivityEvent>,
    pub system_alerts: Vec<Alert>,
}

/// Performance metrics for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub uam_current: f64,
    pub uam_trend: TrendDirection,
    pub cost_per_minute: f64,
    pub cost_efficiency: f64,
    pub token_efficiency: f64,
    pub total_unique_count: u64,
    pub uptime_seconds: u64,
}

/// Provider statistics for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStats {
    pub uam: f64,
    pub cost_per_minute: f64,
    pub success_rate: f64,
    pub avg_response_time_ms: f64,
    pub health_score: f64,
    pub status: shared::ProviderStatus,
}

/// Producer statistics for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProducerStats {
    pub uam: f64,
    pub cost_per_minute: f64,
    pub uniqueness_ratio: f64,
    pub status: shared::ProcessStatus,
    pub last_activity: u64,
}

/// Activity events for recent activity feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub event_type: ActivityType,
    pub message: String,
    pub timestamp: u64,
    pub metadata: Option<serde_json::Value>,
}

/// Types of system activities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityType {
    #[serde(rename = "generation_started")]
    GenerationStarted,
    #[serde(rename = "generation_stopped")]
    GenerationStopped,
    #[serde(rename = "producer_spawned")]
    ProducerSpawned,
    #[serde(rename = "producer_failed")]
    ProducerFailed,
    #[serde(rename = "optimization_applied")]
    OptimizationApplied,
    #[serde(rename = "attributes_generated")]
    AttributesGenerated,
}

/// Trend direction indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
    #[serde(rename = "stable")]
    Stable,
}

/// Alert structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub level: AlertLevel,
    pub title: String,
    pub message: String,
    pub timestamp: u64,
    pub dismissible: bool,
    pub acknowledged: bool,
}

/// Helper function to convert orchestrator updates to client messages
pub fn convert_to_websocket_message(update: OrchestratorUpdate) -> Vec<ClientMessage> {
    match update {
        OrchestratorUpdate::NewAttributes { attributes, provider_metadata } => {
            vec![ClientMessage::AttributeUpdate {
                attributes,
                producer_id: ProcessId::current().clone(), // Would need actual producer ID from update
                metadata: provider_metadata.unwrap_or_else(|| create_default_provider_metadata()),
                uniqueness_ratio: 1.0,                     // Would be calculated
            }]
        }

        OrchestratorUpdate::StatisticsUpdate {
            timestamp,
            active_producers,
            current_topic,
            total_unique_attributes,
            metrics,
        } => {
            vec![ClientMessage::StatisticsUpdate {
                timestamp,
                active_producers,
                current_topic,
                total_unique_attributes,
                metrics,
            }]
        }

        OrchestratorUpdate::GenerationComplete {
            timestamp,
            topic,
            total_iterations,
            final_unique_count,
            completion_reason,
        } => {
            vec![ClientMessage::GenerationComplete {
                timestamp,
                topic,
                total_iterations,
                final_unique_count,
                completion_reason,
            }]
        }

        OrchestratorUpdate::ErrorNotification(error_msg) => {
            vec![ClientMessage::Alert {
                level: AlertLevel::Error,
                title: "System Error".to_string(),
                message: error_msg,
                timestamp: Utc::now().timestamp() as u64,
                dismissible: true,
            }]
        }

        _ => vec![], // Handle other update types as needed
    }
}

/// Helper function to create default provider metadata
pub fn create_default_provider_metadata() -> ProviderMetadata {
    ProviderMetadata {
        provider_id: shared::ProviderId::Random,
        model: "random-model".to_string(),
        response_time_ms: 0,
        tokens: shared::TokenUsage::default(),
        request_timestamp: Utc::now().timestamp() as u64,
    }
}
