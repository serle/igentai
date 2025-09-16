//! Service Constellation Management
//!
//! Manages orchestrator processes in CLI or webserver mode.
//! For individual binary testing, use the specific binary test suites instead.

use crate::config::OrchestratorConfig;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

pub struct ServiceConstellation {
    trace_endpoint: String,
    orchestrator: Option<Child>,
}

impl ServiceConstellation {
    pub fn new(trace_endpoint: String) -> Self {
        Self {
            trace_endpoint,
            orchestrator: None,
        }
    }

    /// Start orchestrator with the specified configuration
    pub async fn start_orchestrator(&mut self, config: OrchestratorConfig) -> Result<(), Box<dyn std::error::Error>> {
        let mode_str = match config.mode {
            crate::config::OrchestratorMode::Cli => "CLI",
            crate::config::OrchestratorMode::WebServer => "WebServer",
        };

        tracing::info!("🚀 Starting orchestrator in {} mode", mode_str);

        if !config.is_valid() {
            return Err(format!("Invalid configuration for {} mode", mode_str).into());
        }

        // Build config with trace endpoint
        let mut final_config = config;
        if final_config.trace_endpoint.is_none() {
            final_config.trace_endpoint = Some(self.trace_endpoint.clone());
        }

        let mut cmd = Command::new("./target/release/orchestrator");
        cmd.args(final_config.to_args());

        let child = cmd.spawn()?;
        self.orchestrator = Some(child);

        // Wait for orchestrator to start
        sleep(Duration::from_secs(2)).await;
        tracing::info!("✅ Orchestrator started in {} mode", mode_str);

        Ok(())
    }

    /// Keep running indefinitely (useful for WebServer mode testing)
    pub async fn keep_running(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("🔄 Keeping orchestrator running indefinitely (press Ctrl+C to stop)");

        if let Some(child) = &mut self.orchestrator {
            loop {
                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        tracing::warn!("⚠️ Orchestrator exited unexpectedly with status: {}", exit_status);
                        self.orchestrator = None;
                        return Err("Orchestrator exited unexpectedly".into());
                    }
                    Ok(None) => {
                        // Still running - sleep and check again
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    Err(e) => {
                        tracing::error!("❌ Error checking orchestrator status: {}", e);
                        self.orchestrator = None;
                        return Err(e.into());
                    }
                }
            }
        } else {
            Err("No orchestrator process running".into())
        }
    }

    /// Wait for orchestrator to complete with maximum duration
    pub async fn wait_for_completion(&mut self, max_duration: Duration) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("⏳ Waiting for orchestrator to complete (max: {:?})", max_duration);

        if let Some(child) = &mut self.orchestrator {
            let start_time = std::time::Instant::now();

            while start_time.elapsed() < max_duration {
                match child.try_wait() {
                    Ok(Some(exit_status)) => {
                        tracing::info!("🏁 Orchestrator completed with exit status: {}", exit_status);
                        self.orchestrator = None;
                        return Ok(());
                    }
                    Ok(None) => {
                        // Still running
                        sleep(Duration::from_millis(500)).await;
                    }
                    Err(e) => {
                        tracing::warn!("⚠️ Error checking orchestrator status: {}", e);
                        self.orchestrator = None;
                        return Err(e.into());
                    }
                }
            }

            tracing::warn!("⏰ Orchestrator did not complete within {:?}", max_duration);
            // Don't return error, just continue - timeout is often expected in tests
        } else {
            tracing::warn!("⚠️ No orchestrator process to wait for");
        }

        Ok(())
    }

    /// Check if orchestrator is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(child) = &mut self.orchestrator {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    self.orchestrator = None;
                    false
                }
                Ok(None) => {
                    // Still running
                    true
                }
                Err(_) => {
                    // Error checking - assume not running
                    self.orchestrator = None;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Shutdown orchestrator gracefully
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("🛑 Shutting down service constellation");

        if let Some(mut child) = self.orchestrator.take() {
            tracing::info!("🔄 Terminating orchestrator");

            // Try graceful termination first
            #[cfg(unix)]
            {
                if let Err(e) = self.terminate_gracefully(&mut child) {
                    tracing::warn!("⚠️ Failed to terminate gracefully: {}", e);
                }
            }

            // Wait a bit for graceful shutdown
            sleep(Duration::from_millis(1000)).await;

            // Force kill if still running
            match child.try_wait() {
                Ok(Some(_)) => {
                    tracing::info!("✅ Orchestrator terminated gracefully");
                }
                Ok(None) => {
                    tracing::warn!("🔨 Force killing orchestrator");
                    if let Err(e) = child.kill() {
                        tracing::error!("❌ Failed to kill orchestrator: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("⚠️ Error checking orchestrator status: {}", e);
                }
            }
        }

        tracing::info!("✅ Service constellation shutdown complete");
        Ok(())
    }

    #[cfg(unix)]
    fn terminate_gracefully(&self, child: &mut Child) -> std::io::Result<()> {
        // Send SIGTERM first for graceful shutdown
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        Ok(())
    }
}

impl Drop for ServiceConstellation {
    fn drop(&mut self) {
        // Emergency cleanup - force kill any remaining processes
        if let Some(mut child) = self.orchestrator.take() {
            tracing::warn!("🚨 Emergency cleanup: force killing orchestrator");
            let _ = child.kill();
        }
    }
}
