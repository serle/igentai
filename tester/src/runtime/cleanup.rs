//! Process and Port Cleanup Utilities
//!
//! This module provides comprehensive cleanup functionality to ensure clean test environments
//! by killing stale processes and freeing up ports before each test scenario runs.

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Comprehensive cleanup manager for test environments
pub struct CleanupManager {
    /// Ports that should be cleaned up before tests
    target_ports: Vec<u16>,
    /// Process names that should be killed before tests
    target_processes: Vec<String>,
}

impl CleanupManager {
    /// Create a new cleanup manager with default targets
    pub fn new() -> Self {
        Self {
            target_ports: vec![
                6000, 6001, 6002, // Orchestrator and producer ports
                8080, 8081, 8082, // WebServer ports
                9000, 9001, 9002, 9003, 9004, 9005, // Producer command ports (starting from 9000)
                9999, // Tracing collector port
            ],
            target_processes: vec![
                "orchestrator".to_string(),
                "producer".to_string(),
                "webserver".to_string(),
                "tester".to_string(), // Don't kill ourselves, but clean up old instances
            ],
        }
    }

    /// Create a cleanup manager with custom ports and processes
    pub fn with_targets(ports: Vec<u16>, processes: Vec<String>) -> Self {
        Self {
            target_ports: ports,
            target_processes: processes,
        }
    }

    /// Add additional ports to clean up
    pub fn add_ports(&mut self, ports: &[u16]) {
        self.target_ports.extend_from_slice(ports);
    }

    /// Add additional process names to clean up
    pub fn add_processes(&mut self, processes: &[String]) {
        self.target_processes.extend_from_slice(processes);
    }

    /// Perform comprehensive cleanup before starting a test scenario
    pub async fn cleanup_before_test(&self, scenario_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("🧹 Starting cleanup before scenario: {}", scenario_name);

        // Step 1: Kill target processes
        self.kill_target_processes().await?;

        // Step 2: Clear API key environment variables to force .env loading
        self.clear_api_key_environment_variables();

        // Step 3: Free up target ports
        self.free_target_ports().await?;

        // Step 4: Wait for cleanup to settle
        sleep(Duration::from_millis(500)).await;

        // Step 5: Verify cleanup was successful
        self.verify_cleanup().await?;

        info!("✅ Cleanup completed successfully for scenario: {}", scenario_name);
        Ok(())
    }

    /// Kill all processes matching target process names
    async fn kill_target_processes(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔪 Killing target processes: {:?}", self.target_processes);

        for process_name in &self.target_processes {
            // Skip killing ourselves (current tester process)
            if process_name == "tester" && self.is_current_process(process_name) {
                debug!("⏭️ Skipping current tester process");
                continue;
            }

            match self.kill_processes_by_name(process_name).await {
                Ok(count) => {
                    if count > 0 {
                        info!("🔪 Killed {} '{}' processes", count, process_name);
                    } else {
                        debug!("✅ No '{}' processes found to kill", process_name);
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to kill '{}' processes: {}", process_name, e);
                }
            }
        }

        Ok(())
    }

    /// Kill all processes with the given name
    async fn kill_processes_by_name(&self, process_name: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let pids = self.find_processes_by_name(process_name)?;
        let mut killed_count = 0;

        for pid in pids {
            // Skip our own PID
            if pid == std::process::id() as i32 {
                continue;
            }

            match self.kill_process_gracefully(pid).await {
                Ok(()) => {
                    killed_count += 1;
                    debug!("🔪 Killed process {} ({})", pid, process_name);
                }
                Err(e) => {
                    warn!("⚠️ Failed to kill process {} ({}): {}", pid, process_name, e);
                }
            }
        }

        Ok(killed_count)
    }

    /// Find all process IDs matching the given name
    fn find_processes_by_name(&self, process_name: &str) -> Result<Vec<i32>, Box<dyn std::error::Error>> {
        let output = Command::new("pgrep")
            .arg("-f") // Match full command line
            .arg(process_name)
            .output()?;

        if !output.status.success() {
            // pgrep returns non-zero when no processes found - this is normal
            return Ok(vec![]);
        }

        let stdout = String::from_utf8(output.stdout)?;
        let pids: Result<Vec<i32>, _> = stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().parse::<i32>())
            .collect();

        Ok(pids?)
    }

    /// Kill a process gracefully (SIGTERM first, then SIGKILL if needed)
    async fn kill_process_gracefully(&self, pid: i32) -> Result<(), Box<dyn std::error::Error>> {
        let nix_pid = Pid::from_raw(pid);

        // Try SIGTERM first for graceful shutdown
        match signal::kill(nix_pid, Signal::SIGTERM) {
            Ok(()) => {
                debug!("📤 Sent SIGTERM to process {}", pid);
                
                // Wait up to 2 seconds for graceful shutdown
                for _ in 0..20 {
                    if !self.process_exists(pid)? {
                        debug!("✅ Process {} terminated gracefully", pid);
                        return Ok(());
                    }
                    sleep(Duration::from_millis(100)).await;
                }

                // If still running, force kill with SIGKILL
                warn!("🔨 Process {} didn't respond to SIGTERM, using SIGKILL", pid);
                signal::kill(nix_pid, Signal::SIGKILL)?;
                
                // Wait a bit more for SIGKILL to take effect
                sleep(Duration::from_millis(200)).await;
                
                if self.process_exists(pid)? {
                    return Err(format!("Process {} still exists after SIGKILL", pid).into());
                }
                
                debug!("🔨 Process {} force killed", pid);
                Ok(())
            }
            Err(nix::errno::Errno::ESRCH) => {
                // Process doesn't exist - that's fine
                debug!("✅ Process {} already gone", pid);
                Ok(())
            }
            Err(e) => Err(format!("Failed to send signal to process {}: {}", pid, e).into()),
        }
    }

    /// Check if a process exists
    fn process_exists(&self, pid: i32) -> Result<bool, Box<dyn std::error::Error>> {
        match signal::kill(Pid::from_raw(pid), None) {
            Ok(()) => Ok(true),
            Err(nix::errno::Errno::ESRCH) => Ok(false),
            Err(e) => Err(format!("Error checking if process {} exists: {}", pid, e).into()),
        }
    }

    /// Check if the given process name matches the current process
    fn is_current_process(&self, process_name: &str) -> bool {
        // Simple heuristic: if we're looking for "tester" and our binary name contains "tester"
        if process_name == "tester" {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(file_name) = exe_path.file_name() {
                    if let Some(name_str) = file_name.to_str() {
                        return name_str.contains("tester");
                    }
                }
            }
            false
        } else {
            false
        }
    }

    /// Free up all target ports by killing processes using them
    async fn free_target_ports(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("🔓 Freeing up target ports: {:?}", self.target_ports);

        for &port in &self.target_ports {
            match self.free_port(port).await {
                Ok(freed) => {
                    if freed {
                        info!("🔓 Freed port {}", port);
                    } else {
                        debug!("✅ Port {} already free", port);
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to free port {}: {}", port, e);
                }
            }
        }

        Ok(())
    }

    /// Free a specific port by killing the process using it
    async fn free_port(&self, port: u16) -> Result<bool, Box<dyn std::error::Error>> {
        let pids = self.find_processes_using_port(port)?;
        
        if pids.is_empty() {
            return Ok(false); // Port was already free
        }

        for pid in pids {
            // Skip our own PID
            if pid == std::process::id() as i32 {
                continue;
            }

            match self.kill_process_gracefully(pid).await {
                Ok(()) => {
                    debug!("🔓 Killed process {} using port {}", pid, port);
                }
                Err(e) => {
                    warn!("⚠️ Failed to kill process {} using port {}: {}", pid, port, e);
                }
            }
        }

        Ok(true)
    }

    /// Find all process IDs using the given port
    fn find_processes_using_port(&self, port: u16) -> Result<Vec<i32>, Box<dyn std::error::Error>> {
        let output = Command::new("lsof")
            .arg("-ti") // -t for terse output (PIDs only), -i for internet connections
            .arg(format!(":{}", port))
            .output()?;

        if !output.status.success() {
            // lsof returns non-zero when no processes found - this is normal
            return Ok(vec![]);
        }

        let stdout = String::from_utf8(output.stdout)?;
        let pids: Result<Vec<i32>, _> = stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().parse::<i32>())
            .collect();

        Ok(pids?)
    }

    /// Verify that cleanup was successful
    async fn verify_cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("🔍 Verifying cleanup was successful");

        let mut issues = Vec::new();

        // Check that target ports are free
        for &port in &self.target_ports {
            let pids = self.find_processes_using_port(port)?;
            if !pids.is_empty() {
                issues.push(format!("Port {} still in use by processes: {:?}", port, pids));
            }
        }

        // Check that target processes are gone (except current tester)
        for process_name in &self.target_processes {
            if process_name == "tester" && self.is_current_process(process_name) {
                continue; // Skip current tester process
            }

            let pids = self.find_processes_by_name(process_name)?;
            if !pids.is_empty() {
                issues.push(format!("Process '{}' still running: {:?}", process_name, pids));
            }
        }

        if !issues.is_empty() {
            warn!("⚠️ Cleanup verification found issues:");
            for issue in &issues {
                warn!("  - {}", issue);
            }
            // Don't fail the test for verification issues - just warn
        } else {
            debug!("✅ Cleanup verification passed");
        }

        Ok(())
    }

    /// Emergency cleanup - more aggressive cleanup for when normal cleanup fails
    pub async fn emergency_cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        warn!("🚨 Performing emergency cleanup");

        // Kill all processes more aggressively
        for process_name in &self.target_processes {
            if process_name == "tester" && self.is_current_process(process_name) {
                continue;
            }

            // Use pkill with SIGKILL directly
            let _ = Command::new("pkill")
                .arg("-9") // SIGKILL
                .arg("-f") // Match full command line
                .arg(process_name)
                .output();
        }

        // Force free all ports
        for &port in &self.target_ports {
            let _ = Command::new("lsof")
                .arg("-ti")
                .arg(format!(":{}", port))
                .output()
                .and_then(|output| {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if let Ok(pid) = line.trim().parse::<i32>() {
                            if pid != std::process::id() as i32 {
                                let _ = Command::new("kill")
                                    .arg("-9")
                                    .arg(pid.to_string())
                                    .output();
                            }
                        }
                    }
                    Ok(())
                });
        }

        // Wait for emergency cleanup to settle
        sleep(Duration::from_secs(1)).await;

        warn!("🚨 Emergency cleanup completed");
        Ok(())
    }

    /// Clear API key environment variables to force loading from .env file
    /// This ensures tests use the API keys from .env rather than any existing environment variables
    fn clear_api_key_environment_variables(&self) {
        let api_key_vars = [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY", 
            "GEMINI_API_KEY",
            "GOOGLE_API_KEY", // Alternative name for Gemini
            "RANDOM_API_KEY",
        ];

        let mut cleared_count = 0;
        for var_name in &api_key_vars {
            if std::env::var(var_name).is_ok() {
                unsafe {
                    std::env::remove_var(var_name);
                }
                cleared_count += 1;
                debug!("🔧 Cleared environment variable: {}", var_name);
            }
        }

        if cleared_count > 0 {
            info!("🔧 Cleared {} API key environment variables to force .env loading", cleared_count);
        } else {
            debug!("✅ No API key environment variables found to clear");
        }
    }
}

impl Default for CleanupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cleanup_manager_creation() {
        let cleanup = CleanupManager::new();
        assert!(!cleanup.target_ports.is_empty());
        assert!(!cleanup.target_processes.is_empty());
    }

    #[tokio::test]
    async fn test_custom_cleanup_manager() {
        let cleanup = CleanupManager::with_targets(
            vec![8080, 9000],
            vec!["test_process".to_string()]
        );
        assert_eq!(cleanup.target_ports, vec![8080, 9000]);
        assert_eq!(cleanup.target_processes, vec!["test_process"]);
    }

    #[test]
    fn test_process_exists_nonexistent() {
        let cleanup = CleanupManager::new();
        // PID 999999 should not exist
        assert_eq!(cleanup.process_exists(999999).unwrap(), false);
    }

    #[test]
    fn test_find_processes_nonexistent() {
        let cleanup = CleanupManager::new();
        let pids = cleanup.find_processes_by_name("nonexistent_process_12345").unwrap();
        assert!(pids.is_empty());
    }
}
