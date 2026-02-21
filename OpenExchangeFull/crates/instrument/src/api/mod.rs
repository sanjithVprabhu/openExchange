//! HTTP API for instrument management.
//!
//! Provides RESTful endpoints for querying instruments, checking status,
//! and admin operations like force regeneration.
//!
//! ## Modules
//!
//! - `handlers` - Direct HTTP handlers (for monolith/instrument mode)
//! - `routes` - Axum router with direct handlers
//! - `forwarding` - Forwarding handlers (for gateway mode)
//! - `forwarding_routes` - Axum router that forwards to instrument service
//! - `models` - Request/response types

pub mod handlers;
pub mod models;
pub mod routes;
pub mod forwarding;
pub mod forwarding_routes;

pub use routes::instrument_routes;
pub use forwarding_routes::instrument_forwarding_routes;
pub use forwarding::{ForwardingState, InstrumentForwarder};
