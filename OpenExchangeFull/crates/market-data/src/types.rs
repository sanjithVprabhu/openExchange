//! Shared types for Market Data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Option type (Call or Put)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

/// Inputs for Black-Scholes pricing
#[derive(Debug, Clone, Copy)]
pub struct BSInputs {
    /// Spot price (index price of underlying)
    pub spot: f64,
    /// Strike price
    pub strike: f64,
    /// Time to expiry (in years)
    pub time: f64,
    /// Implied volatility (as decimal, e.g., 0.5 = 50%)
    pub vol: f64,
    /// Risk-free rate (typically ~0 for crypto)
    pub rate: f64,
    /// Option type
    pub option_type: OptionType,
}

impl BSInputs {
    /// Validate and clamp inputs to safe ranges
    pub fn validate(&mut self) {
        self.time = self.time.max(1.0 / (365.25 * 24.0 * 3600.0));
        self.vol = self.vol.clamp(0.01, 5.0);
        self.spot = self.spot.max(1e-6);
        self.strike = self.strike.max(1e-6);
    }
}

/// Option Greeks
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Greeks {
    /// Delta: ∂V/∂S (rate of change with spot)
    pub delta: f64,
    /// Gamma: ∂²V/∂S² (curvature of delta)
    pub gamma: f64,
    /// Vega: ∂V/∂σ (sensitivity to volatility)
    pub vega: f64,
    /// Theta: ∂V/∂t (time decay)
    pub theta: f64,
    /// Rho: ∂V/∂r (sensitivity to interest rate)
    pub rho: f64,
}

/// Price level in order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub quantity: u32,
    pub order_count: usize,
}

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub instrument_id: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
}

/// Trade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub trade_id: String,
    pub instrument_id: String,
    pub price: f64,
    pub quantity: u32,
    pub aggressor_side: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Index price for an underlying asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexPrice {
    pub asset: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
}

/// Mark price data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkPrice {
    pub instrument_id: String,
    pub mark_price: f64,
    pub index_price: f64,
    pub implied_vol: f64,
    pub timestamp: DateTime<Utc>,
}
