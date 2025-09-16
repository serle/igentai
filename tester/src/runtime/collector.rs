//! Event Collector
//!
//! HTTP server that collects trace events from all services in the constellation.
//! Provides query and assertion capabilities for E2E testing.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::{Router, extract::State, http::StatusCode, response::Json, routing::post};
use serde::{Deserialize, Serialize};
use shared::logging::TraceEvent;
use tokio::net::TcpListener;

#[derive(Debug, Clone)]
pub struct TracingCollector {
    events: Arc<Mutex<VecDeque<CollectedEvent>>>,
    server_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedEvent {
    pub trace_event: TraceEvent,
    pub received_at: SystemTime,
    pub batch_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceQuery {
    pub process_filter: Option<String>,
    pub level_filter: Option<String>,
    pub message_contains: Option<String>,
    pub since_seconds_ago: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceQueryResponse {
    pub events: Vec<CollectedEvent>,
    pub total_count: usize,
    pub query_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectorStats {
    pub total_events: usize,
    pub events_by_process: std::collections::HashMap<String, usize>,
    pub events_by_level: std::collections::HashMap<String, usize>,
    pub oldest_event: Option<SystemTime>,
    pub newest_event: Option<SystemTime>,
}

impl TracingCollector {
    pub async fn new(port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let collector = TracingCollector {
            events: events.clone(),
            server_handle: Arc::new(Mutex::new(None)),
        };

        // Start HTTP server with minimal endpoints for testing
        let app = Router::new()
            .route("/traces", post(receive_traces))
            .route("/query", post(query_traces))
            .with_state(events);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        let addr = listener.local_addr()?;

        let server_task = tokio::spawn(async move {
            tracing::info!("📡 Tracing collector listening on {}", addr);
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("Tracing collector server error: {}", e);
            }
        });

        // Store server handle for cleanup
        *collector.server_handle.lock().unwrap() = Some(server_task);

        // Wait a bit for server to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(collector)
    }

    /// Get all events (useful for debugging)
    pub fn get_all_events(&self) -> Vec<CollectedEvent> {
        let events = self.events.lock().unwrap();
        events.iter().cloned().collect()
    }

    /// Get events count
    pub fn event_count(&self) -> usize {
        let events = self.events.lock().unwrap();
        events.len()
    }

    /// Clear all collected events
    pub fn clear(&self) {
        let mut events = self.events.lock().unwrap();
        events.clear();
        tracing::info!("🗑️ Cleared all collected trace events");
    }

    /// Query events with filters
    pub fn query(&self, query: &TraceQuery) -> Vec<CollectedEvent> {
        let start_time = std::time::Instant::now();
        let events = self.events.lock().unwrap();

        let cutoff_time = query
            .since_seconds_ago
            .map(|seconds| SystemTime::now() - Duration::from_secs(seconds));

        let mut filtered: Vec<CollectedEvent> = events
            .iter()
            .filter(|event| {
                // Process filter
                if let Some(ref process_filter) = query.process_filter {
                    if !event.trace_event.process.contains(process_filter) {
                        return false;
                    }
                }

                // Level filter
                if let Some(ref level_filter) = query.level_filter {
                    if event.trace_event.level.to_lowercase() != level_filter.to_lowercase() {
                        return false;
                    }
                }

                // Message content filter
                if let Some(ref message_contains) = query.message_contains {
                    if !event.trace_event.message.contains(message_contains) {
                        return false;
                    }
                }

                // Time filter
                if let Some(cutoff) = cutoff_time {
                    if event.received_at < cutoff {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect();

        // Sort by timestamp (oldest first - chronological order)
        filtered.sort_by(|a, b| a.received_at.cmp(&b.received_at));

        // Apply limit
        if let Some(limit) = query.limit {
            filtered.truncate(limit);
        }

        let query_time = start_time.elapsed();
        tracing::debug!(
            "🔍 Query completed in {:?}, returned {} events",
            query_time,
            filtered.len()
        );

        filtered
    }

    /// Get collector statistics
    pub fn get_stats(&self) -> CollectorStats {
        let events = self.events.lock().unwrap();

        let mut events_by_process = std::collections::HashMap::new();
        let mut events_by_level = std::collections::HashMap::new();
        let mut oldest_event = None;
        let mut newest_event = None;

        for event in events.iter() {
            // Count by process
            *events_by_process.entry(event.trace_event.process.clone()).or_insert(0) += 1;

            // Count by level
            *events_by_level.entry(event.trace_event.level.clone()).or_insert(0) += 1;

            // Track oldest and newest
            if oldest_event.is_none() || event.received_at < oldest_event.unwrap() {
                oldest_event = Some(event.received_at);
            }
            if newest_event.is_none() || event.received_at > newest_event.unwrap() {
                newest_event = Some(event.received_at);
            }
        }

        CollectorStats {
            total_events: events.len(),
            events_by_process,
            events_by_level,
            oldest_event,
            newest_event,
        }
    }

    /// Wait for specific number of events from a process
    pub async fn wait_for_events(&self, process: &str, count: usize, timeout: Duration) -> bool {
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout {
            let query = TraceQuery {
                process_filter: Some(process.to_string()),
                level_filter: None,
                message_contains: None,
                since_seconds_ago: None,
                limit: None,
            };

            let events = self.query(&query);
            if events.len() >= count {
                tracing::info!("✅ Found {} events from {}", events.len(), process);
                return true;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        tracing::warn!("⏰ Timeout waiting for {} events from {}", count, process);
        false
    }

    /// Wait for a specific log message
    pub async fn wait_for_message(&self, message_contains: &str, timeout: Duration) -> bool {
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout {
            let query = TraceQuery {
                process_filter: None,
                level_filter: None,
                message_contains: Some(message_contains.to_string()),
                since_seconds_ago: None,
                limit: Some(1),
            };

            let events = self.query(&query);
            if !events.is_empty() {
                tracing::info!("✅ Found message containing: '{}'", message_contains);
                return true;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        tracing::warn!("⏰ Timeout waiting for message containing: '{}'", message_contains);
        false
    }

    /// Extract topic name from collected traces
    /// Looks for topic start events and extracts the topic name
    pub fn extract_topic(&self) -> Option<String> {
        let events = self.events.lock().unwrap();

        // Look for topic start patterns in chronological order
        for event in events.iter() {
            let message = &event.trace_event.message;

            // Pattern: "✅ Topic 'TOPIC_NAME' started with X iteration budget" or "✅ Topic 'TOPIC_NAME' started with no iteration budget"
            if let Some(start_pos) = message.find("✅ Topic '") {
                let topic_start = start_pos + "✅ Topic '".len();
                if let Some(end_pos) = message[topic_start..].find("' started") {
                    let topic = &message[topic_start..topic_start + end_pos];
                    tracing::info!("📝 Extracted topic from traces: '{}'", topic);
                    return Some(topic.to_string());
                }
            }
        }

        tracing::warn!("⚠️ Could not extract topic from collected traces");
        None
    }

    /// Get all traces from topic start to topic completion
    /// Returns traces in chronological order (oldest first)
    pub fn get_topic_trace_subset(&self, topic: &str) -> Vec<CollectedEvent> {
        let events = self.events.lock().unwrap();
        let mut topic_events = Vec::new();
        let mut capturing = false;
        let mut topic_completed = false;

        for event in events.iter() {
            let message = &event.trace_event.message;

            // Check for topic start patterns
            if !capturing {
                // Pattern: "✅ Topic 'TOPIC_NAME' started with X iteration budget"
                if message.contains(&format!("✅ Topic '{}' started", topic)) {
                    capturing = true;
                    tracing::info!("📍 Found topic start event for '{}'", topic);
                }
            }

            // Collect events while capturing
            if capturing {
                topic_events.push(event.clone());

                // Check for definitive topic completion boundary
                if message.contains(&format!("✅ Topic '{}' completed after", topic)) {
                    tracing::info!("📍 Found topic completion boundary for '{}'", topic);
                    topic_completed = true;
                    break; // Stop collecting after completion event
                }
            }
        }

        if capturing && !topic_completed {
            tracing::warn!("⚠️ Topic '{}' started but no completion boundary found", topic);
        }

        tracing::info!(
            "📊 Extracted {} trace events for topic '{}' (completed: {})",
            topic_events.len(),
            topic,
            topic_completed
        );
        topic_events
    }

    /// Get chronologically ordered events (oldest first)
    pub fn get_chronological_events(&self) -> Vec<CollectedEvent> {
        let events = self.events.lock().unwrap();
        let mut all_events: Vec<CollectedEvent> = events.iter().cloned().collect();

        // Sort by timestamp (oldest first - chronological order)
        all_events.sort_by(|a, b| a.received_at.cmp(&b.received_at));

        all_events
    }

    /// Check if a topic has completed based on trace events
    pub fn is_topic_completed(&self, topic: &str) -> bool {
        let events = self.events.lock().unwrap();

        for event in events.iter() {
            let message = &event.trace_event.message;
            if message.contains(&format!("✅ Topic '{}' completed after", topic)) {
                return true;
            }
        }

        false
    }

    /// Wait for topic completion event
    pub async fn wait_for_topic_completion(&self, topic: &str, timeout: Duration) -> bool {
        let start_time = std::time::Instant::now();

        while start_time.elapsed() < timeout {
            if self.is_topic_completed(topic) {
                tracing::info!("✅ Topic '{}' completion detected", topic);
                return true;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        tracing::warn!("⏰ Timeout waiting for topic '{}' completion", topic);
        false
    }
}

// HTTP handlers

async fn receive_traces(
    State(events): State<Arc<Mutex<VecDeque<CollectedEvent>>>>,
    Json(trace_batch): Json<Vec<TraceEvent>>,
) -> Result<StatusCode, StatusCode> {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let received_at = SystemTime::now();

    let mut events_store = events.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for trace_event in trace_batch {
        let collected_event = CollectedEvent {
            trace_event,
            received_at,
            batch_id: batch_id.clone(),
        };
        events_store.push_back(collected_event);

        // Keep only last 10000 events to prevent memory issues
        if events_store.len() > 10000 {
            events_store.pop_front();
        }
    }

    Ok(StatusCode::OK)
}

async fn query_traces(
    State(events): State<Arc<Mutex<VecDeque<CollectedEvent>>>>,
    Json(query): Json<TraceQuery>,
) -> Result<Json<TraceQueryResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    let events_store = events.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let total_count = events_store.len();

    let cutoff_time = query
        .since_seconds_ago
        .map(|seconds| SystemTime::now() - Duration::from_secs(seconds));

    let mut filtered: Vec<CollectedEvent> = events_store
        .iter()
        .filter(|event| {
            if let Some(ref process_filter) = query.process_filter {
                if !event.trace_event.process.contains(process_filter) {
                    return false;
                }
            }

            if let Some(ref level_filter) = query.level_filter {
                if event.trace_event.level.to_lowercase() != level_filter.to_lowercase() {
                    return false;
                }
            }

            if let Some(ref message_contains) = query.message_contains {
                if !event.trace_event.message.contains(message_contains) {
                    return false;
                }
            }

            if let Some(cutoff) = cutoff_time {
                if event.received_at < cutoff {
                    return false;
                }
            }

            true
        })
        .cloned()
        .collect();

    // Sort by timestamp (oldest first - chronological order)
    filtered.sort_by(|a, b| a.received_at.cmp(&b.received_at));

    if let Some(limit) = query.limit {
        filtered.truncate(limit);
    }

    let query_time_ms = start_time.elapsed().as_millis() as u64;

    Ok(Json(TraceQueryResponse {
        events: filtered,
        total_count,
        query_time_ms,
    }))
}
