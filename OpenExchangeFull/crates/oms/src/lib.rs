//! Order Management System for OpenExchange
//!
//! This crate handles order lifecycle management.
//!
//! # Features
//!
//! - Order creation and validation
//! - Order status tracking
//! - Risk engine integration
//! - Matching engine integration
//! - Order modification and cancellation
//! - Order history and fills
//!
//! # Feature Flags
//!
//! - `postgres` - Enable PostgreSQL storage
//! - `api` - Enable HTTP API
//! - `client` - Enable HTTP clients for external services

pub mod types;
pub mod error;
pub mod store;
pub mod clients;
pub mod manager;

#[cfg(feature = "api")]
pub mod api;

// Re-export commonly used types
pub use types::{Order, OrderFill, OrderStatus, Environment};
pub use error::{OmsError, Result};
pub use manager::OrderManager;

// Store exports
pub use store::traits::OrderStore;
pub use store::memory::InMemoryOrderStore;

#[cfg(feature = "postgres")]
pub use store::postgres::PostgresOrderStore;

// Client exports
pub use clients::risk::{RiskClient, RiskCheckResult, MockRiskClient};
pub use clients::matching::{MatchingClient, MockMatchingClient};

#[cfg(feature = "client")]
pub use clients::risk::http::HttpRiskClient;

#[cfg(feature = "client")]
pub use clients::matching::http::HttpMatchingClient;
