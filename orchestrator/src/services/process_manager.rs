//! Process management service implementation
//!
//! This module contains the production process manager that spawns, monitors,
//! and manages producer and webserver processes with their communication channels.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;
use tokio::{sync::mpsc, task::JoinHandle};

use shared::{ProducerId, SystemMetrics, ProcessHealth, ProcessType, ProcessStatus};
use crate::error::OrchestratorResult;
use crate::traits::{ProcessManager, ProducerHandle, WebServerHandle, KeyValuePair};

/// Internal runtime tracking for a simulated producer task
struct ProducerRuntime {
    #[allow(dead_code)]
    id: ProducerId,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Internal runtime tracking for the simulated webserver task
struct WebServerRuntime {
    handle: JoinHandle<()>,
}

/// Real process manager using in-memory tasks and channels to simulate processes
/// 
/// This implementation creates tokio tasks that simulate separate processes,
/// providing realistic communication patterns and process lifecycle management.
pub struct RealProcessManager {
    producers: Arc<Mutex<HashMap<ProducerId, ProducerRuntime>>>,
    webserver: Arc<Mutex<Option<WebServerRuntime>>>,
}

impl RealProcessManager {
    /// Create a new process manager service instance
    pub fn new() -> Self {
        Self {
            producers: Arc::new(Mutex::new(HashMap::new())),
            webserver: Arc::new(Mutex::new(None)),
        }
    }

    /// Launch a producer task with communication channel
    /// 
    /// Creates a tokio task that simulates a producer process generating
    /// batches of attributes for a given topic.
    fn launch_producer_task(
        id: ProducerId,
        topic: String,
        api_keys: Vec<KeyValuePair>,
    ) -> (ProducerRuntime, mpsc::Receiver<Vec<String>>) {
        // Channel from producer -> orchestrator/transport
        let (tx, rx) = mpsc::channel::<Vec<String>>(100);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();

        // Clone id for use in the async task
        let id_for_task = id.clone();

        // Simulate that API keys are injected into the producer process environment
        let key_names: Vec<String> = api_keys.iter().map(|kv| kv.key.clone()).collect();
        println!(
            "Spawning producer {} for topic '{}' with {} API keys: [{}]",
            id, topic, api_keys.len(), key_names.join(", ")
        );

        let handle = tokio::spawn(async move {
            // A simple counter to simulate unique batch generation per producer
            let mut counter: u64 = 0;

            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    println!("Producer {} received stop signal", id_for_task);
                    break;
                }

                // Simulate generating a batch of attributes
                let batch = vec![
                    format!("{}_p{}_item_{}", topic, id_for_task, counter),
                    format!("{}_p{}_item_{}", topic, id_for_task, counter + 1),
                    format!("{}_p{}_item_{}", topic, id_for_task, counter + 2),
                ];
                counter = counter.wrapping_add(3);

                if tx.send(batch).await.is_err() {
                    // Receiver dropped: channel closed
                    println!("Producer {} channel closed; stopping", id_for_task);
                    break;
                }

                tokio::time::sleep(Duration::from_millis(250)).await;
            }

            println!("Producer {} exited", id_for_task);
        });

        (
            ProducerRuntime {
                id: id.clone(),
                stop,
                handle,
            },
            rx,
        )
    }

    /// Launch a webserver task with communication channel
    /// 
    /// Creates a tokio task that simulates a webserver process receiving
    /// system metrics updates from the orchestrator.
    fn launch_webserver_task(port: u16) -> (WebServerRuntime, mpsc::Sender<SystemMetrics>) {
        // Orchestrator -> WebServer updates
        let (tx, mut rx) = mpsc::channel::<SystemMetrics>(200);

        let handle = tokio::spawn(async move {
            println!("WebServer task started on port {}", port);
            // Receive updates until channel closes or task is aborted
            while let Some(_metrics) = rx.recv().await {
                // Intentionally avoid relying on Debug for SystemMetrics
                println!("WebServer(port {}): received metrics update", port);
            }
            println!("WebServer task on port {} exiting (channel closed)", port);
        });

        (WebServerRuntime { handle }, tx)
    }
}

#[async_trait::async_trait]
impl ProcessManager for RealProcessManager {
    async fn spawn_producers_with_channels(
        &self,
        count: u32,
        topic: &str,
        api_keys: Vec<KeyValuePair>,
    ) -> OrchestratorResult<Vec<ProducerHandle>> {

        let mut handles = Vec::new();
        let mut guard = self.producers.lock().await;

        for _ in 0..count {
            let id = ProducerId::new();
            let (runtime, rx) = Self::launch_producer_task(
                id.clone(),
                topic.to_string(),
                api_keys.clone(),
            );

            guard.insert(id.clone(), runtime);
            handles.push(ProducerHandle { id, inbound: rx });
        }

        println!(
            "Spawned {} producer(s) with channels for topic '{}'",
            handles.len(),
            topic
        );
        Ok(handles)
    }

    async fn spawn_webserver_with_channel(&self, port: u16) -> OrchestratorResult<WebServerHandle> {
        let (runtime, _tx) = Self::launch_webserver_task(port);
        let mut guard = self.webserver.lock().await;
        *guard = Some(runtime);
        println!("Spawned webserver with channel on port {}", port);
        Ok(WebServerHandle { address: SocketAddr::from(([127, 0, 0, 1], port)) })
    }

    async fn monitor_processes(&self) -> OrchestratorResult<Vec<ProcessHealth>> {
        let producer_guard = self.producers.lock().await;
        let mut reports = Vec::with_capacity(producer_guard.len() + 1);
        
        // Monitor producer processes
        for (id, runtime) in producer_guard.iter() {
            let status = if runtime.handle.is_finished() {
                ProcessStatus::Failed
            } else {
                ProcessStatus::Running
            };
            
            reports.push(ProcessHealth {
                process_id: id.to_string(),
                process_type: ProcessType::Producer,
                status,
                last_heartbeat: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                restart_count: 0,
                error_message: None,
            });
        }
        
        // Monitor webserver process
        let webserver_guard = self.webserver.lock().await;
        if let Some(ref webserver_runtime) = *webserver_guard {
            let status = if webserver_runtime.handle.is_finished() {
                ProcessStatus::Failed
            } else {
                ProcessStatus::Running
            };
            
            reports.push(ProcessHealth {
                process_id: "webserver".to_string(),
                process_type: ProcessType::WebServer,
                status,
                last_heartbeat: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                restart_count: 0,
                error_message: None,
            });
        }
        
        Ok(reports)
    }

    async fn restart_failed_producer(
        &self,
        producer_id: ProducerId,
        topic: &str,
        api_keys: Vec<KeyValuePair>,
    ) -> OrchestratorResult<ProducerHandle> {

        let mut guard = self.producers.lock().await;

        if let Some(old) = guard.remove(&producer_id) {
            // Abort the old task if still running
            old.stop.store(true, Ordering::Relaxed);
            old.handle.abort();
        }

        let (runtime, rx) = Self::launch_producer_task(
            producer_id.clone(),
            topic.to_string(),
            api_keys,
        );
        guard.insert(producer_id.clone(), runtime);

        println!("Restarted producer {}", producer_id);
        Ok(ProducerHandle {
            id: producer_id,
            inbound: rx,
        })
    }

    async fn restart_failed_webserver(&self, port: u16) -> OrchestratorResult<WebServerHandle> {
        let mut guard = self.webserver.lock().await;
        
        // Stop existing failed webserver
        if let Some(old) = guard.take() {
            old.handle.abort();
            println!("Stopped failed webserver");
        }
        
        // Start new webserver task
        let (runtime, _tx) = Self::launch_webserver_task(port);
        *guard = Some(runtime);
        
        println!("Restarted webserver on port {}", port);
        Ok(WebServerHandle { address: SocketAddr::from(([127, 0, 0, 1], port)) })
    }

    async fn stop_producers(&self) -> OrchestratorResult<()> {
        let mut guard = self.producers.lock().await;

        for (_id, runtime) in guard.iter() {
            runtime.stop.store(true, Ordering::Relaxed);
        }

        // Abort tasks to ensure prompt shutdown
        for (id, runtime) in guard.drain() {
            runtime.handle.abort();
            println!("Stopped producer {}", id);
        }

        Ok(())
    }

    async fn stop_webserver(&self) -> OrchestratorResult<()> {
        let mut guard = self.webserver.lock().await;
        if let Some(runtime) = guard.take() {
            runtime.handle.abort();
            println!("Stopped webserver");
        }
        Ok(())
    }
}

