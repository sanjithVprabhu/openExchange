//! Store traits for the Matching Engine
//!
//! This module defines the trait that all store implementations must satisfy.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{BookOrder, OrderBook, Trade};
use crate::engine::MatchingEngine;
use crate::event::MatchingEvent;
use crate::result::MatchResult;

/// Errors that can occur in the store
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),
    
    #[error("Instrument not found: {0}")]
    InstrumentNotFound(String),
    
    #[error("Redis error: {0}")]
    RedisError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Store error: {0}")]
    Other(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

/// Trait for order matching storage
///
/// This trait defines the interface for storing order books and trades.
/// Implementations can be in-memory, Redis, or any other storage backend.
#[async_trait]
pub trait MatchingStore: Send + Sync {
    // ------------------------------------------------------------------------
    // Order Operations
    // ------------------------------------------------------------------------
    
    /// Submit an order to the matching engine
    ///
    /// This will:
    /// 1. Add the order to the in-memory engine
    /// 2. Run matching
    /// 3. Return the result (trades + remaining order)
    async fn submit_order(&self, order: BookOrder) -> StoreResult<MatchResult>;
    
    /// Cancel an order from the book
    async fn cancel_order(&self, instrument_id: &str, order_id: Uuid) -> StoreResult<Option<BookOrder>>;
    
    // ------------------------------------------------------------------------
    // Book Queries
    // ------------------------------------------------------------------------
    
    /// Get the current order book for an instrument
    async fn get_book(&self, instrument_id: &str) -> StoreResult<Option<OrderBook>>;
    
    /// Get best bid price for an instrument
    async fn get_best_bid(&self, instrument_id: &str) -> StoreResult<Option<f64>>;
    
    /// Get best ask price for an instrument
    async fn get_best_ask(&self, instrument_id: &str) -> StoreResult<Option<f64>>;
    
    /// Get spread for an instrument
    async fn get_spread(&self, instrument_id: &str) -> StoreResult<Option<f64>>;
    
    /// Check if an instrument has an order book
    async fn has_book(&self, instrument_id: &str) -> StoreResult<bool>;
    
    /// List all instruments with order books
    async fn instruments(&self) -> StoreResult<Vec<String>>;
    
    // ------------------------------------------------------------------------
    // Trade Queries
    // ------------------------------------------------------------------------
    
    /// Get recent trades for an instrument
    async fn get_trades(&self, instrument_id: &str, limit: u32) -> StoreResult<Vec<Trade>>;
    
    // ------------------------------------------------------------------------
    // Event Log (for determinism)
    // ------------------------------------------------------------------------
    
    /// Append an event to the event log
    async fn append_event(&self, event: MatchingEvent) -> StoreResult<()>;
    
    /// Get events from a sequence number onwards
    async fn get_events(&self, from_sequence: u64) -> StoreResult<Vec<MatchingEvent>>;
    
    /// Get current sequence number
    async fn get_sequence(&self) -> StoreResult<u64>;
    
    // ------------------------------------------------------------------------
    // Engine Access (for advanced operations)
    // ------------------------------------------------------------------------
    
    /// Get a clone of the internal matching engine
    ///
    /// Note: This is a heavy operation and should be used sparingly.
    fn engine(&self) -> MatchingEngine;

    /// Get internal engine reference (for read operations)
    fn engine_ref(&self) -> &MatchingEngine;
}
