//! Common types used across OpenExchange
//!
//! This module provides the fundamental domain types used throughout
//! the exchange system.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    /// Create a new random OrderId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an OrderId from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for trades
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeId(pub Uuid);

impl TradeId {
    /// Create a new random TradeId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TradeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TradeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

impl Side {
    /// Returns the opposite side
    pub fn opposite(&self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    /// Returns true if this is a buy order
    pub fn is_buy(&self) -> bool {
        matches!(self, Side::Buy)
    }

    /// Returns true if this is a sell order
    pub fn is_sell(&self) -> bool {
        matches!(self, Side::Sell)
    }
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "buy"),
            Side::Sell => write!(f, "sell"),
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    /// Market order - execute immediately at best available price
    Market,
    /// Limit order - execute at specified price or better
    Limit,
    /// Stop market order - becomes market order when stop price is reached
    StopMarket,
    /// Stop limit order - becomes limit order when stop price is reached
    StopLimit,
}

impl std::fmt::Display for OrderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderType::Market => write!(f, "market"),
            OrderType::Limit => write!(f, "limit"),
            OrderType::StopMarket => write!(f, "stop_market"),
            OrderType::StopLimit => write!(f, "stop_limit"),
        }
    }
}

/// Time in force for orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeInForce {
    /// Good till cancelled - remains active until filled or cancelled
    #[serde(alias = "gtc")]
    #[default]
    Gtc,
    /// Immediate or cancel - fill immediately or cancel unfilled portion
    #[serde(alias = "ioc")]
    Ioc,
    /// Fill or kill - fill entirely or cancel entirely
    #[serde(alias = "fok")]
    Fok,
    /// Day order - expires at end of trading day
    #[serde(alias = "day")]
    Day,
}

impl std::fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeInForce::Gtc => write!(f, "GTC"),
            TimeInForce::Ioc => write!(f, "IOC"),
            TimeInForce::Fok => write!(f, "FOK"),
            TimeInForce::Day => write!(f, "DAY"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order has been created but not yet submitted
    Pending,
    /// Order has been submitted and is open
    Open,
    /// Order has been partially filled
    PartiallyFilled,
    /// Order has been completely filled
    Filled,
    /// Order has been cancelled
    Cancelled,
    /// Order has been rejected
    Rejected,
    /// Order has expired
    Expired,
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "pending"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Rejected => write!(f, "rejected"),
            OrderStatus::Expired => write!(f, "expired"),
        }
    }
}

/// Asset symbol (e.g., "BTC", "ETH")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    /// Create a new Symbol
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().to_uppercase())
    }

    /// Get the symbol as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Trading pair (e.g., "BTC-USD", "ETH-USDT")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradingPair {
    /// Base asset (e.g., BTC in BTC-USD)
    pub base: Symbol,
    /// Quote asset (e.g., USD in BTC-USD)
    pub quote: Symbol,
}

impl TradingPair {
    /// Create a new trading pair
    pub fn new(base: impl Into<Symbol>, quote: impl Into<Symbol>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }

    /// Get the pair as a string (e.g., "BTC-USD")
    pub fn as_string(&self) -> String {
        format!("{}-{}", self.base, self.quote)
    }
}

impl std::fmt::Display for TradingPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.base, self.quote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_id() {
        let id1 = OrderId::new();
        let id2 = OrderId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_side() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
        assert!(Side::Buy.is_buy());
        assert!(Side::Sell.is_sell());
    }

    #[test]
    fn test_symbol() {
        let sym = Symbol::new("btc");
        assert_eq!(sym.as_str(), "BTC");
    }

    #[test]
    fn test_trading_pair() {
        let pair = TradingPair::new("BTC", "USD");
        assert_eq!(pair.as_string(), "BTC-USD");
    }
}
