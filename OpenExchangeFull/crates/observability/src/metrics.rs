//! Prometheus metrics infrastructure
//!
//! This module provides utilities for initializing Prometheus metrics
//! and creating server-specific metric sets.

use metrics::{counter, gauge, histogram, Counter, Gauge, Histogram};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;
use std::time::Duration;

/// Initialize the Prometheus metrics exporter
///
/// This starts an HTTP server on the specified port that exposes metrics
/// at the `/metrics` endpoint.
///
/// # Arguments
///
/// * `port` - Port to expose metrics on
///
/// # Example
///
/// ```ignore
/// observability::metrics::init_metrics(9090)?;
/// // Metrics available at http://localhost:9090/metrics
/// ```
pub fn init_metrics(port: u16) -> anyhow::Result<()> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;

    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()?;

    tracing::info!(%addr, "Metrics server listening");
    Ok(())
}

/// Server-specific metrics
///
/// This struct provides a set of metrics for monitoring server performance.
/// Each server should create its own instance with its name.
///
/// # Metrics
///
/// * `server_requests_total` - Total number of requests processed
/// * `server_request_duration_seconds` - Request duration histogram
/// * `server_active_connections` - Number of active connections
///
/// # Example
///
/// ```ignore
/// let metrics = ServerMetrics::new("http");
///
/// // Record a request
/// metrics.record_request(Duration::from_millis(50), 200);
///
/// // Track connections
/// metrics.connection_opened();
/// // ... handle connection ...
/// metrics.connection_closed();
/// ```
#[derive(Clone)]
pub struct ServerMetrics {
    requests_total: Counter,
    requests_by_status: fn(u16) -> Counter,
    request_duration: Histogram,
    active_connections: Gauge,
    server_name: String,
}

impl ServerMetrics {
    /// Create metrics for a specific server
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the server (e.g., "http", "grpc", "websocket")
    pub fn new(server_name: &str) -> Self {
        let name = server_name.to_string();
        
        Self {
            requests_total: counter!("server_requests_total", "server" => name.clone()),
            requests_by_status: |status| {
                counter!("server_requests_by_status", "status" => status.to_string())
            },
            request_duration: histogram!("server_request_duration_seconds", "server" => name.clone()),
            active_connections: gauge!("server_active_connections", "server" => name.clone()),
            server_name: name,
        }
    }

    /// Record a completed request
    ///
    /// # Arguments
    ///
    /// * `duration` - How long the request took
    /// * `status_code` - HTTP status code (or gRPC status, etc.)
    pub fn record_request(&self, duration: Duration, status_code: u16) {
        self.requests_total.increment(1);
        (self.requests_by_status)(status_code).increment(1);
        self.request_duration.record(duration.as_secs_f64());
    }

    /// Update active connection count to a specific value
    pub fn set_active_connections(&self, count: u64) {
        self.active_connections.set(count as f64);
    }

    /// Increment active connections (call when a connection is opened)
    pub fn connection_opened(&self) {
        self.active_connections.increment(1.0);
    }

    /// Decrement active connections (call when a connection is closed)
    pub fn connection_closed(&self) {
        self.active_connections.decrement(1.0);
    }

    /// Get the server name
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

/// Request metrics guard that automatically records duration on drop
///
/// # Example
///
/// ```ignore
/// let metrics = ServerMetrics::new("http");
/// {
///     let _guard = RequestMetricsGuard::new(&metrics);
///     // ... handle request ...
/// } // Duration automatically recorded when guard is dropped
/// ```
pub struct RequestMetricsGuard<'a> {
    metrics: &'a ServerMetrics,
    start: std::time::Instant,
    status_code: u16,
}

impl<'a> RequestMetricsGuard<'a> {
    /// Create a new metrics guard
    pub fn new(metrics: &'a ServerMetrics) -> Self {
        Self {
            metrics,
            start: std::time::Instant::now(),
            status_code: 200,
        }
    }

    /// Set the status code (call before drop)
    pub fn set_status(&mut self, code: u16) {
        self.status_code = code;
    }
}

impl Drop for RequestMetricsGuard<'_> {
    fn drop(&mut self) {
        self.metrics.record_request(self.start.elapsed(), self.status_code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_metrics_creation() {
        // Just verify it doesn't panic
        let metrics = ServerMetrics::new("test");
        assert_eq!(metrics.server_name(), "test");
    }
}
