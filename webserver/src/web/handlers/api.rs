//! REST API handlers
//!
//! HTTP API endpoints for dashboard and control operations

use axum::{extract::State, http::StatusCode, response::Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::traits::{OrchestratorClient, WebSocketManager};

/// Get dashboard data
pub async fn get_dashboard<W>(State(websocket_manager): State<Arc<W>>) -> Result<Json<Value>, StatusCode>
where
    W: WebSocketManager,
{
    let client_count = websocket_manager.client_count().await;

    let response = json!({
        "status": "ok",
        "data": {
            "connected_clients": client_count,
            "server_time": Utc::now().timestamp(),
            "dashboard": {
                "metrics": null,
                "insights": []
            }
        }
    });

    Ok(Json(response))
}

/// Get system status
pub async fn get_status<W>(State(websocket_manager): State<Arc<W>>) -> Result<Json<Value>, StatusCode>
where
    W: WebSocketManager,
{
    let client_count = websocket_manager.client_count().await;

    let response = json!({
        "status": "ok",
        "data": {
            "server_status": "running",
            "connected_clients": client_count,
            "orchestrator_connected": false, // Would get actual status
            "uptime_seconds": 0, // Would calculate actual uptime
            "version": env!("CARGO_PKG_VERSION")
        }
    });

    Ok(Json(response))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartRequest {
    pub topic: String,
    pub producer_count: u32,
    pub iterations: Option<u32>,
    pub routing_strategy: Option<String>,
    pub routing_config: Option<String>,
}

/// Start generation endpoint - /api/start
pub async fn start_generation<O>(
    State(orchestrator_client): State<Arc<Mutex<O>>>,
    Json(request): Json<StartRequest>,
) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
{
    use shared::{GenerationConstraints, OptimizationMode, WebServerRequest};

    let optimization_mode = OptimizationMode::MaximizeEfficiency;
    let constraints = GenerationConstraints {
        max_cost_per_minute: 1.0,
        target_uam: 10.0,
        max_runtime_seconds: None,
    };

    let webserver_request = WebServerRequest::StartGeneration {
        request_id: 1,
        topic: request.topic.clone(),
        producer_count: request.producer_count,
        optimization_mode,
        constraints,
        iterations: request.iterations,
        routing_strategy: request.routing_strategy,
        routing_config: request.routing_config,
    };

    let client = orchestrator_client.lock().await;
    match client.send_request(webserver_request).await {
        Ok(_) => {
            let response = json!({
                "status": "success",
                "message": format!("Generation started for topic '{}' with {} producers", request.topic, request.producer_count)
            });
            Ok(Json(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Stop generation endpoint - /api/stop
pub async fn stop_generation<O>(
    State(orchestrator_client): State<Arc<Mutex<O>>>,
) -> Result<Json<Value>, StatusCode>
where
    O: OrchestratorClient + Send + Sync + 'static,
{
    use shared::WebServerRequest;

    let webserver_request = WebServerRequest::StopGeneration { request_id: 2 };

    let client = orchestrator_client.lock().await;
    match client.send_request(webserver_request).await {
        Ok(_) => {
            let response = json!({
                "status": "success",
                "message": "Generation stopped"
            });
            Ok(Json(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
