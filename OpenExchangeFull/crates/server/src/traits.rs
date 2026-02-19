//! Server traits for polymorphic server handling
//!
//! This module defines the core [`Server`] trait that all server implementations
//! must implement, along with the [`ServerExt`] extension trait that provides
//! convenience methods.

use async_trait::async_trait;
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use crate::error::Result;

/// Core server trait that all server implementations must implement.
///
/// This trait provides a consistent interface for starting, running, and
/// monitoring servers across different protocols (HTTP, gRPC, WebSocket).
///
/// # Implementors
///
/// - [`HttpServer`](crate::http::HttpServer) - HTTP server using Axum
/// - [`GrpcServer`](crate::grpc::GrpcServer) - gRPC server using Tonic
/// - [`WebSocketServer`](crate::websocket::WebSocketServer) - WebSocket server
/// - [`CombinedServer`](crate::CombinedServer) - Runs multiple servers
///
/// # Example
///
/// ```ignore
/// use server::{Server, ServerExt, HttpServer, ServerConfig};
///
/// let config = ServerConfig::http_only("127.0.0.1", 8080);
/// let server = HttpServer::simple(config);
///
/// // Run with automatic shutdown handling
/// server.run_with_ctrl_c().await?;
/// ```
#[async_trait]
pub trait Server: Send + Sync + 'static {
    /// Returns the server's name for logging and identification.
    ///
    /// This is typically the protocol name (e.g., "http", "grpc", "websocket")
    /// or a more specific identifier.
    fn name(&self) -> &str;

    /// Returns the address the server is bound to, if running.
    ///
    /// Returns `None` if the server is not currently running or has not
    /// yet bound to an address.
    fn address(&self) -> Option<SocketAddr>;

    /// Returns true if the server is currently running.
    fn is_running(&self) -> bool;

    /// Runs the server until the shutdown token is cancelled.
    ///
    /// This method should:
    /// 1. Bind to the configured address
    /// 2. Start accepting connections
    /// 3. Process requests until `shutdown` is cancelled
    /// 4. Gracefully drain existing connections
    /// 5. Return `Ok(())` on clean shutdown
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Cancellation token that signals when to shut down
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start or encounters a fatal error.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let token = CancellationToken::new();
    /// let token_clone = token.clone();
    ///
    /// // Spawn server
    /// let handle = tokio::spawn(async move {
    ///     server.run(token_clone).await
    /// });
    ///
    /// // Later, trigger shutdown
    /// token.cancel();
    /// handle.await??;
    /// ```
    async fn run(&self, shutdown: CancellationToken) -> Result<()>;
}

/// Extension trait providing convenience methods for servers.
///
/// This trait is automatically implemented for all types that implement [`Server`].
/// It provides common patterns for spawning and running servers.
pub trait ServerExt: Server + Sized {
    /// Spawns the server on a new task and returns a handle and shutdown token.
    ///
    /// This is useful when you want to run the server in the background and
    /// control its lifecycle separately.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - `JoinHandle` - Handle to await the server task
    /// - `CancellationToken` - Token to trigger shutdown
    ///
    /// # Example
    ///
    /// ```ignore
    /// let server = HttpServer::simple(config);
    /// let (handle, token) = server.spawn();
    ///
    /// // Do other work...
    ///
    /// // Trigger shutdown
    /// token.cancel();
    ///
    /// // Wait for server to stop
    /// handle.await??;
    /// ```
    fn spawn(self) -> (tokio::task::JoinHandle<Result<()>>, CancellationToken) {
        let token = CancellationToken::new();
        let token_clone = token.clone();
        let handle = tokio::spawn(async move { self.run(token_clone).await });
        (handle, token)
    }

    /// Runs the server with automatic Ctrl+C handling.
    ///
    /// This is a convenience method that sets up a shutdown controller
    /// that listens for Ctrl+C (SIGINT) and triggers graceful shutdown.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let server = HttpServer::simple(config);
    /// server.run_with_ctrl_c().await?;
    /// // Server runs until Ctrl+C is pressed
    /// ```
    fn run_with_ctrl_c(self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            let shutdown = crate::shutdown::ShutdownController::with_ctrl_c();
            self.run(shutdown.token()).await
        }
    }
}

// Blanket implementation for all Server types
impl<T: Server + Sized> ServerExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock server for testing
    struct MockServer {
        name: String,
    }

    #[async_trait]
    impl Server for MockServer {
        fn name(&self) -> &str {
            &self.name
        }

        fn address(&self) -> Option<SocketAddr> {
            None
        }

        fn is_running(&self) -> bool {
            false
        }

        async fn run(&self, shutdown: CancellationToken) -> Result<()> {
            shutdown.cancelled().await;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_server_ext_spawn() {
        let server = MockServer {
            name: "test".to_string(),
        };

        let (handle, token) = server.spawn();

        // Cancel immediately
        token.cancel();

        // Should complete quickly
        let result = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;
        assert!(result.is_ok());
    }
}
