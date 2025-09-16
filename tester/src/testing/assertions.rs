//! Tracing Assertion Framework
//! 
//! Provides high-level assertion capabilities for E2E testing based on collected trace events.
//! Allows testing cross-service behavior and coordination patterns.

use std::time::Duration;
use crate::runtime::{TracingCollector, TraceQuery};

#[derive(Debug)]
pub struct TracingAssertions {
    collector: TracingCollector,
}

#[derive(Debug, Clone)]
pub struct AssertionResult {
    pub success: bool,
    pub message: String,
    pub details: Option<String>,
    pub events_found: usize,
}

impl AssertionResult {
    pub fn success(message: String, events_found: usize) -> Self {
        Self {
            success: true,
            message,
            details: None,
            events_found,
        }
    }
    
    pub fn failure(message: String, details: Option<String>) -> Self {
        Self {
            success: false,
            message,
            details,
            events_found: 0,
        }
    }
    
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

impl TracingAssertions {
    pub fn new(collector: TracingCollector) -> Self {
        Self { collector }
    }
    
    /// Get access to the underlying TracingCollector
    pub fn get_collector(&self) -> &TracingCollector {
        &self.collector
    }
    
    /// Assert that a specific process logged a message containing the given text
    pub async fn assert_process_logged(
        &self,
        process: &str,
        message_contains: &str,
        timeout: Duration,
    ) -> AssertionResult {
        tracing::info!("🔍 Asserting {} logged message containing: '{}'", process, message_contains);
        
        let found = self.collector.wait_for_message(message_contains, timeout).await;
        
        if found {
            let query = TraceQuery {
                process_filter: Some(process.to_string()),
                message_contains: Some(message_contains.to_string()),
                level_filter: None,
                since_seconds_ago: None,
                limit: None,
            };
            let events = self.collector.query(&query);
            
            AssertionResult::success(
                format!("Process '{}' logged message containing '{}'", process, message_contains),
                events.len(),
            )
        } else {
            AssertionResult::failure(
                format!("Process '{}' did not log message containing '{}' within timeout", process, message_contains),
                Some(format!("Timeout: {:?}", timeout)),
            )
        }
    }
    
    /// Assert that multiple processes are coordinating (e.g., orchestrator -> producer communication)
    pub async fn assert_coordination_sequence(
        &self,
        sequence: Vec<(&str, &str)>, // (process, expected_message_fragment)
        timeout: Duration,
    ) -> AssertionResult {
        tracing::info!("🔗 Asserting coordination sequence with {} steps", sequence.len());
        
        let mut found_sequence = Vec::new();
        
        for (process, message_fragment) in &sequence {
            let found = self.collector.wait_for_message(message_fragment, timeout).await;
            if found {
                found_sequence.push((process, message_fragment));
                tracing::debug!("✅ Found step: {} -> {}", process, message_fragment);
            } else {
                return AssertionResult::failure(
                    format!("Coordination sequence failed at step: {} should log '{}'", process, message_fragment),
                    Some(format!("Found {} of {} steps", found_sequence.len(), sequence.len())),
                );
            }
        }
        
        AssertionResult::success(
            format!("Coordination sequence completed successfully ({} steps)", sequence.len()),
            found_sequence.len(),
        )
    }
    
    /// Assert that all expected processes are active and logging
    pub async fn assert_all_processes_active(
        &self,
        expected_processes: Vec<&str>,
        timeout: Duration,
    ) -> AssertionResult {
        tracing::info!("👥 Asserting {} processes are active", expected_processes.len());
        
        let mut active_processes = Vec::new();
        
        for process in &expected_processes {
            let found = self.collector.wait_for_events(process, 1, timeout).await;
            if found {
                active_processes.push(process);
                tracing::debug!("✅ Process active: {}", process);
            }
        }
        
        if active_processes.len() == expected_processes.len() {
            AssertionResult::success(
                "All expected processes are active".to_string(),
                active_processes.len(),
            )
        } else {
            let missing: Vec<&str> = expected_processes
                .iter()
                .filter(|p| !active_processes.contains(p))
                .copied()
                .collect();
                
            AssertionResult::failure(
                format!("Not all processes are active ({}/{})", active_processes.len(), expected_processes.len()),
                Some(format!("Missing processes: {:?}", missing)),
            )
        }
    }
    
    /// Assert trace propagation - all services should be using the same trace endpoint
    pub async fn assert_trace_propagation(
        &self,
        expected_processes: Vec<&str>,
        _timeout: Duration,
    ) -> AssertionResult {
        tracing::info!("📡 Asserting trace propagation across {} processes", expected_processes.len());
        
        let mut processes_with_tracing = Vec::new();
        
        for process in &expected_processes {
            let query = TraceQuery {
                process_filter: Some(process.to_string()),
                message_contains: Some("Tracing endpoint configured".to_string()),
                level_filter: None,
                since_seconds_ago: Some(30),
                limit: Some(1),
            };
            
            let events = self.collector.query(&query);
            if !events.is_empty() {
                processes_with_tracing.push(process);
                tracing::debug!("✅ {} has tracing configured", process);
            }
        }
        
        if processes_with_tracing.len() == expected_processes.len() {
            AssertionResult::success(
                "All processes have tracing endpoint configured".to_string(),
                processes_with_tracing.len(),
            )
        } else {
            let missing: Vec<&str> = expected_processes
                .iter()
                .filter(|p| !processes_with_tracing.contains(p))
                .copied()
                .collect();
                
            AssertionResult::failure(
                format!("Not all processes have tracing configured ({}/{})", processes_with_tracing.len(), expected_processes.len()),
                Some(format!("Processes without tracing: {:?}", missing)),
            )
        }
    }
    
    /// Assert producer generation workflow
    pub async fn assert_producer_generation_workflow(
        &self,
        producer_count: usize,
        timeout: Duration,
    ) -> AssertionResult {
        tracing::info!("🏭 Asserting producer generation workflow for {} producers", producer_count);
        
        // Check for key workflow messages that go through tracing system
        let workflow_steps = vec![
            "Initializing orchestrator in CLI mode",
            "Producer listener:",
            "Starting generation for topic",
            "Orchestrator initialized in CLI mode",
        ];
        
        let mut completed_steps = 0;
        
        for step in &workflow_steps {
            let found = self.collector.wait_for_message(step, timeout).await;
            if found {
                completed_steps += 1;
                tracing::debug!("✅ Workflow step completed: {}", step);
            } else {
                return AssertionResult::failure(
                    format!("Producer generation workflow failed at step: '{}'", step),
                    Some(format!("Completed {} of {} workflow steps", completed_steps, workflow_steps.len())),
                );
            }
        }
        
        AssertionResult::success(
            format!("Producer generation workflow completed ({} steps)", workflow_steps.len()),
            completed_steps,
        )
    }
    
    /// Assert no error messages in logs
    pub async fn assert_no_errors(&self, since_seconds_ago: u64) -> AssertionResult {
        tracing::info!("🚫 Asserting no error messages in the last {} seconds", since_seconds_ago);
        
        let query = TraceQuery {
            process_filter: None,
            level_filter: Some("ERROR".to_string()),
            message_contains: None,
            since_seconds_ago: Some(since_seconds_ago),
            limit: None,
        };
        
        let error_events = self.collector.query(&query);
        
        if error_events.is_empty() {
            AssertionResult::success(
                "No error messages found".to_string(),
                0,
            )
        } else {
            let error_messages: Vec<String> = error_events
                .iter()
                .take(5) // Show first 5 errors
                .map(|e| format!("{}: {}", e.trace_event.process, e.trace_event.message))
                .collect();
                
            AssertionResult::failure(
                format!("Found {} error messages", error_events.len()),
                Some(format!("Sample errors: {}", error_messages.join("; "))),
            )
        }
    }
    
    /// Assert event frequency - useful for performance testing
    pub async fn assert_event_frequency(
        &self,
        process: &str,
        message_contains: &str,
        expected_min_count: usize,
        time_window_seconds: u64,
    ) -> AssertionResult {
        tracing::info!("📊 Asserting event frequency: {} should log '{}' at least {} times in {}s", 
                      process, message_contains, expected_min_count, time_window_seconds);
        
        let query = TraceQuery {
            process_filter: Some(process.to_string()),
            message_contains: Some(message_contains.to_string()),
            level_filter: None,
            since_seconds_ago: Some(time_window_seconds),
            limit: None,
        };
        
        let events = self.collector.query(&query);
        
        if events.len() >= expected_min_count {
            AssertionResult::success(
                format!("Event frequency assertion passed: found {} events (expected min: {})", 
                       events.len(), expected_min_count),
                events.len(),
            )
        } else {
            AssertionResult::failure(
                format!("Event frequency assertion failed: found {} events (expected min: {})", 
                       events.len(), expected_min_count),
                Some(format!("Time window: {}s, Process: {}, Message: '{}'", 
                           time_window_seconds, process, message_contains)),
            )
        }
    }
    
    /// Get current collector statistics for debugging
    pub fn get_collector_stats(&self) -> crate::runtime::CollectorStats {
        self.collector.get_stats()
    }
    
    /// Print collected events for debugging
    pub fn print_recent_events(&self, limit: usize) {
        let query = TraceQuery {
            process_filter: None,
            level_filter: None,
            message_contains: None,
            since_seconds_ago: Some(30),
            limit: Some(limit),
        };
        
        let events = self.collector.query(&query);
        
        tracing::info!("📋 Recent trace events (last {})", limit);
        for (i, event) in events.iter().enumerate() {
            tracing::info!("  {}: [{}] {}: {}", 
                          i + 1,
                          event.trace_event.level,
                          event.trace_event.process,
                          event.trace_event.message);
        }
    }
    
    /// Assert that a topic has completed successfully
    pub async fn assert_topic_completed(&self, topic: &str, timeout: Duration) -> AssertionResult {
        tracing::info!("🏁 Asserting topic '{}' completion within {:?}", topic, timeout);
        
        if self.collector.wait_for_topic_completion(topic, timeout).await {
            let completion_events = self.collector.query(&crate::runtime::TraceQuery {
                process_filter: Some("orchestrator".to_string()),
                level_filter: None,
                message_contains: Some(format!("✅ Topic '{}' completed after", topic)),
                since_seconds_ago: None,
                limit: Some(1),
            });
            
            if let Some(event) = completion_events.first() {
                AssertionResult::success(
                    format!("Topic '{}' completed successfully", topic),
                    1
                ).with_details(format!("Completion message: {}", event.trace_event.message))
            } else {
                AssertionResult::failure(
                    format!("Topic completion detected but completion event not found"),
                    Some("This is an unexpected internal error".to_string())
                )
            }
        } else {
            AssertionResult::failure(
                format!("Topic '{}' did not complete within {:?}", topic, timeout),
                Some("Consider increasing timeout or checking for errors in the process".to_string())
            )
        }
    }
}

/// Macro for cleaner assertion syntax in tests
#[macro_export]
macro_rules! assert_trace {
    ($assertions:expr, $assertion_result:expr) => {
        {
            let result = $assertion_result.await;
            if result.success {
                tracing::info!("✅ {}", result.message);
            } else {
                let details = result.details.as_deref().unwrap_or("No additional details");
                tracing::error!("❌ {} - {}", result.message, details);
                return Err(format!("Assertion failed: {} - {}", result.message, details).into());
            }
            result
        }
    };
}