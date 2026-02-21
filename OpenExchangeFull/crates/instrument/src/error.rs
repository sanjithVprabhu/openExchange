//! Error types for the instrument crate.

use thiserror::Error;

/// Result type alias for instrument operations.
pub type InstrumentResult<T> = Result<T, InstrumentError>;

/// Errors that can occur in instrument operations.
#[derive(Error, Debug, Clone)]
pub enum InstrumentError {
    /// Instrument not found.
    #[error("Instrument not found: {0}")]
    NotFound(String),

    /// Instrument already exists.
    #[error("Instrument already exists: {0}")]
    AlreadyExists(String),

    /// Invalid instrument symbol format.
    #[error("Invalid instrument symbol: {symbol}. Expected format: {expected}")]
    InvalidSymbol { symbol: String, expected: String },

    /// Invalid strike price.
    #[error("Invalid strike price: {0}. Strike must be positive.")]
    InvalidStrike(f64),

    /// Invalid expiry date.
    #[error("Invalid expiry date: {0}")]
    InvalidExpiry(String),

    /// Expiry date is in the past.
    #[error("Expiry date is in the past: {0}")]
    ExpiredInstrument(String),

    /// Asset not supported.
    #[error("Asset not supported: {0}")]
    UnsupportedAsset(String),

    /// Asset not enabled in config.
    #[error("Asset not enabled: {0}")]
    AssetNotEnabled(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Generation error.
    #[error("Failed to generate instruments: {0}")]
    GenerationError(String),

    /// Storage error.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for InstrumentError {
    fn from(err: serde_json::Error) -> Self {
        InstrumentError::SerializationError(err.to_string())
    }
}
