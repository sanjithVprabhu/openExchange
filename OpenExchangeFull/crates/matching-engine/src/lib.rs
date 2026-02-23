//! Order Matching Engine for OpenExchange
//!
//! This crate implements the core order matching logic for the exchange.
//!
//! # Features
//!
//! - Price-time priority matching (FIFO)
//! - Support for GTC, IOC, FOK time-in-force
//! - In-memory and Redis storage backends
//! - Deterministic event log for crash recovery
//! - Atomic trade execution
//!
//! # Architecture
//!
//! The matching engine is designed as a pure function:
//! `(old_state, order) -> (new_state, trades)`
//!
//! This ensures determinism - given the same inputs, the engine
//! always produces the same outputs.
//!
//! ## Core Components
//!
//! - [`domain`] - Core types (Trade, BookOrder, OrderBook)
//! - [`engine`] - Core matching algorithm
//! - [`store`] - Storage backends (in-memory, Redis)
//! - [`event`] - Event types for the event log
//!
//! # Example
//!
//! ```rust
//! use matching_engine::{domain::*, engine::*, store::*};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create store
//!     let store = create_store(StoreType::InMemory, None)
//!         .await
//!         .unwrap();
//!
//!     // Create and submit an order
//!     let order = BookOrder::new(
//!         uuid::Uuid::new_v4(),
//!         uuid::Uuid::new_v4(),
//!         OrderSide::Buy,
//!         100.0,
//!         10,
//!         1,
//!         TimeInForce::Gtc,
//!     );
//!
//!     let result = store.submit_order(order).await.unwrap();
//!     println!("Trades: {:?}", result.trades.len());
//! }
//! ```

pub mod domain;
pub mod engine;
pub mod result;
pub mod event;
pub mod log;
pub mod store;
pub mod error;
pub mod circuit_breaker;
pub mod metrics;

#[cfg(feature = "api")]
pub mod api;

pub use domain::{
    BookOrder, OrderBook, OrderSide, PriceLevel, TimeInForce, Trade, OrderBookSnapshot,
};
pub use engine::MatchingEngine;
pub use result::{CancelResult, MatchResult};
pub use event::MatchingEvent;
pub use store::{
    create_store, create_store_from_config, InMemoryStore, MatchingStore, RedisStore, StoreError, StoreResult, StoreType,
};
pub use circuit_breaker::{CircuitBreakerManager, CircuitBreakerConfig, CircuitBreakerStatus};
pub use metrics::{MatchingEngineMetrics, MetricsSnapshot};

pub use error::MatchingError;
