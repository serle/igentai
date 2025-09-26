//! Helper to handle child process stdout/stderr output
//! 
//! This module ensures child process output is properly handled:
//! - If no tracing endpoint: forward to parent's stdout/stderr
//! - If tracing endpoint exists: let child process handle its own logging

use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use shared::{ProcessId, process_debug};

/// Configure stdio for a child process based on tracing configuration
pub fn configure_child_stdio(
    cmd: &mut tokio::process::Command,
    has_trace_endpoint: bool,
    process_name: &str,
) {
    if has_trace_endpoint {
        // With tracing endpoint: let child handle its own output
        // Still pipe to prevent blocking, but we won't forward it
        cmd.stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .stdin(Stdio::null());
        
        process_debug!(
            ProcessId::current(), 
            "📡 {} will send traces to endpoint (output not forwarded to parent)",
            process_name
        );
    } else {
        // Without tracing endpoint: inherit parent's stdout/stderr
        cmd.stdout(Stdio::inherit())
           .stderr(Stdio::inherit())
           .stdin(Stdio::null());
        
        process_debug!(
            ProcessId::current(),
            "🔗 {} output will be forwarded to parent stdout/stderr",
            process_name
        );
    }
}

/// If we piped output (for trace endpoint mode), spawn tasks to consume it
/// This prevents child processes from blocking on full pipes
pub fn spawn_output_consumers(
    mut child: Child,
    _process_name: String,
) -> Child {
    // Only consume if we have piped output
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            // Silently consume to prevent blocking
            while let Ok(Some(_)) = lines.next_line().await {}
        });
    }
    
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            
            // Silently consume to prevent blocking
            while let Ok(Some(_)) = lines.next_line().await {}
        });
    }
    
    child
}