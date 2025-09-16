//! Topic - Main E2E Testing Interface
//! 
//! Provides a clean, high-level interface for writing E2E test scenarios
//! centered around topic execution and assertion.

use std::time::Duration;
use crate::{
    runtime::{TracingCollector, CollectedEvent},
    testing::{OutputLoader, OutputData, TracingAssertions},
};

/// Main interface for E2E testing scenarios
/// 
/// Represents a topic execution with access to traces and output data
#[derive(Debug)]
pub struct Topic {
    name: String,
    collector: TracingCollector,
    assertions: TracingAssertions,
    trace_events: Vec<CollectedEvent>,
    output_data: Option<OutputData>,
}

impl Topic {
    /// Wait for a topic to complete and return a Topic instance for testing
    /// 
    /// This is the main entry point for E2E testing scenarios
    pub async fn wait_for_topic(
        topic_name: &str,
        collector: TracingCollector,
        timeout: Duration,
    ) -> Option<Self> {
        tracing::info!("🔍 Waiting for topic '{}' to complete", topic_name);
        
        // Wait for topic completion
        if !collector.wait_for_topic_completion(topic_name, timeout).await {
            tracing::error!("❌ Topic '{}' did not complete within {:?}", topic_name, timeout);
            return None;
        }
        
        // Extract trace events for this topic
        let trace_events = collector.get_topic_trace_subset(topic_name);
        
        if trace_events.is_empty() {
            tracing::error!("❌ No trace events found for topic '{}'", topic_name);
            return None;
        }
        
        // Try to load output data
        let output_data = match OutputLoader::load_from_topic(topic_name) {
            Ok(data) => {
                tracing::info!("📄 Loaded output data: {} attributes", data.attribute_count());
                Some(data)
            }
            Err(e) => {
                tracing::warn!("⚠️ Could not load output data: {}", e);
                None
            }
        };
        
        let assertions = TracingAssertions::new(collector.clone());
        
        tracing::info!("✅ Topic '{}' ready for testing ({} trace events)", 
                      topic_name, trace_events.len());
        
        Some(Topic {
            name: topic_name.to_string(),
            collector,
            assertions,
            trace_events,
            output_data,
        })
    }

    /// Wait for a topic indefinitely (useful for WebServer mode testing)
    /// 
    /// This allows manual HTTP testing without timeout constraints
    pub async fn wait_for_topic_indefinitely(
        topic_name: &str,
        collector: TracingCollector,
    ) -> Option<Self> {
        tracing::info!("🔍 Waiting indefinitely for topic '{}' to complete (WebServer mode)", topic_name);
        
        loop {
            // Check every 5 seconds if topic has completed
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            let trace_events = collector.get_topic_trace_subset(topic_name);
            if !trace_events.is_empty() {
                // Check if we have a completion event
                let has_completion = trace_events.iter().any(|event| {
                    event.trace_event.message.contains(&format!("✅ Topic '{}' completed after", topic_name))
                });
                
                if has_completion {
                    tracing::info!("✅ Topic '{}' completed, processing results", topic_name);
                    
                    // Try to load output data
                    let output_data = match OutputLoader::load_from_topic(topic_name) {
                        Ok(data) => {
                            tracing::info!("📄 Loaded output data: {} attributes", data.attribute_count());
                            Some(data)
                        }
                        Err(e) => {
                            tracing::warn!("⚠️ Could not load output data: {}", e);
                            None
                        }
                    };
                    
                    let assertions = TracingAssertions::new(collector.clone());
                    
                    return Some(Topic {
                        name: topic_name.to_string(),
                        collector,
                        assertions,
                        trace_events,
                        output_data,
                    });
                }
            }
        }
    }
    
    /// Get the topic name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the number of trace events for this topic
    pub fn trace_event_count(&self) -> usize {
        self.trace_events.len()
    }
    
    /// Get all trace events for this topic (chronological order)
    pub fn trace_events(&self) -> &[CollectedEvent] {
        &self.trace_events
    }
    
    /// Get trace events from a specific process
    pub fn trace_events_from_process(&self, process: &str) -> Vec<&CollectedEvent> {
        self.trace_events
            .iter()
            .filter(|event| event.trace_event.process == process)
            .collect()
    }
    
    /// Get trace events containing a specific message
    pub fn trace_events_containing(&self, message: &str) -> Vec<&CollectedEvent> {
        self.trace_events
            .iter()
            .filter(|event| event.trace_event.message.contains(message))
            .collect()
    }
    
    /// Get output data if available
    pub fn output_data(&self) -> Option<&OutputData> {
        self.output_data.as_ref()
    }
    
    /// Get number of generated attributes
    pub fn attribute_count(&self) -> usize {
        self.output_data
            .as_ref()
            .map(|data| data.attribute_count())
            .unwrap_or(0)
    }
    
    /// Check if output was generated
    pub fn has_output(&self) -> bool {
        self.output_data.is_some() && self.attribute_count() > 0
    }
    
    // === Trace Assertions ===
    
    /// Assert that a specific process logged a message
    pub async fn assert_process_logged(&self, process: &str, message: &str) -> bool {
        let result = self.assertions.assert_process_logged(process, message, Duration::from_secs(10)).await;
        if result.success {
            tracing::info!("✅ {}", result.message);
            true
        } else {
            tracing::error!("❌ {}", result.message);
            false
        }
    }
    
    /// Assert that no errors occurred during topic execution
    pub async fn assert_no_errors(&self) -> bool {
        let result = self.assertions.assert_no_errors(10).await;
        if result.success {
            tracing::info!("✅ {}", result.message);
            true
        } else {
            tracing::error!("❌ {}", result.message);
            false
        }
    }
    
    /// Assert that the topic completed successfully
    pub async fn assert_completed(&self) -> bool {
        let completion_events = self.trace_events_containing(&format!("✅ Topic '{}' completed after", self.name));
        if !completion_events.is_empty() {
            tracing::info!("✅ Topic '{}' completed successfully", self.name);
            true
        } else {
            tracing::error!("❌ Topic '{}' completion event not found", self.name);
            false
        }
    }
    
    /// Assert that the topic started with expected iteration budget
    pub async fn assert_started_with_budget(&self, expected_budget: Option<usize>) -> bool {
        let budget_str = match expected_budget {
            Some(budget) => format!("with {} iteration budget", budget),
            None => "with no iteration budget".to_string(),
        };
        
        let start_events = self.trace_events_containing(&format!("✅ Topic '{}' started {}", self.name, budget_str));
        if !start_events.is_empty() {
            tracing::info!("✅ Topic '{}' started with correct budget", self.name);
            true
        } else {
            tracing::error!("❌ Topic '{}' start event with expected budget not found", self.name);
            false
        }
    }
    
    // === Output Assertions ===
    
    /// Assert minimum number of attributes generated
    pub fn assert_min_attributes(&self, min_count: usize) -> bool {
        let count = self.attribute_count();
        if count >= min_count {
            tracing::info!("✅ Generated {} attributes (>= {} required)", count, min_count);
            true
        } else {
            tracing::error!("❌ Generated {} attributes (< {} required)", count, min_count);
            false
        }
    }
    
    /// Assert that specific attributes were generated
    pub fn assert_contains_attributes(&self, expected_attributes: &[&str]) -> bool {
        if let Some(output) = &self.output_data {
            let mut missing = Vec::new();
            for attr in expected_attributes {
                if !output.contains_attribute(attr) {
                    missing.push(*attr);
                }
            }
            
            if missing.is_empty() {
                tracing::info!("✅ All expected attributes found");
                true
            } else {
                tracing::error!("❌ Missing attributes: {:?}", missing);
                false
            }
        } else {
            tracing::error!("❌ No output data available");
            false
        }
    }
    
    /// Assert that attributes contain specific patterns
    pub fn assert_attributes_matching(&self, pattern: &str, min_count: usize) -> bool {
        if let Some(output) = &self.output_data {
            let matching = output.find_attributes_containing(pattern);
            if matching.len() >= min_count {
                tracing::info!("✅ Found {} attributes matching '{}' (>= {} required)", 
                              matching.len(), pattern, min_count);
                true
            } else {
                tracing::error!("❌ Found {} attributes matching '{}' (< {} required)", 
                               matching.len(), pattern, min_count);
                false
            }
        } else {
            tracing::error!("❌ No output data available");
            false
        }
    }
    
    // === Utility Methods ===
    
    /// Print summary of topic execution
    pub fn print_summary(&self) {
        tracing::info!("📊 Topic '{}' Summary:", self.name);
        tracing::info!("   • Trace Events: {}", self.trace_event_count());
        tracing::info!("   • Generated Attributes: {}", self.attribute_count());
        tracing::info!("   • Has Output: {}", self.has_output());
        
        if !self.trace_events.is_empty() {
            let first_event = &self.trace_events[0];
            let last_event = &self.trace_events[self.trace_events.len() - 1];
            tracing::info!("   • Duration: {:?}", 
                          last_event.received_at.duration_since(first_event.received_at).unwrap_or_default());
        }
    }
    
    /// Print recent trace events for debugging
    pub fn print_recent_traces(&self, count: usize) {
        tracing::info!("📋 Recent {} trace events for '{}':", count.min(self.trace_events.len()), self.name);
        for (i, event) in self.trace_events.iter().rev().take(count).enumerate() {
            tracing::info!("  {}: [{}] {}: {}", 
                          i + 1,
                          event.trace_event.level,
                          event.trace_event.process,
                          event.trace_event.message);
        }
    }
    
    /// Get sample attributes for inspection
    pub fn sample_attributes(&self, count: usize) -> Vec<String> {
        if let Some(output) = &self.output_data {
            output.first_n_attributes(count).to_vec()
        } else {
            Vec::new()
        }
    }
}