//! gRPC server implementation using Tonic
//!
//! This module provides a gRPC server that implements the [`Server`](crate::Server) trait.
//! It supports running tonic gRPC services with graceful shutdown.

use async_trait::async_trait;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::config::ServerConfig;
use crate::error::{Result, ServerError};
use crate::traits::Server;

/// Type alias for a function that runs a tonic server with shutdown.
type TonicServerFn = Box<
    dyn Fn(SocketAddr, CancellationToken) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

/// gRPC server implementation using Tonic
///
/// This server can run tonic gRPC services with graceful shutdown support.
/// Use `GrpcServerBuilder` to construct a server with services.
///
/// # Example
///
/// ```ignore
/// use server::{GrpcServerBuilder, ServerConfig, Server, ServerExt};
/// use tonic::transport::Server;
///
/// let config = ServerConfig::grpc_only("127.0.0.1", 9081);
///
/// let server = GrpcServerBuilder::new(config)
///     .with_tonic_server(|builder| {
///         builder.add_service(MyService::new())
///     })
///     .build();
///
/// server.run_with_ctrl_c().await?;
/// ```
#[derive(Clone)]
pub struct GrpcServer {
    config: ServerConfig,
    /// Optional tonic server runner. If None, acts as placeholder.
    tonic_runner: Option<Arc<TonicServerFn>>,
    running: Arc<AtomicBool>,
    bound_addr: Arc<RwLock<Option<SocketAddr>>>,
}

impl GrpcServer {
    /// Create a new gRPC server (placeholder mode - no services)
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            tonic_runner: None,
            running: Arc::new(AtomicBool::new(false)),
            bound_addr: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a gRPC server with a tonic server configuration.
    ///
    /// The `setup_fn` receives a `tonic::transport::Server::builder()` and should
    /// add services to it.
    pub fn with_tonic<F>(config: ServerConfig, setup_fn: F) -> Self
    where
        F: Fn(tonic::transport::Server) -> tonic::transport::server::Router + Send + Sync + 'static,
    {
        let setup = Arc::new(setup_fn);

        let runner: TonicServerFn = Box::new(move |addr: SocketAddr, shutdown_token: CancellationToken| {
            let setup_clone = setup.clone();
            Box::pin(async move {
                let builder = tonic::transport::Server::builder();
                let router = setup_clone(builder);

                router
                    .serve_with_shutdown(addr, async move {
                        shutdown_token.cancelled().await;
                    })
                    .await
                    .map_err(|e| ServerError::Internal(format!("gRPC server error: {}", e)))?;

                Ok(())
            })
        });

        Self {
            config,
            tonic_runner: Some(Arc::new(runner)),
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

    /// Check if this server has tonic services configured
    pub fn has_services(&self) -> bool {
        self.tonic_runner.is_some()
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
        *self.bound_addr.write() = Some(addr);
        self.running.store(true, Ordering::SeqCst);

        if let Some(ref runner) = self.tonic_runner {
            info!(%addr, "Starting gRPC server with services");
            let result = runner(addr, shutdown_token.clone()).await;
            self.running.store(false, Ordering::SeqCst);
            *self.bound_addr.write() = None;
            info!("gRPC server shutdown complete");
            result
        } else {
            info!(%addr, "Starting gRPC server (placeholder - no services registered)");

            // Placeholder mode: just bind to port and wait for shutdown
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .map_err(|e| ServerError::bind(addr.to_string(), e))?;

            let local_addr = listener.local_addr().map_err(ServerError::Io)?;
            *self.bound_addr.write() = Some(local_addr);

            info!(%local_addr, "gRPC server listening (placeholder)");

            shutdown_token.cancelled().await;
            drop(listener);

            self.running.store(false, Ordering::SeqCst);
            *self.bound_addr.write() = None;

            info!("gRPC server shutdown complete");
            Ok(())
        }
    }
}

/// Builder for creating a gRPC server with tonic services.
///
/// # Example
///
/// ```ignore
/// let server = GrpcServerBuilder::new(config)
///     .with_tonic_server(|builder| {
///         builder
///             .add_service(MyServiceServer::new(my_service))
///             .add_service(OtherServiceServer::new(other_service))
///     })
///     .build();
/// ```
pub struct GrpcServerBuilder {
    config: ServerConfig,
    tonic_runner: Option<Arc<TonicServerFn>>,
}

impl GrpcServerBuilder {
    /// Create a new builder with the given configuration.
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            tonic_runner: None,
        }
    }

    /// Configure the tonic server with services.
    ///
    /// The `setup_fn` receives `tonic::transport::Server::builder()` and should
    /// add services to it, returning the configured router.
    ///
    /// # Example
    ///
    /// ```ignore
    /// builder.with_tonic_server(|b| {
    ///     b.add_service(InstrumentServiceServer::new(service))
    /// })
    /// ```
    pub fn with_tonic_server<F>(mut self, setup_fn: F) -> Self
    where
        F: Fn(tonic::transport::Server) -> tonic::transport::server::Router + Send + Sync + 'static,
    {
        let setup = Arc::new(setup_fn);

        let runner: TonicServerFn = Box::new(move |addr: SocketAddr, shutdown_token: CancellationToken| {
            let setup_clone = setup.clone();
            Box::pin(async move {
                let builder = tonic::transport::Server::builder();
                let router = setup_clone(builder);

                router
                    .serve_with_shutdown(addr, async move {
                        shutdown_token.cancelled().await;
                    })
                    .await
                    .map_err(|e| ServerError::Internal(format!("gRPC server error: {}", e)))?;

                Ok(())
            })
        });

        self.tonic_runner = Some(Arc::new(runner));
        self
    }

    /// Build the gRPC server.
    pub fn build(self) -> GrpcServer {
        GrpcServer {
            config: self.config,
            tonic_runner: self.tonic_runner,
            running: Arc::new(AtomicBool::new(false)),
            bound_addr: Arc::new(RwLock::new(None)),
        }
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
            grpc_port: Some(0),
            websocket_port: None,
        };

        let server = GrpcServer::new(config);
        let (handle, token) = server.spawn();

        tokio::time::sleep(Duration::from_millis(100)).await;
        token.cancel();

        let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(result.is_ok(), "Server should shutdown within timeout");
    }

    #[test]
    fn test_grpc_server_name() {
        let config = ServerConfig::grpc_only("127.0.0.1", 9080);
        let server = GrpcServer::new(config);
        assert_eq!(server.name(), "grpc");
    }

    #[test]
    fn test_grpc_server_builder() {
        let config = ServerConfig::grpc_only("127.0.0.1", 9080);
        let server = GrpcServerBuilder::new(config)
            .build();
        
        assert!(!server.has_services());
    }
}
