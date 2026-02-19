//! Port validation utilities
//!
//! Note: Port validation before binding has an inherent TOCTOU (time-of-check-time-of-use)
//! race condition. Between checking and actually binding, another process could take the port.
//! These utilities are useful for early feedback but should not be relied upon for correctness.
//! The actual bind operation is the source of truth.

use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

use crate::config::ServerConfig;
use crate::error::{Result, ServerError};

/// Validate that all configured ports are available
///
/// This performs an async check of port availability. Note that there's a race condition
/// between checking and actually binding - another process could grab the port in between.
/// This is useful for early user feedback but the actual bind is what matters.
pub async fn validate_ports_available(config: &ServerConfig) -> Result<()> {
    info!("Validating server ports...");

    let mut ports_to_check = Vec::new();

    if let Some(port) = config.http_port {
        ports_to_check.push(("HTTP", port));
    }
    if let Some(port) = config.grpc_port {
        ports_to_check.push(("gRPC", port));
    }
    if let Some(port) = config.websocket_port {
        ports_to_check.push(("WebSocket", port));
    }

    if ports_to_check.is_empty() {
        warn!("No ports configured for server");
        return Ok(());
    }

    for (protocol, port) in ports_to_check {
        validate_single_port(&config.host, port, protocol).await?;
    }

    info!("All server ports validated successfully");
    Ok(())
}

/// Validate a single port is available
async fn validate_single_port(host: &str, port: u16, protocol: &str) -> Result<()> {
    let addr = format!("{}:{}", host, port);
    debug!("Checking {} port {}", protocol, port);

    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            let local_addr = listener
                .local_addr()
                .map_err(|e| ServerError::bind(addr.clone(), e))?;

            // Drop the listener to release the port
            drop(listener);

            info!("{} port {} is available ({})", protocol, port, local_addr);
            Ok(())
        }
        Err(e) => {
            error!("{} port {} is NOT available: {}", protocol, port, e);
            Err(ServerError::port_in_use(port, e.to_string()))
        }
    }
}

/// Check if a port is in use (async version)
///
/// Returns `true` if the port appears to be in use, `false` if it's available.
/// Note: This is subject to TOCTOU race conditions.
pub async fn is_port_in_use(host: &str, port: u16) -> bool {
    let addr = format!("{}:{}", host, port);
    TcpListener::bind(&addr).await.is_err()
}

/// Get next available port starting from given port (async version)
///
/// Scans ports starting from `start_port` and returns the first available one.
/// Note: This is subject to TOCTOU race conditions.
pub async fn find_available_port(host: &str, start_port: u16) -> Option<u16> {
    for port in start_port..=65535 {
        if !is_port_in_use(host, port).await {
            return Some(port);
        }
    }
    None
}

/// Validate port range
///
/// Checks if the port number is valid:
/// - Port 0 is rejected (ephemeral port assignment)
/// - Ports below 1024 generate a warning (privileged ports)
pub fn validate_port_range(port: u16) -> Result<()> {
    if port == 0 {
        Err(ServerError::ConfigError(
            "Port cannot be 0 (ephemeral port assignment not supported for explicit binding)"
                .to_string(),
        ))
    } else if port < 1024 {
        warn!(
            "Port {} is a privileged port (requires root/admin privileges)",
            port
        );
        Ok(())
    } else {
        Ok(())
    }
}

/// Validate all ports in a configuration
pub fn validate_config_ports(config: &ServerConfig) -> Result<()> {
    if let Some(port) = config.http_port {
        validate_port_range(port)?;
    }
    if let Some(port) = config.grpc_port {
        validate_port_range(port)?;
    }
    if let Some(port) = config.websocket_port {
        validate_port_range(port)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_port_range() {
        assert!(validate_port_range(0).is_err());
        assert!(validate_port_range(80).is_ok()); // Warning but OK
        assert!(validate_port_range(8080).is_ok());
        assert!(validate_port_range(65535).is_ok());
    }

    #[tokio::test]
    async fn test_is_port_in_use() {
        // Bind to a port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Port should be in use
        assert!(is_port_in_use("127.0.0.1", port).await);

        // Drop the listener
        drop(listener);

        // Port should now be available (though there's a small race window)
        assert!(!is_port_in_use("127.0.0.1", port).await);
    }

    #[tokio::test]
    async fn test_find_available_port() {
        // Should find an available port starting from a high number
        let port = find_available_port("127.0.0.1", 50000).await;
        assert!(port.is_some());
        assert!(port.unwrap() >= 50000);
    }
}
