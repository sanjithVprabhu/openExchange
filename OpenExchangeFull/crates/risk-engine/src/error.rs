//! Risk engine error types

use thiserror::Error;

/// Errors that can occur during risk calculations
#[derive(Error, Debug)]
pub enum RiskError {
    /// Insufficient margin
    #[error("Insufficient margin: required {required}, available {available}")]
    InsufficientMargin { required: f64, available: f64 },

    /// Position limit exceeded
    #[error("Position limit exceeded: {0}")]
    PositionLimitExceeded(String),

    /// Invalid position
    #[error("Invalid position: {0}")]
    InvalidPosition(String),

    /// Calculation error
    #[error("Calculation error: {0}")]
    Calculation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}
