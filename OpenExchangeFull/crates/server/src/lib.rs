//! Server infrastructure for OpenExchange
//!
//! This crate provides HTTP, gRPC, and WebSocket server implementations
//! with unified lifecycle management and graceful shutdown.
//!
// Allow large error types - WebSocket errors are unavoidably large
#![allow(clippy::result_large_err)]
//!
//! # Architecture
//!
//! All servers implement the [`Server`] trait, which provides a consistent
//! interface for running and monitoring servers. The [`ServerExt`] trait
//! provides convenience methods like `spawn()` and `run_with_ctrl_c()`.
//!
//! Shutdown coordination uses `CancellationToken` from `tokio_util`, allowing
//! hierarchical shutdown where cancelling a parent token automatically cancels
//! all child tokens.
//!
//! # Quick Start
//!
//! ```ignore
//! use server::{CombinedServer, ServerConfig, Server, ServerExt};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ServerConfig::for_service("gateway");
//!     let server = CombinedServer::new(config);
//!     
//!     // Run with Ctrl+C handling
//!     server.run_with_ctrl_c().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Modules
//!
//! - [`config`] - Server configuration and port constants
//! - [`traits`] - `Server` and `ServerExt` traits
//! - [`http`] - HTTP server using Axum
//! - [`grpc`] - gRPC server using Tonic (placeholder)
//! - [`websocket`] - WebSocket server using Tungstenite
//! - [`health`] - Health check endpoints and client
//! - [`shutdown`] - Graceful shutdown utilities

use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

// Core modules
pub mod config;
pub mod error;
pub mod shutdown;
pub mod traits;

// Server implementations
pub mod grpc;
pub mod health;
pub mod http;
pub mod port_validator;
pub mod websocket;

// Re-exports for convenience
pub use config::{ports, ServerConfig};
pub use error::{Result, ServerError};
pub use grpc::GrpcServer;
pub use health::{HealthClient, HealthState, HealthStatus};
pub use http::HttpServer;
pub use port_validator::validate_ports_available;
pub use shutdown::{shutdown_signal, ShutdownController};
pub use traits::{Server, ServerExt};
pub use websocket::{MessageHandler, WebSocketServer};

/// Combined server that runs HTTP, gRPC, and WebSocket protocols
///
/// This struct coordinates multiple server types and provides unified
/// startup and shutdown handling using `CancellationToken`.
///
/// # Example
///
/// ```ignore
/// use server::{CombinedServer, ServerConfig, Server, ServerExt};
///
/// let config = ServerConfig::for_service("gateway");
/// let server = CombinedServer::new(config);
///
/// // Option 1: Run with Ctrl+C handling
/// server.run_with_ctrl_c().await?;
///
/// // Option 2: Manual control
/// let (handle, token) = server.spawn();
/// // ... later ...
/// token.cancel();
/// handle.await??;
/// ```
pub struct CombinedServer {
    name: String,
    config: ServerConfig,
    http_server: Option<HttpServer>,
    grpc_server: Option<GrpcServer>,
    ws_server: Option<WebSocketServer>,
}

impl CombinedServer {
    /// Create a new combined server with default servers for all configured ports
    pub fn new(config: ServerConfig) -> Self {
        Self::with_name("combined", config)
    }

    /// Create a new combined server with a custom name
    pub fn with_name(name: impl Into<String>, config: ServerConfig) -> Self {
        let http_server = config.http_port.map(|_| HttpServer::simple(config.clone()));
        let grpc_server = config.grpc_port.map(|_| GrpcServer::new(config.clone()));
        let ws_server = config.websocket_port.map(|_| WebSocketServer::new(config.clone()));

        Self {
            name: name.into(),
            config,
            http_server,
            grpc_server,
            ws_server,
        }
    }

    /// Create a new combined server with a custom HTTP router
    pub fn with_http_router(config: ServerConfig, http_router: axum::Router) -> Self {
        let http_server = config
            .http_port
            .map(|_| HttpServer::new(config.clone(), http_router));
        let grpc_server = config.grpc_port.map(|_| GrpcServer::new(config.clone()));
        let ws_server = config.websocket_port.map(|_| WebSocketServer::new(config.clone()));

        Self {
            name: "combined".into(),
            config,
            http_server,
            grpc_server,
            ws_server,
        }
    }

    /// Create a simple ping/health server with default config for service
    pub fn ping_server(service_name: impl Into<String>) -> Self {
        let service_name = service_name.into();
        let config = ServerConfig::for_service(&service_name);
        Self::ping_server_with_config(service_name, config)
    }

    /// Create a simple ping/health server with custom config
    pub fn ping_server_with_config(service_name: impl Into<String>, config: ServerConfig) -> Self {
        let service_name = service_name.into();
        let service_name_clone = service_name.clone();

        // Create HTTP router with health endpoint
        let http_router = axum::Router::new()
            .route(
                "/health",
                axum::routing::get(health::simple_health_handler),
            )
            .route(
                "/",
                axum::routing::get(move || async move { format!("{} Service", service_name_clone) }),
            );

        let mut server = Self::with_http_router(config, http_router);
        server.name = service_name;
        server
    }

    /// Get the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Validate that all configured ports are available
    pub async fn validate_ports(&self) -> Result<()> {
        validate_ports_available(&self.config).await
    }
}

#[async_trait::async_trait]
impl Server for CombinedServer {
    fn name(&self) -> &str {
        &self.name
    }

    fn address(&self) -> Option<std::net::SocketAddr> {
        // Return the HTTP address if available, otherwise gRPC, otherwise WebSocket
        self.http_server
            .as_ref()
            .and_then(|s| s.address())
            .or_else(|| self.grpc_server.as_ref().and_then(|s| s.address()))
            .or_else(|| self.ws_server.as_ref().and_then(|s| s.address()))
    }

    fn is_running(&self) -> bool {
        self.http_server
            .as_ref()
            .map(|s| s.is_running())
            .unwrap_or(false)
            || self
                .grpc_server
                .as_ref()
                .map(|s| s.is_running())
                .unwrap_or(false)
            || self
                .ws_server
                .as_ref()
                .map(|s| s.is_running())
                .unwrap_or(false)
    }

    async fn run(&self, shutdown_token: CancellationToken) -> Result<()> {
        info!(server = %self.name, "Starting combined server...");

        let mut handles: Vec<tokio::task::JoinHandle<Result<()>>> = Vec::new();

        // Start HTTP server if configured
        if let Some(ref http) = self.http_server {
            let http = http.clone();
            let token = shutdown_token.child_token();
            if let Some(port) = self.config.http_port {
                info!(port, "Starting HTTP server");
            }
            handles.push(tokio::spawn(async move { http.run(token).await }));
        }

        // Start gRPC server if configured
        if let Some(ref grpc) = self.grpc_server {
            let grpc = grpc.clone();
            let token = shutdown_token.child_token();
            if let Some(port) = self.config.grpc_port {
                info!(port, "Starting gRPC server");
            }
            handles.push(tokio::spawn(async move { grpc.run(token).await }));
        }

        // Start WebSocket server if configured
        if let Some(ref ws) = self.ws_server {
            let ws = ws.clone();
            let token = shutdown_token.child_token();
            if let Some(port) = self.config.websocket_port {
                info!(port, "Starting WebSocket server");
            }
            handles.push(tokio::spawn(async move { ws.run(token).await }));
        }

        if handles.is_empty() {
            warn!("No servers configured to start");
            return Ok(());
        }

        info!(server = %self.name, "All server components started");

        // Wait for either:
        // 1. The shutdown token to be cancelled, OR
        // 2. Any server to exit unexpectedly
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                info!("Shutdown signal received");
            }
            result = wait_for_first_completion(&mut handles) => {
                match result {
                    Some(Ok(Ok(()))) => {
                        warn!("A server exited unexpectedly (but successfully)");
                    }
                    Some(Ok(Err(e))) => {
                        error!(%e, "A server exited with error");
                    }
                    Some(Err(e)) => {
                        error!(%e, "A server task panicked");
                    }
                    None => {}
                }
                // Cancel remaining servers
                shutdown_token.cancel();
            }
        }

        // Wait for all servers to shut down with a timeout
        info!("Waiting for all servers to shut down...");
        let shutdown_timeout = std::time::Duration::from_secs(30);

        match tokio::time::timeout(shutdown_timeout, wait_for_all_completion(handles)).await {
            Ok(results) => {
                let errors: Vec<_> = results
                    .into_iter()
                    .filter_map(|r| match r {
                        Ok(Ok(())) => None,
                        Ok(Err(e)) => Some(e.to_string()),
                        Err(e) => Some(format!("Task panicked: {}", e)),
                    })
                    .collect();

                if errors.is_empty() {
                    info!(server = %self.name, "All servers shut down successfully");
                } else {
                    warn!(?errors, "Some servers had errors during shutdown");
                }
            }
            Err(_) => {
                warn!("Timed out waiting for servers to shut down");
            }
        }

        info!(server = %self.name, "Combined server shutdown complete");
        Ok(())
    }
}

/// Wait for the first handle to complete
async fn wait_for_first_completion(
    handles: &mut [tokio::task::JoinHandle<Result<()>>],
) -> Option<std::result::Result<Result<()>, tokio::task::JoinError>> {
    if handles.is_empty() {
        return None;
    }

    let (result, _index, _remaining) =
        futures::future::select_all(handles.iter_mut().map(Box::pin)).await;

    Some(result)
}

/// Wait for all handles to complete
async fn wait_for_all_completion(
    handles: Vec<tokio::task::JoinHandle<Result<()>>>,
) -> Vec<std::result::Result<Result<()>, tokio::task::JoinError>> {
    futures::future::join_all(handles).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_server_config() {
        let config = ServerConfig::new("127.0.0.1", 8080, 9080, 7080);

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.grpc_port, Some(9080));
        assert_eq!(config.websocket_port, Some(7080));
        assert!(config.has_servers());
    }

    #[test]
    fn test_server_config_for_service() {
        let config = ServerConfig::for_service("gateway");
        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.grpc_port, Some(9080));
        assert_eq!(config.websocket_port, Some(7080));

        let config = ServerConfig::for_service("oms");
        assert_eq!(config.http_port, Some(8082));
        assert_eq!(config.grpc_port, Some(9082));
        assert_eq!(config.websocket_port, Some(7082));

        // Verify correct port assignments
        let config = ServerConfig::for_service("risk");
        assert_eq!(config.http_port, Some(8086));

        let config = ServerConfig::for_service("wallet");
        assert_eq!(config.http_port, Some(8084));
    }

    #[tokio::test]
    async fn test_combined_server_shutdown() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            http_port: Some(0), // Use ephemeral ports
            grpc_port: Some(0),
            websocket_port: Some(0),
        };

        let server = CombinedServer::new(config);
        let (handle, token) = server.spawn();

        // Give servers time to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Trigger shutdown
        token.cancel();

        // Wait for servers to stop
        let result = tokio::time::timeout(Duration::from_secs(10), handle).await;

        assert!(result.is_ok(), "Server should shutdown within timeout");
    }
}
