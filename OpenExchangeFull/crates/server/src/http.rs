//! HTTP server implementation using Axum
//!
//! This module provides an HTTP server built on Axum, implementing the
//! [`Server`](crate::Server) trait for consistent lifecycle management.

use async_trait::async_trait;
use axum::{routing::get, Router};
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::config::ServerConfig;
use crate::error::{Result, ServerError};
use crate::traits::Server;

/// HTTP server implementation using Axum
///
/// This server wraps an Axum router and provides graceful shutdown support
/// through the [`Server`] trait.
///
/// # Example
///
/// ```ignore
/// use server::{HttpServer, ServerConfig, Server, ServerExt};
///
/// let config = ServerConfig::http_only("127.0.0.1", 8080);
/// let server = HttpServer::simple(config);
///
/// // Run with Ctrl+C handling
/// server.run_with_ctrl_c().await?;
/// ```
#[derive(Clone)]
pub struct HttpServer {
    config: ServerConfig,
    router: Router,
    running: Arc<AtomicBool>,
    bound_addr: Arc<RwLock<Option<SocketAddr>>>,
}

impl HttpServer {
    /// Create a new HTTP server with a custom router
    pub fn new(config: ServerConfig, router: Router) -> Self {
        Self {
            config,
            router,
            running: Arc::new(AtomicBool::new(false)),
            bound_addr: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a simple HTTP server with default routes
    ///
    /// The default routes include:
    /// - `GET /` - Returns "OpenExchange HTTP Server"
    /// - `GET /health` - Returns health check JSON
    pub fn simple(config: ServerConfig) -> Self {
        let router = Router::new()
            .route("/", get(|| async { "OpenExchange HTTP Server" }))
            .route("/health", get(crate::health::simple_health_handler));

        Self::new(config, router)
    }

    /// Create an HTTP server with a custom name prefix on the index route
    pub fn with_name(config: ServerConfig, name: &str) -> Self {
        let name = name.to_string();
        let router = Router::new()
            .route("/", get(move || {
                let name = name.clone();
                async move { format!("{} HTTP Server", name) }
            }))
            .route("/health", get(crate::health::simple_health_handler));

        Self::new(config, router)
    }

    /// Get the bind address, returning an error if HTTP port is not configured
    fn bind_addr(&self) -> Result<SocketAddr> {
        let port = self
            .config
            .http_port
            .ok_or_else(|| ServerError::ConfigError("HTTP port not configured".into()))?;

        format!("{}:{}", self.config.host, port)
            .parse()
            .map_err(|_| ServerError::InvalidAddress(format!("{}:{}", self.config.host, port)))
    }

    /// Get the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get the router (for testing or inspection)
    pub fn router(&self) -> &Router {
        &self.router
    }
}

#[async_trait]
impl Server for HttpServer {
    fn name(&self) -> &str {
        "http"
    }

    fn address(&self) -> Option<SocketAddr> {
        *self.bound_addr.read()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    async fn run(&self, shutdown_token: CancellationToken) -> Result<()> {
        let addr = self.bind_addr()?;

        info!(%addr, "Starting HTTP server");

        // Create TCP listener
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ServerError::bind(addr.to_string(), e))?;

        let local_addr = listener.local_addr().map_err(ServerError::Io)?;
        
        // Store the bound address
        *self.bound_addr.write() = Some(local_addr);
        
        info!(%local_addr, "HTTP server listening");

        self.running.store(true, Ordering::SeqCst);

        // Run server with graceful shutdown
        let result = axum::serve(listener, self.router.clone())
            .with_graceful_shutdown(async move {
                shutdown_token.cancelled().await;
                info!("HTTP server received shutdown signal");
            })
            .await;

        self.running.store(false, Ordering::SeqCst);
        *self.bound_addr.write() = None;

        match result {
            Ok(()) => {
                info!("HTTP server shutdown complete");
                Ok(())
            }
            Err(e) => {
                error!(%e, "HTTP server error");
                Err(ServerError::Io(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ServerExt;
    use std::time::Duration;

    #[tokio::test]
    async fn test_http_server_shutdown() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            http_port: Some(0), // Use ephemeral port
            grpc_port: None,
            websocket_port: None,
        };

        let server = HttpServer::simple(config);
        let (handle, token) = server.spawn();

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger shutdown
        token.cancel();

        // Wait for server to stop
        let result = tokio::time::timeout(Duration::from_secs(5), handle).await;

        assert!(result.is_ok(), "Server should shutdown within timeout");
    }

    #[test]
    fn test_http_server_name() {
        let config = ServerConfig::http_only("127.0.0.1", 8080);
        let server = HttpServer::simple(config);
        assert_eq!(server.name(), "http");
    }
}
