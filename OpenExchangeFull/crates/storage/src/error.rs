//! Storage error types

use thiserror::Error;

/// Errors that can occur during storage operations
#[derive(Error, Debug)]
pub enum StorageError {
    /// Connection error
    #[error("Connection error: {0}")]
    Connection(String),

    /// Query error
    #[error("Query error: {0}")]
    Query(String),

    /// Record not found
    #[error("Record not found: {0}")]
    NotFound(String),

    /// Duplicate record
    #[error("Duplicate record: {0}")]
    Duplicate(String),

    /// Transaction error
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),
}
