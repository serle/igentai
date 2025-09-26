//! Real process management service implementation
//!
//! Manages spawning and monitoring of producer and webserver processes
//! with health checking and graceful shutdown capabilities.

use async_trait::async_trait;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::error::{OrchestratorError, OrchestratorResult};
use crate::traits::{ProcessHealthInfo, ProcessManager, ProcessStatus, ProducerInfo, WebServerInfo};
use shared::{process_debug, process_error, ProviderId};

/// Real process manager implementation
pub struct RealProcessManager {
    /// Active producer processes
    active_producers: tokio::sync::Mutex<HashMap<shared::ProcessId, ProcessHandle>>,

    /// Active webserver process
    active_webserver: tokio::sync::Mutex<Option<ProcessHandle>>,

    /// Base port for assigning to processes
    next_port: Arc<Mutex<u16>>,

    /// Optional tracing endpoint to pass to spawned processes
    trace_endpoint: Option<String>,

    /// Log level to pass to spawned processes
    log_level: String,
}

/// Handle for a managed process
struct ProcessHandle {
    pub child: Child,
    pub info: ProcessInfo,
}

/// Information about a managed process
#[derive(Debug, Clone)]
struct ProcessInfo {
    pub process_id: u32,
    pub listen_address: SocketAddr,
    pub command_address: SocketAddr,
    pub start_time: std::time::Instant,
    pub process_type: ProcessType,
}

#[derive(Debug, Clone)]
enum ProcessType {
    Producer(shared::ProcessId),
    WebServer,
}

impl RealProcessManager {
    /// Create new process manager with default settings
    pub fn new() -> Self {
        Self {
            active_producers: tokio::sync::Mutex::new(HashMap::new()),
            active_webserver: tokio::sync::Mutex::new(None),
            next_port: Arc::new(Mutex::new(9000)), // Start ports from 9000 to avoid conflicts
            trace_endpoint: None,
            log_level: "info".to_string(), // Default log level
        }
    }

    /// Configure tracing endpoint (fluent API)
    pub fn with_trace_endpoint(mut self, trace_endpoint: Option<String>) -> Self {
        self.trace_endpoint = trace_endpoint;
        self
    }

    /// Configure log level (fluent API)
    pub fn with_log_level(mut self, log_level: String) -> Self {
        self.log_level = log_level;
        self
    }

    /// Configure base port (fluent API)
    pub fn with_base_port(mut self, base_port: u16) -> Self {
        self.next_port = Arc::new(Mutex::new(base_port));
        self
    }

    /// Get next available port
    async fn get_next_port(&self) -> u16 {
        let mut port = self.next_port.lock().await;
        let current = *port;
        *port += 1;
        current
    }

    /// Spawn a single producer process
    async fn spawn_single_producer(
        &self,
        producer_id: u32,
        _topic: &str,
        api_keys: &HashMap<ProviderId, String>,
        orchestrator_addr: SocketAddr,
        routing_strategy: Option<&shared::RoutingStrategy>,
    ) -> OrchestratorResult<ProcessHandle> {
        // Determine if we're using test provider (Random only) or env provider
        let use_test_provider = api_keys.len() == 1 && api_keys.contains_key(&ProviderId::Random);

        // Get a unique port for this producer
        let producer_port = self.get_next_port().await;

        // Build command
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--bin")
            .arg("producer")
            .arg("--")
            .arg("--id")
            .arg(producer_id.to_string())
            .arg("--orchestrator-addr")
            .arg(orchestrator_addr.to_string())
            .arg("--listen-port")
            .arg(producer_port.to_string());

        // Add tracing endpoint if configured
        if let Some(ref trace_ep) = self.trace_endpoint {
            cmd.arg("--trace-ep").arg(trace_ep);
        }

        // Add log level
        cmd.arg("--log-level").arg(&self.log_level);

        // Pass structured routing configuration to producer
        if let Some(routing) = routing_strategy {
            let routing_config = match routing {
                shared::RoutingStrategy::Backoff { provider } => {
                    let provider_name = match provider {
                        shared::ProviderId::OpenAI => "openai",
                        shared::ProviderId::Anthropic => "anthropic", 
                        shared::ProviderId::Gemini => "gemini",
                        shared::ProviderId::Random => "random",
                    };
                    format!("strategy:backoff,provider:{}", provider_name)
                },
                shared::RoutingStrategy::RoundRobin { providers } => {
                    let provider_list = providers.iter()
                        .map(|p| match p {
                            shared::ProviderId::OpenAI => "openai",
                            shared::ProviderId::Anthropic => "anthropic",
                            shared::ProviderId::Gemini => "gemini",
                            shared::ProviderId::Random => "random",
                        })
                        .collect::<Vec<_>>()
                        .join("+");
                    format!("strategy:roundrobin,providers:{}", provider_list)
                },
                shared::RoutingStrategy::PriorityOrder { providers } => {
                    let provider_list = providers.iter()
                        .map(|p| match p {
                            shared::ProviderId::OpenAI => "openai",
                            shared::ProviderId::Anthropic => "anthropic",
                            shared::ProviderId::Gemini => "gemini",
                            shared::ProviderId::Random => "random",
                        })
                        .collect::<Vec<_>>()
                        .join("+");
                    format!("strategy:priority,providers:{}", provider_list)
                },
                shared::RoutingStrategy::Weighted { weights } => {
                    let weights_str = weights.iter()
                        .map(|(provider, weight)| {
                            let provider_name = match provider {
                                shared::ProviderId::OpenAI => "openai",
                                shared::ProviderId::Anthropic => "anthropic",
                                shared::ProviderId::Gemini => "gemini",
                                shared::ProviderId::Random => "random",
                            };
                            format!("{}:{}", provider_name, weight)
                        })
                        .collect::<Vec<_>>()
                        .join("+");
                    format!("strategy:weighted,weights:{}", weights_str)
                }
            };
            
            cmd.arg("--routing-config").arg(routing_config);
            
            // Add default model for now (TODO: make this configurable)
            let default_model = match routing {
                shared::RoutingStrategy::Backoff { provider } => match provider {
                    shared::ProviderId::OpenAI => "gpt-4o-mini",
                    shared::ProviderId::Anthropic => "claude-3-sonnet",
                    shared::ProviderId::Gemini => "gemini-pro",
                    shared::ProviderId::Random => "random",
                },
                _ => "gpt-4o-mini", // Default model for multi-provider strategies
            };
            cmd.arg("--model").arg(default_model);
        } else {
            // Fallback: use test provider if only Random keys available
            if use_test_provider {
                cmd.arg("--routing-config").arg("strategy:backoff,provider:random");
                cmd.arg("--model").arg("random");
            } else {
                cmd.arg("--routing-config").arg("strategy:backoff,provider:openai");
                cmd.arg("--model").arg("gpt-4o-mini");
            }
        }

        // Add API keys as environment variables (except for Random provider which doesn't need a key)
        for (provider_id, api_key) in api_keys {
            let env_var = match provider_id {
                ProviderId::OpenAI => "OPENAI_API_KEY",
                ProviderId::Anthropic => "ANTHROPIC_API_KEY",
                ProviderId::Gemini => "GOOGLE_API_KEY",
                ProviderId::Random => continue, // Skip Random provider - it doesn't need env var
            };
            cmd.env(env_var, api_key);
        }

        // Configure stdio
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

        // Spawn process
        let child = cmd
            .spawn()
            .map_err(|e| OrchestratorError::process(format!("Failed to spawn producer: {e}")))?;

        let process_id = child.id().unwrap_or(0);
        let shared_producer_id = shared::ProcessId::Producer(producer_id);

        // Create producer's listen address
        let producer_addr = SocketAddr::from(([127, 0, 0, 1], producer_port));

        let info = ProcessInfo {
            process_id,
            listen_address: producer_addr,  // Producer now listens on its own port
            command_address: producer_addr, // Orchestrator will connect here to send commands
            start_time: std::time::Instant::now(),
            process_type: ProcessType::Producer(shared_producer_id),
        };

        process_debug!(
            shared::ProcessId::current(),
            "🏭 Spawned producer_{} (PID: {}) listening on {} (connects to orchestrator at {})",
            producer_id,
            process_id,
            producer_addr,
            orchestrator_addr
        );

        Ok(ProcessHandle { child, info })
    }

    /// Check if a process is still running
    fn is_process_running(child: &mut Child) -> bool {
        match child.try_wait() {
            Ok(None) => true,     // Still running
            Ok(Some(_)) => false, // Exited
            Err(_) => false,      // Error checking status
        }
    }
}

#[async_trait]
impl ProcessManager for RealProcessManager {
    async fn spawn_producers(
        &self,
        count: u32,
        topic: &str,
        api_keys: HashMap<ProviderId, String>,
        orchestrator_addr: std::net::SocketAddr,
        routing_strategy: Option<shared::RoutingStrategy>,
    ) -> OrchestratorResult<Vec<ProducerInfo>> {
        if api_keys.is_empty() {
            return Err(OrchestratorError::config("No API keys provided for producers"));
        }

        let mut producer_infos = Vec::new();
        let mut new_producers = HashMap::new();

        for i in 0..count {
            // Producer IDs are 1-based (producer_1, producer_2, etc.)
            let producer_id = i + 1;

            match self
                .spawn_single_producer(producer_id, topic, &api_keys, orchestrator_addr, routing_strategy.as_ref())
                .await
            {
                Ok(handle) => {
                    let shared_producer_id = if let ProcessType::Producer(id) = &handle.info.process_type {
                        id.clone()
                    } else {
                        continue;
                    };

                    let info = ProducerInfo {
                        id: shared_producer_id.clone(),
                        process_id: handle.info.process_id,
                        listen_address: handle.info.listen_address, // Producer's own listen address
                        command_address: handle.info.command_address, // Producer's own command address
                    };

                    producer_infos.push(info);
                    new_producers.insert(shared_producer_id, handle);
                }
                Err(e) => {
                    process_error!(
                        shared::ProcessId::current(),
                        "⚠️ Failed to spawn producer {}: {}",
                        i + 1,
                        e
                    );
                    // Continue with other producers
                }
            }
        }

        // Store active producers
        {
            let mut active = self.active_producers.lock().await;
            for (id, handle) in new_producers {
                active.insert(id, handle);
            }
        }

        process_debug!(
            shared::ProcessId::current(),
            "🚀 Spawned {} producers for topic '{}'",
            producer_infos.len(),
            topic
        );
        Ok(producer_infos)
    }

    async fn spawn_webserver(&self, port: u16, orchestrator_addr: SocketAddr) -> OrchestratorResult<WebServerInfo> {
        // Use 6000 range for internal IPC communication (standardized like producers)
        let listen_port = 6003; // Fixed listen port for receiving orchestrator updates (like producers)

        // Determine the working directory (assume we're in project root)
        let current_dir = std::env::current_dir()
            .map_err(|e| OrchestratorError::process(format!("Failed to get current directory: {e}")))?;
        let static_dir = current_dir.join("webserver").join("static");

        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--bin")
            .arg("webserver")
            .arg("--")
            .arg("--port")
            .arg(port.to_string())
            .arg("--listen-port")
            .arg(listen_port.to_string())
            .arg("--orchestrator-addr")
            .arg(orchestrator_addr.to_string())
            .arg("--static-dir")
            .arg(static_dir.to_string_lossy().as_ref());

        // Add tracing endpoint if configured
        if let Some(ref trace_ep) = self.trace_endpoint {
            cmd.arg("--trace-ep").arg(trace_ep);
        }

        // Add log level
        cmd.arg("--log-level").arg(&self.log_level);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| OrchestratorError::process(format!("Failed to spawn webserver: {e}")))?;

        let process_id = child.id().unwrap_or(0);

        let info = ProcessInfo {
            process_id,
            listen_address: SocketAddr::from(([127, 0, 0, 1], port)), // HTTP port for browsers
            command_address: SocketAddr::from(([127, 0, 0, 1], listen_port)), // IPC port for orchestrator updates
            start_time: std::time::Instant::now(),
            process_type: ProcessType::WebServer,
        };

        let webserver_info = WebServerInfo {
            process_id,
            listen_address: info.listen_address, // HTTP port
            api_address: SocketAddr::from(([127, 0, 0, 1], listen_port)), // IPC listen port (standardized like producers)
        };

        // Store active webserver
        {
            let mut active = self.active_webserver.lock().await;
            *active = Some(ProcessHandle { child, info });
        }

        process_debug!(
            shared::ProcessId::current(),
            "🌐 Spawned webserver (PID: {}) HTTP:{} IPC:{}",
            process_id,
            port,
            listen_port
        );
        Ok(webserver_info)
    }

    async fn check_process_health(&self) -> OrchestratorResult<Vec<ProcessHealthInfo>> {
        let mut health_infos = Vec::new();

        // Check producers
        {
            let mut producers = self.active_producers.lock().await;
            let mut failed_producers = Vec::new();

            for (producer_id, handle) in producers.iter_mut() {
                let status = if Self::is_process_running(&mut handle.child) {
                    ProcessStatus::Running
                } else {
                    ProcessStatus::Failed
                };

                if status == ProcessStatus::Failed {
                    failed_producers.push(producer_id.clone());
                }

                health_infos.push(ProcessHealthInfo {
                    process_id: handle.info.process_id,
                    producer_id: Some(producer_id.clone()),
                    status,
                    last_heartbeat: Some(handle.info.start_time),
                    memory_usage_mb: None, // Could be implemented with system calls
                });
            }

            // Remove failed producers
            for producer_id in failed_producers {
                producers.remove(&producer_id);
            }
        }

        // Check webserver
        {
            let mut webserver = self.active_webserver.lock().await;
            let mut should_remove = false;

            if let Some(handle) = webserver.as_mut() {
                let status = if Self::is_process_running(&mut handle.child) {
                    ProcessStatus::Running
                } else {
                    ProcessStatus::Failed
                };

                if status == ProcessStatus::Failed {
                    should_remove = true;
                }

                health_infos.push(ProcessHealthInfo {
                    process_id: handle.info.process_id,
                    producer_id: None,
                    status,
                    last_heartbeat: Some(handle.info.start_time),
                    memory_usage_mb: None,
                });
            }

            if should_remove {
                *webserver = None;
            }
        }

        Ok(health_infos)
    }

    async fn stop_producer(&self, producer_id: shared::ProcessId) -> OrchestratorResult<()> {
        let mut producers = self.active_producers.lock().await;

        if let Some(mut handle) = producers.remove(&producer_id) {
            let _ = handle.child.kill().await;
            let _ = handle.child.wait().await;
            process_debug!(shared::ProcessId::current(), "🛑 Stopped producer {}", producer_id);
        }

        Ok(())
    }

    async fn stop_webserver(&self) -> OrchestratorResult<()> {
        let mut webserver = self.active_webserver.lock().await;

        if let Some(mut handle) = webserver.take() {
            let _ = handle.child.kill().await;
            let _ = handle.child.wait().await;
            process_debug!(shared::ProcessId::current(), "🛑 Stopped webserver");
        }

        Ok(())
    }

    async fn stop_all(&self) -> OrchestratorResult<()> {
        // Stop all producers
        {
            let mut producers = self.active_producers.lock().await;
            for (producer_id, mut handle) in producers.drain() {
                let _ = handle.child.kill().await;
                let _ = handle.child.wait().await;
                process_debug!(shared::ProcessId::current(), "🛑 Stopped producer {}", producer_id);
            }
        }

        // Stop webserver
        self.stop_webserver().await?;

        process_debug!(shared::ProcessId::current(), "🛑 All processes stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_manager_creation() {
        let manager = RealProcessManager::new();

        // Should start with empty state
        assert!(manager.active_producers.lock().await.is_empty());
        assert!(manager.active_webserver.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_port_allocation() {
        let manager = RealProcessManager::new().with_base_port(10000);

        // First port should be the base port
        let port1 = manager.get_next_port().await;
        assert_eq!(port1, 10000);
    }

    #[tokio::test]
    async fn test_empty_api_keys_error() {
        let manager = RealProcessManager::new();
        let empty_keys = HashMap::new();
        let addr = "127.0.0.1:6001".parse().unwrap();

        let result = manager.spawn_producers(1, "test", empty_keys, addr).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No API keys"));
    }

    #[tokio::test]
    async fn test_stop_all() {
        let manager = RealProcessManager::new();

        // Should not fail even with no active processes
        let result = manager.stop_all().await;
        assert!(result.is_ok());
    }
}
