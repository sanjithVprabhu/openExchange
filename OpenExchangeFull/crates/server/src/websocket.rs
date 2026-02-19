//! WebSocket server implementation using Tokio-Tungstenite
//!
//! This module provides a WebSocket server that implements the [`Server`](crate::Server) trait
//! with connection tracking and customizable message handling.

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock as SyncRwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::ServerConfig;
use crate::error::{Result, ServerError};
use crate::traits::Server;

/// A unique identifier for each WebSocket connection
pub type ConnectionId = u64;

/// Information about an active WebSocket connection
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Unique connection identifier
    pub id: ConnectionId,
    /// Remote peer address
    pub peer_addr: SocketAddr,
    /// When the connection was established
    pub connected_at: std::time::Instant,
}

/// Trait for handling WebSocket messages
///
/// Implement this trait to define custom message handling logic.
///
/// # Example
///
/// ```ignore
/// struct MyHandler;
///
/// impl MessageHandler for MyHandler {
///     fn handle(&self, conn_id: ConnectionId, message: Message) -> Option<Message> {
///         match message {
///             Message::Text(text) => {
///                 // Process text message
///                 Some(Message::Text(format!("Echo: {}", text)))
///             }
///             _ => None,
///         }
///     }
///
///     fn on_connect(&self, conn_id: ConnectionId, peer_addr: SocketAddr) {
///         println!("Client {} connected from {}", conn_id, peer_addr);
///     }
/// }
/// ```
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message and optionally return a response
    ///
    /// Return `Some(message)` to send a response, or `None` for no response.
    fn handle(&self, conn_id: ConnectionId, message: Message) -> Option<Message>;

    /// Called when a new connection is established
    fn on_connect(&self, _conn_id: ConnectionId, _peer_addr: SocketAddr) {}

    /// Called when a connection is closed
    fn on_disconnect(&self, _conn_id: ConnectionId) {}
}

/// A simple echo handler that echoes back all messages
#[derive(Debug, Clone, Copy, Default)]
pub struct EchoHandler;

impl MessageHandler for EchoHandler {
    fn handle(&self, _conn_id: ConnectionId, message: Message) -> Option<Message> {
        // Echo back text and binary messages
        match &message {
            Message::Text(_) | Message::Binary(_) => Some(message),
            _ => None,
        }
    }
}

/// WebSocket server implementation with connection tracking
///
/// This server provides:
/// - Connection tracking with unique IDs
/// - Customizable message handling via [`MessageHandler`]
/// - Graceful shutdown with connection draining
///
/// # Example
///
/// ```ignore
/// use server::{WebSocketServer, ServerConfig, Server, ServerExt};
///
/// let config = ServerConfig::websocket_only("127.0.0.1", 7080);
/// let server = WebSocketServer::new(config);
///
/// // Run with Ctrl+C handling
/// server.run_with_ctrl_c().await?;
/// ```
#[derive(Clone)]
pub struct WebSocketServer {
    config: ServerConfig,
    running: Arc<AtomicBool>,
    bound_addr: Arc<SyncRwLock<Option<SocketAddr>>>,
    next_conn_id: Arc<AtomicU64>,
    connections: Arc<RwLock<HashMap<ConnectionId, ConnectionInfo>>>,
    handler: Arc<dyn MessageHandler>,
}

impl WebSocketServer {
    /// Create a new WebSocket server with a custom message handler
    pub fn with_handler<H: MessageHandler + 'static>(config: ServerConfig, handler: H) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            bound_addr: Arc::new(SyncRwLock::new(None)),
            next_conn_id: Arc::new(AtomicU64::new(1)),
            connections: Arc::new(RwLock::new(HashMap::new())),
            handler: Arc::new(handler),
        }
    }

    /// Create a simple echo server
    pub fn new(config: ServerConfig) -> Self {
        Self::with_handler(config, EchoHandler)
    }

    /// Get the bind address, returning an error if WebSocket port is not configured
    fn bind_addr(&self) -> Result<SocketAddr> {
        let port = self
            .config
            .websocket_port
            .ok_or_else(|| ServerError::ConfigError("WebSocket port not configured".into()))?;

        format!("{}:{}", self.config.host, port)
            .parse()
            .map_err(|_| ServerError::InvalidAddress(format!("{}:{}", self.config.host, port)))
    }

    /// Get the server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get the number of active connections
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Get information about all active connections
    pub async fn active_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Generate the next connection ID
    fn next_connection_id(&self) -> ConnectionId {
        self.next_conn_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Register a new connection
    async fn register_connection(&self, id: ConnectionId, peer_addr: SocketAddr) {
        let info = ConnectionInfo {
            id,
            peer_addr,
            connected_at: std::time::Instant::now(),
        };
        self.connections.write().await.insert(id, info);
        self.handler.on_connect(id, peer_addr);
    }

    /// Unregister a connection
    async fn unregister_connection(&self, id: ConnectionId) {
        self.connections.write().await.remove(&id);
        self.handler.on_disconnect(id);
    }

    /// Handle a single WebSocket connection
    async fn handle_connection(
        &self,
        conn_id: ConnectionId,
        stream: TcpStream,
        peer_addr: SocketAddr,
        conn_token: CancellationToken,
    ) -> Result<()> {
        debug!(conn_id, %peer_addr, "WebSocket connection established");

        // Accept WebSocket upgrade
        let ws_stream = accept_async(stream).await.map_err(ServerError::WebSocket)?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Register connection
        self.register_connection(conn_id, peer_addr).await;

        // Handle messages until disconnect or shutdown
        loop {
            tokio::select! {
                // Check for shutdown
                _ = conn_token.cancelled() => {
                    debug!(conn_id, "Connection shutting down due to server shutdown");
                    // Send close frame
                    let _ = ws_sender.send(Message::Close(None)).await;
                    break;
                }

                // Handle incoming messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(message)) => {
                            if message.is_close() {
                                debug!(conn_id, "WebSocket client disconnected gracefully");
                                break;
                            }

                            // Process message through handler
                            if let Some(response) = self.handler.handle(conn_id, message) {
                                if let Err(e) = ws_sender.send(response).await {
                                    error!(conn_id, %e, "Failed to send WebSocket message");
                                    break;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!(conn_id, %e, "WebSocket error");
                            break;
                        }
                        None => {
                            debug!(conn_id, "WebSocket stream ended");
                            break;
                        }
                    }
                }
            }
        }

        // Unregister connection
        self.unregister_connection(conn_id).await;

        debug!(conn_id, "WebSocket connection closed");
        Ok(())
    }
}

#[async_trait]
impl Server for WebSocketServer {
    fn name(&self) -> &str {
        "websocket"
    }

    fn address(&self) -> Option<SocketAddr> {
        *self.bound_addr.read()
    }

    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    async fn run(&self, shutdown_token: CancellationToken) -> Result<()> {
        let addr = self.bind_addr()?;

        info!(%addr, "Starting WebSocket server");

        // Create TCP listener
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ServerError::bind(addr.to_string(), e))?;

        let local_addr = listener.local_addr().map_err(ServerError::Io)?;
        *self.bound_addr.write() = Some(local_addr);
        
        info!(%local_addr, "WebSocket server listening");

        self.running.store(true, Ordering::SeqCst);

        // Track connection tasks for graceful shutdown
        let mut connection_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

        // Accept connections loop
        loop {
            tokio::select! {
                // Check for shutdown
                _ = shutdown_token.cancelled() => {
                    info!("WebSocket server received shutdown signal");
                    break;
                }

                // Accept new connections
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            let conn_id = self.next_connection_id();
                            let server = self.clone();
                            // Create a child token for this connection
                            let conn_token = shutdown_token.child_token();

                            let handle = tokio::spawn(async move {
                                if let Err(e) = server.handle_connection(
                                    conn_id,
                                    stream,
                                    peer_addr,
                                    conn_token
                                ).await {
                                    error!(conn_id, %e, "WebSocket connection error");
                                }
                            });

                            connection_handles.push(handle);

                            // Clean up completed handles periodically
                            connection_handles.retain(|h| !h.is_finished());
                        }
                        Err(e) => {
                            error!(%e, "Failed to accept WebSocket connection");
                        }
                    }
                }
            }
        }

        // Graceful shutdown: wait for all connections to close
        let connection_count = connection_handles.len();
        if connection_count > 0 {
            info!(connection_count, "Waiting for active WebSocket connections to close...");

            // Give connections time to close gracefully (10 second timeout)
            let timeout = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                futures::future::join_all(connection_handles),
            );

            match timeout.await {
                Ok(_) => {
                    info!("All WebSocket connections closed gracefully");
                }
                Err(_) => {
                    warn!("Timed out waiting for WebSocket connections to close");
                }
            }
        }

        self.running.store(false, Ordering::SeqCst);
        *self.bound_addr.write() = None;
        
        info!("WebSocket server shutdown complete");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ServerExt;
    use std::time::Duration;

    #[tokio::test]
    async fn test_websocket_server_shutdown() {
        let config = ServerConfig {
            host: "127.0.0.1".to_string(),
            http_port: None,
            grpc_port: None,
            websocket_port: Some(0), // Use ephemeral port
        };

        let server = WebSocketServer::new(config);
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
    fn test_websocket_server_name() {
        let config = ServerConfig::websocket_only("127.0.0.1", 7080);
        let server = WebSocketServer::new(config);
        assert_eq!(server.name(), "websocket");
    }

    #[test]
    fn test_echo_handler() {
        let handler = EchoHandler;
        
        // Text message should be echoed
        let msg = Message::Text("hello".to_string());
        assert!(handler.handle(1, msg.clone()).is_some());
        
        // Ping should not be echoed
        let ping = Message::Ping(vec![]);
        assert!(handler.handle(1, ping).is_none());
    }
}
