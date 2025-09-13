//! Process coordination and health monitoring types
//!
//! Types for managing and monitoring child processes (producers and web server).

use serde::{Serialize, Deserialize};

/// Process handle for coordination between ProcessManager and MessageTransport
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessHandle {
    pub process_id: String,
    pub process_type: ProcessType,
    pub tcp_port: Option<u16>,
    pub websocket_address: Option<String>,
    pub spawn_time: u64,
    pub status: ProcessStatus,
}

/// Process type enumeration
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProcessType {
    Producer,
    WebServer,
}

/// Process status for monitoring
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProcessStatus {
    Starting,
    Running,
    Failed,
    Stopped,
}

/// Process health for child process management (orchestrator → producer/webserver)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessHealth {
    pub process_id: String,
    pub process_type: ProcessType,
    pub status: ProcessStatus,
    pub last_heartbeat: Option<u64>,
    pub restart_count: u32,
    pub error_message: Option<String>,
}

/// Channel health for communication monitoring
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChannelHealth {
    pub process_id: String,
    pub channel_type: ChannelType,
    pub is_connected: bool,
    pub last_message: Option<u64>,
    pub message_count: u64,
    pub error_count: u64,
}

/// Communication channel type
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChannelType {
    ProducerTcp,
    WebServerWebSocket,
    BrowserWebSocket,
}