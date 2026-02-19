//! gRPC server implementation using Tonic
//!
//! This module provides a gRPC server that implements the [`Server`](crate::Server) trait.
//!
//! # Status
//!
//! **Placeholder** - This is currently a placeholder that binds to the configured port
//! but doesn't serve any gRPC services. To add actual gRPC services:
//!
//! 1. Create `.proto` files in a `proto/` directory
//! 2. Add `tonic-build` to build-dependencies
//! 3. Create `build.rs` to compile protos
//! 4. Implement the generated service traits
//! 5. Use `GrpcServerBuilder` to add services

use async_trait::async_trait;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::ServerConfig;
use crate::error::{Result, ServerError};
use crate::traits::Server;

/// gRPC server implementation
///
/// Currently this is a placeholder that binds to the configured port
/// but doesn't serve any gRPC services. Add proto definitions to enable
/// actual gRPC functionality.
///
/// # Example
///
/// ```ignore
/// use server::{GrpcServer, ServerConfig, Server, ServerExt};
///
/// let config = ServerConfig::grpc_only("127.0.0.1", 9080);
/// let server = GrpcServer::new(config);
///
/// // Note: This is a placeholder - no actual gRPC services are available
/// server.run_with_ctrl_c().await?;
/// ```
#[derive(Clone)]
pub struct GrpcServer {
    config: ServerConfig,
    running: Arc<AtomicBool>,
    bound_addr: Arc<RwLock<Option<SocketAddr>>>,
}

impl GrpcServer {
    /// Create a new gRPC server
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            bound_addr: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the bind address, returning an error if gRPC port is not configured
    fn bind_addr(&self) -> Result<SocketAddr> {
        let port = self
            .config
            .grpc_port
            .ok_or_else(|| ServerError::ConfigError("gRPC port not configured".into()))?;

        format!("{}:{}", self.config.host, port)
            .parse()
            .map_err(|_| ServerError::InvalidAddress(format!("{}:{}", self.config.host, port)))
    }

    /// Get the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

#[async_trait]
impl Server for GrpcServer {
    fn name(&self) -> &str {
        "grpc"
    }

    fn address(&self) -> Option<SocketAddr> {
        *self.bound_addr.read()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    async fn run(&self, shutdown_token: CancellationToken) -> Result<()> {
        let addr = self.bind_addr()?;

        info!(%addr, "Starting gRPC server (placeholder - no services registered)");

        // Bind to reserve the port
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ServerError::bind(addr.to_string(), e))?;

        let local_addr = listener.local_addr().map_err(ServerError::Io)?;
        *self.bound_addr.write() = Some(local_addr);
        
        info!(%local_addr, "gRPC server listening");

        self.running.store(true, Ordering::SeqCst);

        // For the placeholder, we just wait for shutdown.
        // We don't accept connections since we have no services to handle them.
        //
        // When you add real gRPC services, replace this with:
        // ```
        // tonic::transport::Server::builder()
        //     .add_service(YourService::new())
        //     .serve_with_shutdown(addr, async move {
        //         shutdown_token.cancelled().await;
        //     })
        //     .await?;
        // ```
        shutdown_token.cancelled().await;

        // Drop the listener to release the port
        drop(listener);

        self.running.store(false, Ordering::SeqCst);
        *self.bound_addr.write() = None;
        
        info!("gRPC server shutdown complete");

        Ok(())
    }
}

/// Builder for creating a gRPC server with services
///
/// This is a placeholder for future use when proto files are added.
///
/// # Example
///
/// ```ignore
/// let server = GrpcServerBuilder::new(config)
///     .add_service(my_service)
///     .build();
/// ```
pub struct GrpcServerBuilder {
    config: ServerConfig,
    // Future: services will be added here
}

impl GrpcServerBuilder {
    /// Create a new builder
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Build the gRPC server
    ///
    /// Currently returns the placeholder server.
    pub fn build(self) -> GrpcServer {
        GrpcServer::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ServerExt;
    use std::time::Duration;

    #[tokio::test]
    async fn test_grpc_server_shutdown() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            http_port: None,
            grpc_port: Some(0), // Use ephemeral port
            websocket_port: None,
        };

        let server = GrpcServer::new(config);
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
    fn test_grpc_server_name() {
        let config = ServerConfig::grpc_only("127.0.0.1", 9080);
        let server = GrpcServer::new(config);
        assert_eq!(server.name(), "grpc");
    }
}
