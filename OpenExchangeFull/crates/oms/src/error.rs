//! OMS error types

use thiserror::Error;

/// Errors that can occur in the Order Management System
#[derive(Error, Debug)]
pub enum OmsError {
    /// Invalid order
    #[error("Invalid order: {0}")]
    InvalidOrder(String),

    /// Order not found
    #[error("Order not found: {0}")]
    OrderNotFound(String),

    /// Order already exists
    #[error("Order already exists: {0}")]
    OrderExists(String),

    /// Order cannot be modified
    #[error("Order cannot be modified: {0}")]
    OrderNotModifiable(String),

    /// Order cannot be cancelled
    #[error("Order cannot be cancelled: {0}")]
    OrderNotCancellable(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}
