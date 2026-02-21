//! Server error types

use std::io;
use thiserror::Error;

/// Result type alias for server operations
/// 
/// Note: We allow large error variants here because the WebSocket error type is large.
/// This is acceptable for server startup/shutdown errors which are not on the hot path.
#[allow(clippy::result_large_err)]
pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Port {port} is already in use: {reason}")]
    PortInUse { port: u16, reason: String },

    #[error("Failed to bind to address {address}: {source}")]
    BindError {
        address: String,
        #[source]
        source: io::Error,
    },

    #[error("Server failed to start: {0}")]
    StartError(String),

    #[error("Server failed to stop: {0}")]
    StopError(String),

    #[error("Invalid server configuration: {0}")]
    ConfigError(String),

    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Server not running")]
    NotRunning,

    #[error("Server already running")]
    AlreadyRunning,

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ServerError {
    /// Create a bind error from an address string and IO error
    pub fn bind(address: impl Into<String>, source: io::Error) -> Self {
        Self::BindError {
            address: address.into(),
            source,
        }
    }

    /// Create a port in use error
    pub fn port_in_use(port: u16, reason: impl Into<String>) -> Self {
        Self::PortInUse {
            port,
            reason: reason.into(),
        }
    }
}
