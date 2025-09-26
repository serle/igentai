//! Central state management for the WebServer
//!
//! Pure business logic with no I/O dependencies

use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use uuid::Uuid;

use crate::types::{
    ActivityEvent, ActivityType, Alert, AlertLevel, ClientMessage, ClientSession, DashboardData, OptimizationInsight,
    PerformanceMetrics, SystemHealth, TrendDirection, convert_to_websocket_message,
};
use shared::messages::webserver::CompletionReason;
use shared::{OrchestratorUpdate, SystemMetrics};

/// Central WebServer state containing all business logic
pub struct WebServerState {
    /// Active client sessions
    sessions: HashMap<Uuid, ClientSession>,

    /// Current system metrics from orchestrator
    current_metrics: Option<SystemMetrics>,

    /// Historical metrics for trend analysis (limited size)
    metrics_history: VecDeque<TimestampedMetrics>,

    /// Recent activity events
    recent_activities: VecDeque<ActivityEvent>,

    /// Active system alerts
    active_alerts: HashMap<Uuid, Alert>,

    /// Connection state with orchestrator
    orchestrator_connected: bool,

    /// Server startup time
    start_time: Instant,

    /// Maximum history size to prevent memory growth
    max_history_size: usize,

    /// Currently active generation
    active_generation: Option<ActiveGeneration>,
}

/// Metrics with timestamp for historical tracking
#[derive(Debug, Clone)]
pub struct TimestampedMetrics {
    pub metrics: SystemMetrics,
    pub timestamp: DateTime<Utc>,
}

/// Active generation information
#[derive(Debug, Clone)]
pub struct ActiveGeneration {
    pub topic: String,
    pub start_time: Instant,
    pub active_producers: u32,
    pub total_unique_attributes: usize,
}

impl WebServerState {
    /// Create new WebServer state
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            current_metrics: None,
            metrics_history: VecDeque::new(),
            recent_activities: VecDeque::new(),
            active_alerts: HashMap::new(),
            orchestrator_connected: false,
            start_time: Instant::now(),
            max_history_size: 100, // Keep last 100 metric updates
            active_generation: None,
        }
    }

    /// Process orchestrator update and generate client messages
    pub fn process_orchestrator_update(&mut self, update: OrchestratorUpdate) -> Vec<ClientMessage> {
        let mut client_messages = Vec::new();

        match update {
            OrchestratorUpdate::NewAttributes { ref attributes, .. } => {
                // Add activity event
                self.add_activity(ActivityEvent {
                    event_type: ActivityType::AttributesGenerated,
                    message: format!("Generated {} new attributes", attributes.len()),
                    timestamp: Utc::now().timestamp() as u64,
                    metadata: Some(serde_json::json!({ "count": attributes.len() })),
                });

                // Convert to WebSocket message using consistent pattern
                client_messages.extend(convert_to_websocket_message(update.clone()));
            }

            OrchestratorUpdate::StatisticsUpdate {
                timestamp,
                active_producers,
                current_topic,
                total_unique_attributes,
                metrics,
            } => {
                // Update current metrics
                self.current_metrics = Some(metrics.clone());

                // Add to history
                self.add_metrics_to_history(metrics.clone());

                // Update active generation state
                if let Some(topic) = &current_topic {
                    self.active_generation = Some(ActiveGeneration {
                        topic: topic.clone(),
                        start_time: self.start_time,
                        active_producers,
                        total_unique_attributes,
                    });
                }

                // Convert to client message
                client_messages.extend(convert_to_websocket_message(OrchestratorUpdate::StatisticsUpdate {
                    timestamp,
                    active_producers,
                    current_topic,
                    total_unique_attributes,
                    metrics,
                }));
            }

            OrchestratorUpdate::GenerationComplete {
                timestamp,
                topic,
                total_iterations,
                final_unique_count,
                completion_reason,
            } => {
                // Clear active generation
                self.active_generation = None;

                // Add activity event
                let reason_str = match &completion_reason {
                    CompletionReason::IterationLimitReached => "iteration limit reached",
                    CompletionReason::ManualStop => "manual stop",
                    CompletionReason::AllProducersFailed => "all producers failed",
                    CompletionReason::SystemError { error } => &format!("system error: {}", error),
                };

                self.add_activity(ActivityEvent {
                    event_type: ActivityType::GenerationStopped,
                    message: format!("Generation '{}' completed: {}", topic, reason_str),
                    timestamp,
                    metadata: Some(serde_json::json!({
                        "topic": topic,
                        "iterations": total_iterations,
                        "unique_count": final_unique_count,
                        "reason": reason_str
                    })),
                });

                // Convert to client message
                client_messages.extend(convert_to_websocket_message(OrchestratorUpdate::GenerationComplete {
                    timestamp,
                    topic,
                    total_iterations,
                    final_unique_count,
                    completion_reason,
                }));
            }

            OrchestratorUpdate::ErrorNotification(ref error_msg) => {
                // Create alert for internal tracking
                let alert_id = Uuid::new_v4();
                let alert = Alert {
                    id: alert_id,
                    level: AlertLevel::Error,
                    title: "System Error".to_string(),
                    message: error_msg.clone(),
                    timestamp: Utc::now().timestamp() as u64,
                    dismissible: true,
                    acknowledged: false,
                };

                self.active_alerts.insert(alert_id, alert.clone());

                // Convert to WebSocket message using consistent pattern
                client_messages.extend(convert_to_websocket_message(update.clone()));
            }

            _ => {
                // Handle other orchestrator updates as needed
            }
        }

        client_messages
    }

    /// Add client session
    pub fn add_client_session(&mut self, session: ClientSession) -> Uuid {
        let session_id = session.id;
        self.sessions.insert(session_id, session);
        session_id
    }

    /// Remove client session
    pub fn remove_client_session(&mut self, session_id: Uuid) {
        self.sessions.remove(&session_id);
    }

    /// Set orchestrator connection status
    pub fn set_orchestrator_connected(&mut self, connected: bool) {
        if self.orchestrator_connected != connected {
            self.orchestrator_connected = connected;

            let event = if connected {
                ActivityEvent {
                    event_type: ActivityType::GenerationStarted, // Reusing enum value
                    message: "Connected to orchestrator".to_string(),
                    timestamp: Utc::now().timestamp() as u64,
                    metadata: None,
                }
            } else {
                ActivityEvent {
                    event_type: ActivityType::GenerationStopped, // Reusing enum value
                    message: "Disconnected from orchestrator".to_string(),
                    timestamp: Utc::now().timestamp() as u64,
                    metadata: None,
                }
            };

            self.add_activity(event);
        }
    }

    /// Get current dashboard data
    pub fn generate_dashboard_data(&self) -> DashboardData {
        let performance_metrics = if let Some(metrics) = &self.current_metrics {
            PerformanceMetrics {
                uam_current: metrics.uam,
                uam_trend: self.calculate_uam_trend(),
                cost_per_minute: metrics.cost_per_minute,
                cost_efficiency: metrics.unique_per_dollar,
                token_efficiency: metrics.unique_per_1k_tokens,
                total_unique_count: self.calculate_total_unique_count(),
                uptime_seconds: self.start_time.elapsed().as_secs(),
            }
        } else {
            PerformanceMetrics {
                uam_current: 0.0,
                uam_trend: TrendDirection::Stable,
                cost_per_minute: 0.0,
                cost_efficiency: 0.0,
                token_efficiency: 0.0,
                total_unique_count: 0,
                uptime_seconds: self.start_time.elapsed().as_secs(),
            }
        };

        DashboardData {
            performance_metrics,
            provider_breakdown: self.get_provider_breakdown(),
            producer_breakdown: self.get_producer_breakdown(),
            recent_activity: self.recent_activities.iter().cloned().collect(),
            system_alerts: self.active_alerts.values().cloned().collect(),
        }
    }

    /// Get active session count
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Check if orchestrator is connected
    pub fn is_orchestrator_connected(&self) -> bool {
        self.orchestrator_connected
    }

    /// Get current metrics
    pub fn current_metrics(&self) -> Option<&SystemMetrics> {
        self.current_metrics.as_ref()
    }

    // Private helper methods

    /// Add metrics to historical tracking
    fn add_metrics_to_history(&mut self, metrics: SystemMetrics) {
        let timestamped = TimestampedMetrics {
            metrics,
            timestamp: Utc::now(),
        };

        self.metrics_history.push_back(timestamped);

        // Keep history within bounds
        while self.metrics_history.len() > self.max_history_size {
            self.metrics_history.pop_front();
        }
    }

    /// Add activity event
    fn add_activity(&mut self, activity: ActivityEvent) {
        self.recent_activities.push_back(activity);

        // Keep only recent activities (last 50)
        while self.recent_activities.len() > 50 {
            self.recent_activities.pop_front();
        }
    }

    /// Calculate UAM trend from history
    fn calculate_uam_trend(&self) -> TrendDirection {
        if self.metrics_history.len() < 2 {
            return TrendDirection::Stable;
        }

        let recent = &self.metrics_history[self.metrics_history.len() - 1];
        let previous = &self.metrics_history[self.metrics_history.len() - 2];

        let diff = recent.metrics.uam - previous.metrics.uam;

        if diff > 0.1 {
            TrendDirection::Up
        } else if diff < -0.1 {
            TrendDirection::Down
        } else {
            TrendDirection::Stable
        }
    }

    /// Calculate system health
    fn calculate_system_health(&self) -> SystemHealth {
        if !self.orchestrator_connected {
            return SystemHealth::Unhealthy;
        }

        if let Some(metrics) = &self.current_metrics {
            if metrics.active_producers == 0 {
                SystemHealth::Degraded
            } else if metrics.uam > 0.0 {
                SystemHealth::Healthy
            } else {
                SystemHealth::Degraded
            }
        } else {
            SystemHealth::Unknown
        }
    }

    /// Calculate total unique count from metrics
    fn calculate_total_unique_count(&self) -> u64 {
        // This would be accumulated from metrics history or stored separately
        // For now, return 0 as placeholder
        0
    }

    /// Get provider breakdown for dashboard
    fn get_provider_breakdown(&self) -> HashMap<shared::ProviderId, crate::types::ProviderStats> {
        if let Some(metrics) = &self.current_metrics {
            metrics
                .by_provider
                .iter()
                .map(|(provider_id, provider_metrics)| {
                    (
                        *provider_id,
                        crate::types::ProviderStats {
                            uam: provider_metrics.uam,
                            cost_per_minute: provider_metrics.cost_per_minute,
                            success_rate: provider_metrics.success_rate,
                            avg_response_time_ms: provider_metrics.avg_response_time_ms,
                            health_score: provider_metrics.success_rate, // Simple health score
                            status: provider_metrics.status,
                        },
                    )
                })
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Get producer breakdown for dashboard
    fn get_producer_breakdown(&self) -> HashMap<shared::ProcessId, crate::types::ProducerStats> {
        if let Some(metrics) = &self.current_metrics {
            metrics
                .by_producer
                .iter()
                .map(|(_producer_id, producer_metrics)| {
                    (
                        shared::ProcessId::Producer(1), // Default producer ID since ProcessId doesn't have from_string
                        crate::types::ProducerStats {
                            uam: producer_metrics.uam,
                            cost_per_minute: producer_metrics.cost_per_minute,
                            uniqueness_ratio: producer_metrics.uniqueness_ratio,
                            status: producer_metrics.status,
                            last_activity: producer_metrics.last_activity,
                        },
                    )
                })
                .collect()
        } else {
            HashMap::new()
        }
    }

    /// Generate optimization insights (placeholder)
    fn generate_optimization_insights(&self) -> Vec<OptimizationInsight> {
        // This would use the analytics engine in a real implementation
        vec![]
    }
}

impl Default for WebServerState {
    fn default() -> Self {
        Self::new()
    }
}
