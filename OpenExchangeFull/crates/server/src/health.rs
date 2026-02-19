//! Health check and ping service

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

use crate::error::Result;

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: String,
    pub uptime_seconds: Option<u64>,
    pub connections: Option<Vec<ConnectionStatus>>,
}

/// Connection status to upstream services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub service: String,
    pub address: String,
    pub connected: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Shared state for health checks
///
/// This struct is typically wrapped in `Arc<HealthState>` when used with Axum.
/// The `connections` field uses `RwLock` directly since the outer `Arc` provides
/// the thread-safe sharing.
#[derive(Clone)]
pub struct HealthState {
    pub service_name: String,
    pub start_time: Instant,
    pub connections: Arc<tokio::sync::RwLock<Vec<ConnectionStatus>>>,
}

impl HealthState {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            start_time: Instant::now(),
            connections: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub async fn update_connection(&self, status: ConnectionStatus) {
        let mut connections = self.connections.write().await;
        // Remove existing entry for this service if present
        connections.retain(|c| c.service != status.service);
        connections.push(status);
    }

    pub async fn remove_connection(&self, service_name: &str) {
        let mut connections = self.connections.write().await;
        connections.retain(|c| c.service != service_name);
    }

    pub async fn get_connections(&self) -> Vec<ConnectionStatus> {
        self.connections.read().await.clone()
    }

    pub async fn is_healthy(&self) -> bool {
        self.connections.read().await.iter().all(|c| c.connected)
    }
}

/// Health check handler for HTTP
pub async fn health_handler(State(state): State<Arc<HealthState>>) -> Json<Value> {
    let connections = state.get_connections().await;

    let health = json!({
        "status": "ok",
        "service": state.service_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
        "uptime_seconds": state.uptime_seconds(),
        "connections": connections,
    });

    Json(health)
}

/// Simple health handler without state
pub async fn simple_health_handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

/// Detailed health check with dependencies
pub async fn detailed_health_handler(
    State(state): State<Arc<HealthState>>,
) -> (StatusCode, Json<Value>) {
    let connections = state.get_connections().await;

    // Check if all connections are healthy
    let all_healthy = connections.iter().all(|c| c.connected);
    let status_code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let health = json!({
        "status": if all_healthy { "healthy" } else { "degraded" },
        "service": state.service_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
        "uptime_seconds": state.uptime_seconds(),
        "connections": connections,
        "healthy": all_healthy,
    });

    (status_code, Json(health))
}

/// Create health check router
pub fn health_routes(state: Arc<HealthState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/health/detailed", get(detailed_health_handler))
        .with_state(state)
}

/// HTTP client for health checks
///
/// This is a shared client that should be reused across multiple health check calls.
/// Creating a new client for each request is inefficient.
#[derive(Clone)]
pub struct HealthClient {
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl Default for HealthClient {
    fn default() -> Self {
        Self::new(std::time::Duration::from_secs(5))
    }
}

impl HealthClient {
    /// Create a new health client with the specified timeout
    pub fn new(timeout: std::time::Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self { client, timeout }
    }

    /// Test connectivity to a remote service via HTTP health endpoint
    pub async fn check_http(&self, service_name: &str, address: &str) -> ConnectionStatus {
        let start = Instant::now();
        let health_url = format!("http://{}/health", address);

        match self.client.get(&health_url).send().await {
            Ok(response) => {
                let latency = start.elapsed().as_millis() as u64;
                let connected = response.status().is_success();

                ConnectionStatus {
                    service: service_name.to_string(),
                    address: address.to_string(),
                    connected,
                    latency_ms: Some(latency),
                    error: if connected {
                        None
                    } else {
                        Some(format!("HTTP {}", response.status()))
                    },
                }
            }
            Err(e) => ConnectionStatus {
                service: service_name.to_string(),
                address: address.to_string(),
                connected: false,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: Some(e.to_string()),
            },
        }
    }

    /// Test connectivity using TCP connection (fallback when HTTP is not available)
    pub async fn check_tcp(&self, service_name: &str, address: &str) -> ConnectionStatus {
        let start = Instant::now();

        let result = tokio::time::timeout(
            self.timeout,
            tokio::net::TcpStream::connect(address),
        )
        .await;

        match result {
            Ok(Ok(_stream)) => ConnectionStatus {
                service: service_name.to_string(),
                address: address.to_string(),
                connected: true,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: None,
            },
            Ok(Err(e)) => ConnectionStatus {
                service: service_name.to_string(),
                address: address.to_string(),
                connected: false,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: Some(e.to_string()),
            },
            Err(_) => ConnectionStatus {
                service: service_name.to_string(),
                address: address.to_string(),
                connected: false,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error: Some("Connection timeout".to_string()),
            },
        }
    }
}

/// Test connectivity to a remote service via HTTP health endpoint
///
/// Note: For better performance, use `HealthClient` and reuse it across multiple calls.
pub async fn test_service_connection(service_name: &str, address: &str) -> Result<ConnectionStatus> {
    let client = HealthClient::default();
    Ok(client.check_http(service_name, address).await)
}

/// Test connectivity using TCP connection (fallback when HTTP is not available)
///
/// Note: For better performance, use `HealthClient` and reuse it across multiple calls.
pub async fn test_tcp_connection(service_name: &str, address: &str) -> Result<ConnectionStatus> {
    let client = HealthClient::default();
    Ok(client.check_tcp(service_name, address).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_state() {
        let state = HealthState::new("test-service");

        assert_eq!(state.service_name, "test-service");
        assert!(state.is_healthy().await);

        // Add a healthy connection
        state
            .update_connection(ConnectionStatus {
                service: "upstream".to_string(),
                address: "localhost:8080".to_string(),
                connected: true,
                latency_ms: Some(10),
                error: None,
            })
            .await;

        assert!(state.is_healthy().await);
        assert_eq!(state.get_connections().await.len(), 1);

        // Add an unhealthy connection
        state
            .update_connection(ConnectionStatus {
                service: "database".to_string(),
                address: "localhost:5432".to_string(),
                connected: false,
                latency_ms: None,
                error: Some("Connection refused".to_string()),
            })
            .await;

        assert!(!state.is_healthy().await);
        assert_eq!(state.get_connections().await.len(), 2);

        // Remove the unhealthy connection
        state.remove_connection("database").await;

        assert!(state.is_healthy().await);
        assert_eq!(state.get_connections().await.len(), 1);
    }
}
