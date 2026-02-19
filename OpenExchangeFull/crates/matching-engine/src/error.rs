//! Matching engine error types

use thiserror::Error;

/// Errors that can occur during order matching
#[derive(Error, Debug)]
pub enum MatchingError {
    /// Invalid order
    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    /// Order not found
    #[error("Order not found: {0}")]
    OrderNotFound(String),

    /// Insufficient liquidity
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,

    /// Circuit breaker triggered
    #[error("Circuit breaker triggered: {0}")]
    CircuitBreaker(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}
