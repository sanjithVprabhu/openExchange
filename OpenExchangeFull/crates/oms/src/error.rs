//! OMS error types

use thiserror::Error;
use uuid::Uuid;

/// Errors that can occur in the Order Management System
#[derive(Error, Debug)]
pub enum OmsError {
    /// Invalid order
    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    /// Order not found
    #[error("Order not found: {0}")]
    NotFound(Uuid),

    /// Order already exists
    #[error("Order already exists: {0}")]
    OrderExists(Uuid),

    /// Order cannot be modified
    #[error("Order cannot be modified: {0}")]
    OrderNotModifiable(String),

    /// Order cannot be cancelled
    #[error("Order cannot be cancelled: {0}")]
    OrderNotCancellable(String),

    /// Invalid order state transition
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Validation error
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Instrument not found
    #[error("Instrument not found: {0}")]
    InstrumentNotFound(String),

    /// Instrument not tradable
    #[error("Instrument not tradable: {0}")]
    InstrumentNotTradable(String),

    /// Risk rejected
    #[error("Risk rejected: {0}")]
    RiskRejected(String),

    /// Risk engine unavailable
    #[error("Risk engine unavailable: {0}")]
    RiskUnavailable(String),

    /// Matching engine unavailable
    #[error("Matching engine unavailable: {0}")]
    MatchingUnavailable(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for OMS operations
pub type Result<T> = std::result::Result<T, OmsError>;
