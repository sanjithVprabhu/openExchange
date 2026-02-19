//! Server configuration
//!
//! This module provides server configuration types and port constants
//! for all OpenExchange services.

use crate::error::{Result, ServerError};
use std::net::SocketAddr;

/// Standard port assignments for each service
///
/// These are the default ports used by each service in the exchange.
/// HTTP ports are in the 8080-8087 range, gRPC in 9080-9087, and
/// WebSocket in 7080-7087.
pub mod ports {
    // Gateway / Monolith
    /// Gateway HTTP port
    pub const GATEWAY_HTTP: u16 = 8080;
    /// Gateway gRPC port
    pub const GATEWAY_GRPC: u16 = 9080;
    /// Gateway WebSocket port
    pub const GATEWAY_WS: u16 = 7080;

    // Instrument Service
    /// Instrument service HTTP port
    pub const INSTRUMENT_HTTP: u16 = 8081;
    /// Instrument service gRPC port
    pub const INSTRUMENT_GRPC: u16 = 9081;
    /// Instrument service WebSocket port
    pub const INSTRUMENT_WS: u16 = 7081;

    // Order Management System
    /// OMS HTTP port
    pub const OMS_HTTP: u16 = 8082;
    /// OMS gRPC port
    pub const OMS_GRPC: u16 = 9082;
    /// OMS WebSocket port
    pub const OMS_WS: u16 = 7082;

    // Matching Engine
    /// Matching engine HTTP port
    pub const MATCHING_HTTP: u16 = 8083;
    /// Matching engine gRPC port
    pub const MATCHING_GRPC: u16 = 9083;
    /// Matching engine WebSocket port
    pub const MATCHING_WS: u16 = 7083;

    // Wallet Service
    /// Wallet service HTTP port
    pub const WALLET_HTTP: u16 = 8084;
    /// Wallet service gRPC port
    pub const WALLET_GRPC: u16 = 9084;
    /// Wallet service WebSocket port
    pub const WALLET_WS: u16 = 7084;

    // Settlement Service
    /// Settlement service HTTP port
    pub const SETTLEMENT_HTTP: u16 = 8085;
    /// Settlement service gRPC port
    pub const SETTLEMENT_GRPC: u16 = 9085;
    /// Settlement service WebSocket port
    pub const SETTLEMENT_WS: u16 = 7085;

    // Risk Engine
    /// Risk engine HTTP port
    pub const RISK_HTTP: u16 = 8086;
    /// Risk engine gRPC port
    pub const RISK_GRPC: u16 = 9086;
    /// Risk engine WebSocket port
    pub const RISK_WS: u16 = 7086;

    // Market Data
    /// Market data service HTTP port
    pub const MARKET_HTTP: u16 = 8087;
    /// Market data service gRPC port
    pub const MARKET_GRPC: u16 = 9087;
    /// Market data service WebSocket port
    pub const MARKET_WS: u16 = 7087;

    /// Get ports for a service by name
    ///
    /// Returns (HTTP, gRPC, WebSocket) port tuple.
    pub fn for_service(name: &str) -> (u16, u16, u16) {
        match name.to_lowercase().as_str() {
            "gateway" | "monolith" => (GATEWAY_HTTP, GATEWAY_GRPC, GATEWAY_WS),
            "instrument" => (INSTRUMENT_HTTP, INSTRUMENT_GRPC, INSTRUMENT_WS),
            "oms" => (OMS_HTTP, OMS_GRPC, OMS_WS),
            "matching" => (MATCHING_HTTP, MATCHING_GRPC, MATCHING_WS),
            "wallet" => (WALLET_HTTP, WALLET_GRPC, WALLET_WS),
            "settlement" => (SETTLEMENT_HTTP, SETTLEMENT_GRPC, SETTLEMENT_WS),
            "risk" => (RISK_HTTP, RISK_GRPC, RISK_WS),
            "market" => (MARKET_HTTP, MARKET_GRPC, MARKET_WS),
            _ => (GATEWAY_HTTP, GATEWAY_GRPC, GATEWAY_WS),
        }
    }
}

/// Server configuration for all protocols
///
/// This struct holds the configuration for binding HTTP, gRPC, and WebSocket
/// servers. Each port is optional, allowing you to run only the protocols
/// you need.
///
/// # Example
///
/// ```
/// use server::config::ServerConfig;
///
/// // All protocols
/// let config = ServerConfig::new("0.0.0.0", 8080, 9080, 7080);
///
/// // HTTP only
/// let config = ServerConfig::http_only("127.0.0.1", 8080);
///
/// // For a specific service
/// let config = ServerConfig::for_service("gateway");
/// ```
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to (e.g., "0.0.0.0" or "127.0.0.1")
    pub host: String,
    /// Optional HTTP port
    pub http_port: Option<u16>,
    /// Optional gRPC port
    pub grpc_port: Option<u16>,
    /// Optional WebSocket port
    pub websocket_port: Option<u16>,
}

impl ServerConfig {
    /// Create a new server config with all ports
    pub fn new(host: impl Into<String>, http: u16, grpc: u16, ws: u16) -> Self {
        Self {
            host: host.into(),
            http_port: Some(http),
            grpc_port: Some(grpc),
            websocket_port: Some(ws),
        }
    }

    /// Create a server config for HTTP only
    pub fn http_only(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            http_port: Some(port),
            grpc_port: None,
            websocket_port: None,
        }
    }

    /// Create a server config for gRPC only
    pub fn grpc_only(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            http_port: None,
            grpc_port: Some(port),
            websocket_port: None,
        }
    }

    /// Create a server config for WebSocket only
    pub fn websocket_only(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            http_port: None,
            grpc_port: None,
            websocket_port: Some(port),
        }
    }

    /// Create a server config for specific service with default ports
    ///
    /// Uses the port assignments from the [`ports`] module.
    pub fn for_service(service_name: &str) -> Self {
        let (http, grpc, ws) = ports::for_service(service_name);
        Self::new("0.0.0.0", http, grpc, ws)
    }

    /// Get HTTP socket address
    pub fn http_addr(&self) -> Option<Result<SocketAddr>> {
        self.http_port.map(|p| self.parse_addr(p))
    }

    /// Get gRPC socket address
    pub fn grpc_addr(&self) -> Option<Result<SocketAddr>> {
        self.grpc_port.map(|p| self.parse_addr(p))
    }

    /// Get WebSocket socket address
    pub fn websocket_addr(&self) -> Option<Result<SocketAddr>> {
        self.websocket_port.map(|p| self.parse_addr(p))
    }

    /// Check if any servers are configured
    pub fn has_servers(&self) -> bool {
        self.http_port.is_some() || self.grpc_port.is_some() || self.websocket_port.is_some()
    }

    /// Parse an address from host and port
    fn parse_addr(&self, port: u16) -> Result<SocketAddr> {
        format!("{}:{}", self.host, port)
            .parse()
            .map_err(|_| ServerError::InvalidAddress(format!("{}:{}", self.host, port)))
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            http_port: Some(ports::GATEWAY_HTTP),
            grpc_port: Some(ports::GATEWAY_GRPC),
            websocket_port: Some(ports::GATEWAY_WS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_new() {
        let config = ServerConfig::new("127.0.0.1", 8080, 9080, 7080);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.grpc_port, Some(9080));
        assert_eq!(config.websocket_port, Some(7080));
    }

    #[test]
    fn test_server_config_http_only() {
        let config = ServerConfig::http_only("127.0.0.1", 8080);
        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.grpc_port, None);
        assert_eq!(config.websocket_port, None);
    }

    #[test]
    fn test_server_config_for_service() {
        let config = ServerConfig::for_service("gateway");
        assert_eq!(config.http_port, Some(8080));

        let config = ServerConfig::for_service("risk");
        assert_eq!(config.http_port, Some(8086));

        let config = ServerConfig::for_service("wallet");
        assert_eq!(config.http_port, Some(8084));
    }

    #[test]
    fn test_ports_for_service() {
        assert_eq!(ports::for_service("gateway"), (8080, 9080, 7080));
        assert_eq!(ports::for_service("GATEWAY"), (8080, 9080, 7080)); // case insensitive
        assert_eq!(ports::for_service("risk"), (8086, 9086, 7086));
        assert_eq!(ports::for_service("unknown"), (8080, 9080, 7080)); // defaults to gateway
    }
}
