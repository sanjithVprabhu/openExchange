//! Market data error types

use thiserror::Error;

/// Errors that can occur during market data operations
#[derive(Error, Debug)]
pub enum MarketDataError {
    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Provider error
    #[error("Provider error: {0}")]
    Provider(String),

    /// Invalid symbol
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),

    /// Data not available
    #[error("Data not available: {0}")]
    DataNotAvailable(String),

    /// Subscription error
    #[error("Subscription error: {0}")]
    Subscription(String),

    /// Invalid input for calculation
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Calculation error
    #[error("Calculation error: {0}")]
    CalculationError(String),
}

/// Result type for market data operations
pub type Result<T> = std::result::Result<T, MarketDataError>;
