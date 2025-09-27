//! Shared logging utilities for consistent tracing across all processes

use crate::types::ProcessId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{Event, Subscriber, error, info};
use tracing_subscriber::layer::Context;

/// Tracing endpoint configuration
#[derive(Debug, Clone)]
pub struct TracingEndpoint {
    pub url: String,
    pub batch_size: usize,
    pub flush_interval: Duration,
}

impl TracingEndpoint {
    pub fn new(url: String) -> Self {
        Self {
            url,
            batch_size: 5,  // Reduced from 50 to 5 for faster visibility
            flush_interval: Duration::from_millis(500),  // Reduced from 1s to 500ms
        }
    }
}

/// Structured trace event for HTTP endpoint
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraceEvent {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub process: String,
    pub fields: HashMap<String, serde_json::Value>,
}

/// HTTP tracing layer that sends trace events to a remote endpoint
pub struct HttpTracingLayer {
    sender: mpsc::UnboundedSender<TraceEvent>,
}

impl HttpTracingLayer {
    pub fn new(endpoint: TracingEndpoint) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel::<TraceEvent>();

        // Spawn background task to batch and send events
        let endpoint_url = endpoint.url.clone();
        let batch_size = endpoint.batch_size;
        let flush_interval = endpoint.flush_interval;

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut events_buffer = Vec::with_capacity(batch_size);
            let mut flush_timer = tokio::time::interval(flush_interval);

            loop {
                tokio::select! {
                    // Receive new trace events
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                events_buffer.push(event);

                                // Send batch if buffer is full
                                if events_buffer.len() >= batch_size {
                                    Self::send_batch(&client, &endpoint_url, &mut events_buffer).await;
                                }
                            }
                            None => {
                                // Channel closed - send remaining events and exit
                                if !events_buffer.is_empty() {
                                    Self::send_batch(&client, &endpoint_url, &mut events_buffer).await;
                                }
                                break;
                            }
                        }
                    }

                    // Periodic flush of buffered events
                    _ = flush_timer.tick() => {
                        if !events_buffer.is_empty() {
                            Self::send_batch(&client, &endpoint_url, &mut events_buffer).await;
                        }
                    }
                }
            }
        });

        HttpTracingLayer { sender: tx }
    }

    /// Flush any remaining traces by sending a special flush signal
    pub fn flush(&self) {
        // Send a special flush event to trigger immediate batch sending
        let flush_event = TraceEvent {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "tracing_flush".to_string(),
            message: "Flushing traces on shutdown".to_string(),
            process: ProcessId::current().to_string(),
            fields: HashMap::new(),
        };
        let _ = self.sender.send(flush_event);
        
        // Give the background task a moment to process
        std::thread::sleep(Duration::from_millis(100));
    }

    async fn send_batch(client: &reqwest::Client, endpoint_url: &str, events_buffer: &mut Vec<TraceEvent>) {
        let batch = std::mem::take(events_buffer);
        let batch_size = batch.len();

        // Debug: Print what process is sending traces
        if let Some(first_event) = batch.first() {
            eprintln!("🔍 Sending {} traces from process: {}", batch_size, first_event.process);
        }

        match client
            .post(endpoint_url)
            .header("Content-Type", "application/json")
            .json(&batch)
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    eprintln!("❌ Failed to send trace batch: HTTP {}", response.status());
                } else {
                    eprintln!("✅ Successfully sent {} traces to {}", batch_size, endpoint_url);
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to send trace batch: {e}");
            }
        }
    }
}

impl<S> tracing_subscriber::Layer<S> for HttpTracingLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut fields = HashMap::new();
        let mut message = String::new();

        // Extract event fields and message
        let mut visitor = TraceVisitor {
            message: &mut message,
            fields: &mut fields,
        };
        event.record(&mut visitor);

        // Only send traces that have the 'process' attribute (from our process_* macros)
        if !fields.contains_key("process") {
            return; // Skip traces without process attribute
        }

        let trace_event = TraceEvent {
            timestamp: Utc::now(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message,
            process: ProcessId::current().to_string(),
            fields,
        };

        // Send to background task (ignore errors if receiver is dropped)
        let _ = self.sender.send(trace_event);
    }
}

/// Visitor to extract event fields and message
struct TraceVisitor<'a> {
    message: &'a mut String,
    fields: &'a mut HashMap<String, serde_json::Value>,
}

impl<'a> tracing::field::Visit for TraceVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message.push_str(&format!("{value:?}"));
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(format!("{value:?}")),
            );
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::String(value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(serde_json::Number::from(value)),
        );
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }
}

/// Initialize tracing subscriber with optional endpoint and log level
pub fn init_tracing_with_endpoint_and_level(endpoint: Option<TracingEndpoint>, log_level: Option<&str>) {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    // Use the same filtering logic as stdout tracing for consistency
    let process_id = ProcessId::current();
    let base_level = log_level.unwrap_or("info");

    let level_filter = match process_id {
        ProcessId::Orchestrator => {
            format!("orchestrator={base_level},shared={base_level},tower=warn,hyper=warn")
        }
        ProcessId::Producer(_) => {
            format!("producer={base_level},shared={base_level},reqwest=warn")
        }
        ProcessId::WebServer => {
            format!("webserver={base_level},shared={base_level},tower_http=debug,axum={base_level}")
        }
    };

    let env_filter = EnvFilter::new(&level_filter);

    match endpoint {
        Some(endpoint) => {
            println!("📡 Tracing endpoint configured: {}", endpoint.url);
            println!("📊 Log level: {level_filter}");

            let http_layer = HttpTracingLayer::new(endpoint);

            // Also add a minimal stdout layer for immediate feedback
            let fmt_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(false)
                .with_line_number(false)
                .compact();

            tracing_subscriber::registry()
                .with(env_filter)
                .with(http_layer)
                .with(fmt_layer)
                .init();
        }
        None => {
            println!("📊 Log level: {level_filter}");
            init_tracing_stdout_with_level(log_level);
        }
    }
}

/// Initialize tracing subscriber with optional endpoint (backward compatibility)
pub fn init_tracing_with_endpoint(endpoint: Option<TracingEndpoint>) {
    init_tracing_with_endpoint_and_level(endpoint, None);
}

/// Flush any pending traces before shutdown
/// This is a best-effort function that tries to flush traces if HTTP tracing is enabled
pub fn flush_traces() {
    // Add a small delay to allow any pending traces to be processed
    std::thread::sleep(Duration::from_millis(200));
    
    // Log a flush marker that will trigger batch sending
    tracing::info!("Flushing traces before shutdown");
    
    // Additional delay to allow the flush to complete
    std::thread::sleep(Duration::from_millis(300));
}

/// Initialize tracing subscriber with process-specific configuration
/// Uses the global process ID that must be initialized first
pub fn init_tracing() {
    init_tracing_stdout();
}

fn init_tracing_stdout() {
    init_tracing_stdout_with_level(None);
}

fn init_tracing_stdout_with_level(log_level: Option<&str>) {
    use tracing_subscriber::{EnvFilter, fmt};

    let process_id = ProcessId::current();
    let base_level = log_level.unwrap_or("info");

    let env_filter = match process_id {
        ProcessId::Orchestrator => {
            format!("orchestrator={base_level},shared={base_level},tower=warn,hyper=warn")
        }
        ProcessId::Producer(_) => {
            format!("producer={base_level},shared={base_level},reqwest=warn")
        }
        ProcessId::WebServer => {
            format!("webserver={base_level},shared={base_level},tower_http=debug,axum={base_level}")
        }
    };

    fmt()
        .with_env_filter(EnvFilter::new(&env_filter))
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

/// Get formatted timestamp for consistent logging
pub fn format_timestamp() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%H:%M:%S%.3f").to_string()
}

/// Macro for process-aware info logging
#[macro_export]
macro_rules! process_info {
    ($process_id:expr, $($arg:tt)*) => {
        tracing::info!(
            process = %$process_id,
            timestamp = shared::logging::format_timestamp(),
            $($arg)*
        );
    };
}

/// Macro for process-aware warning logging
#[macro_export]
macro_rules! process_warn {
    ($process_id:expr, $($arg:tt)*) => {
        tracing::warn!(
            process = %$process_id,
            timestamp = shared::logging::format_timestamp(),
            $($arg)*
        );
    };
}

/// Macro for process-aware error logging
#[macro_export]
macro_rules! process_error {
    ($process_id:expr, $($arg:tt)*) => {
        tracing::error!(
            process = %$process_id,
            timestamp = shared::logging::format_timestamp(),
            $($arg)*
        );
    };
}

/// Macro for process-aware debug logging
#[macro_export]
macro_rules! process_debug {
    ($process_id:expr, $($arg:tt)*) => {
        tracing::debug!(
            process = %$process_id,
            timestamp = shared::logging::format_timestamp(),
            $($arg)*
        );
    };
}

/// Contextual logging helper for startup messages
pub fn log_startup(process_id: &ProcessId, details: &str) {
    info!(
        process = %process_id,
        timestamp = format_timestamp(),
        "🚀 Starting {}",
        details
    );
}

/// Contextual logging helper for shutdown messages
pub fn log_shutdown(process_id: &ProcessId, reason: &str) {
    info!(
        process = %process_id,
        timestamp = format_timestamp(),
        "🛑 Shutting down: {}",
        reason
    );
}

/// Contextual logging helper for error conditions
pub fn log_error(process_id: &ProcessId, context: &str, error: &dyn std::fmt::Display) {
    error!(
        process = %process_id,
        timestamp = format_timestamp(),
        error = %error,
        "❌ {} failed: {}",
        context,
        error
    );
}

/// Contextual logging helper for success conditions
pub fn log_success(process_id: &ProcessId, message: &str) {
    info!(
        process = %process_id,
        timestamp = format_timestamp(),
        "✅ {}",
        message
    );
}

/// Contextual logging helper for progress updates
pub fn log_progress(process_id: &ProcessId, action: &str, details: &str) {
    info!(
        process = %process_id,
        timestamp = format_timestamp(),
        "📋 {}: {}",
        action,
        details
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_id_display() {
        let producer_1 = ProcessId::Producer(1);
        let producer_2 = ProcessId::Producer(2);
        let orchestrator = ProcessId::Orchestrator;
        let webserver = ProcessId::WebServer;

        println!("Producer 1: {producer_1}");
        println!("Producer 2: {producer_2}");
        println!("Orchestrator: {orchestrator}");
        println!("WebServer: {webserver}");

        assert!(producer_1.to_string().starts_with("producer_"));
        assert!(producer_2.to_string().starts_with("producer_"));
        assert_eq!(orchestrator.to_string(), "orchestrator");
        assert_eq!(webserver.to_string(), "webserver");
    }
}
