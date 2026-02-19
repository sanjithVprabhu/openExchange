//! Order Matching Engine for OpenExchange
//!
//! This crate implements the core order matching logic for the exchange.
//!
//! # Status
//!
//! **Placeholder** - Not yet implemented.
//!
//! # Future Features
//!
//! - Price-time priority matching
//! - Pro-rata matching
//! - Circuit breakers
//! - Order book management

pub mod error;

pub use error::MatchingError;

/// Result type for matching operations
pub type Result<T> = std::result::Result<T, MatchingError>;
