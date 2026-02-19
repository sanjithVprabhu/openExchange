//! Settlement error types

use thiserror::Error;

/// Errors that can occur during settlement operations
#[derive(Error, Debug)]
pub enum SettlementError {
    /// Trade not found
    #[error("Trade not found: {0}")]
    TradeNotFound(String),

    /// Settlement failed
    #[error("Settlement failed: {0}")]
    SettlementFailed(String),

    /// Insufficient balance
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),

    /// Position not found
    #[error("Position not found: {0}")]
    PositionNotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}
