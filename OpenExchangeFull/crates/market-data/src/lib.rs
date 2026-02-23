//! Market Data Feeds for OpenExchange
//!
//! This crate provides market data, pricing, and aggregation services.
//!
//! # Core Components
//!
//! - [`black_scholes`] - Black-Scholes option pricing engine with Greeks
//! - [`vol_surface`] - Volatility surface construction and management
//! - [`mark_price`] - Mark price calculation with EMA smoothing
//! - [`order_book`] - Order book snapshot aggregation
//! - [`index_price`] - Multi-source index price aggregation
//! - [`candles`] - OHLCV candle generation
//! - [`coordinator`] - Unified market data coordinator
//!
//! # Key Invariants
//!
//! - Market Data NEVER writes state - it's a pure projection engine
//! - Mark price ≠ last trade (prevents manipulation)
//! - Greeks are required for Risk Engine margin calculations
//! - Volatility surface prevents single-trade manipulation
//! - Index prices use median aggregation with outlier rejection

pub mod black_scholes;
pub mod candles;
pub mod coordinator;
pub mod error;
pub mod index_price;
pub mod mark_price;
pub mod order_book;
pub mod types;
pub mod vol_surface;

pub use coordinator::MarketDataCoordinator;
pub use error::MarketDataError;
pub use types::{
    Greeks, IndexPrice, MarkPrice, OptionType, OrderBookSnapshot, PriceLevel, Trade, BSInputs,
};

pub type Result<T> = std::result::Result<T, MarketDataError>;
