//! Observability infrastructure for OpenExchange
//!
//! This crate provides:
//! - Structured logging via tracing
//! - Prometheus metrics
//! - Server-specific metric helpers
//!
//! # Quick Start
//!
//! ```ignore
//! use observability::{init_logging, LogFormat};
//!
//! // Initialize logging
//! init_logging("my-service", LogFormat::Pretty)?;
//!
//! // Initialize metrics (optional)
//! observability::metrics::init_metrics(9090)?;
//! ```

pub mod logging;
pub mod metrics;

pub use logging::{init_logging, LogFormat};
pub use metrics::{init_metrics, ServerMetrics};
