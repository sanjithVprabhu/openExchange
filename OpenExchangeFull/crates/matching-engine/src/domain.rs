//! Domain types for the Matching Engine
//!
//! This module defines the core domain types used in the matching engine.
//! These types are shared across all implementations (in-memory, Redis).

use chrono::{DateTime, Utc};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use uuid::Uuid;

// ============================================================================
// Order Side
// ============================================================================

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

impl OrderSide {
    /// Returns the opposite side
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }

    /// Returns true if this is a buy order
    pub fn is_buy(&self) -> bool {
        matches!(self, OrderSide::Buy)
    }
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "buy"),
            OrderSide::Sell => write!(f, "sell"),
        }
    }
}

// ============================================================================
// Time In Force
// ============================================================================

/// Time in force determines how long an order stays active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeInForce {
    /// Good Till Cancel - remains in book until filled or cancelled
    Gtc,
    /// Immediate or Cancel - fill what possible, cancel remainder
    Ioc,
    /// Fill or Kill - entire order must fill or cancel completely
    Fok,
}

impl Default for TimeInForce {
    fn default() -> Self {
        Self::Gtc
    }
}

// ============================================================================
// Book Order
// ============================================================================

/// Order in the matching engine's order book
///
/// This is a simplified view - the full Order lives in OMS.
/// Matching engine only needs what's required for price-time priority.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookOrder {
    /// Order ID
    pub order_id: Uuid,
    /// User who placed order
    pub user_id: Uuid,
    /// Instrument being traded
    pub instrument_id: String,
    /// Buy or Sell
    pub side: OrderSide,
    /// Price (for limit orders)
    pub price: f64,
    /// Remaining quantity to fill
    pub quantity: u32,
    /// Sequence number (determines time priority)
    pub sequence: u64,
    /// Time-in-force
    pub time_in_force: TimeInForce,
}

impl BookOrder {
    /// Create a new book order
    pub fn new(
        order_id: Uuid,
        user_id: Uuid,
        side: OrderSide,
        price: f64,
        quantity: u32,
        sequence: u64,
        time_in_force: TimeInForce,
    ) -> Self {
        Self {
            order_id,
            user_id,
            instrument_id: String::new(),
            side,
            price,
            quantity,
            sequence,
            time_in_force,
        }
    }

    /// Set the instrument ID for this order
    pub fn with_instrument_id(mut self, instrument_id: impl Into<String>) -> Self {
        self.instrument_id = instrument_id.into();
        self
    }

    /// Reduce quantity after partial fill
    pub fn fill(&mut self, qty: u32) {
        self.quantity = self.quantity.saturating_sub(qty);
    }

    /// Check if order is completely filled
    pub fn is_filled(&self) -> bool {
        self.quantity == 0
    }

    /// Get remaining quantity
    pub fn remaining_quantity(&self) -> u32 {
        self.quantity
    }
}

// ============================================================================
// Order Book
// ============================================================================

/// Order book for a single instrument
///
/// CRITICAL PROPERTIES:
/// 1. Bids sorted descending (highest price first)
/// 2. Asks sorted ascending (lowest price first)
/// 3. Each price level is FIFO queue
/// 4. Deterministic iteration order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    /// Instrument this book is for
    pub instrument_id: String,
    /// Buy orders (price → FIFO queue)
    /// BTreeMap ensures deterministic iteration (descending)
    #[serde(skip)]
    pub bids: BTreeMap<std::cmp::Reverse<OrderedFloat<f64>>, VecDeque<BookOrder>>,
    /// Sell orders (price → FIFO queue)
    /// BTreeMap ensures deterministic iteration (ascending)
    #[serde(skip)]
    pub asks: BTreeMap<OrderedFloat<f64>, VecDeque<BookOrder>>,
    /// Sequence counter for this book
    pub sequence: u64,
}

impl OrderBook {
    /// Create a new order book
    pub fn new(instrument_id: String) -> Self {
        Self {
            instrument_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            sequence: 0,
        }
    }

    /// Get best bid price (highest buy)
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next().map(|k| k.0 .0)
    }

    /// Get best ask price (lowest sell)
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|k| k.0)
    }

    /// Get spread
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get total quantity at bid price level
    pub fn bid_quantity_at(&self, price: f64) -> u32 {
        self.bids
            .get(&std::cmp::Reverse(OrderedFloat(price)))
            .map(|orders| orders.iter().map(|o| o.quantity).sum())
            .unwrap_or(0)
    }

    /// Get total quantity at ask price level
    pub fn ask_quantity_at(&self, price: f64) -> u32 {
        self.asks
            .get(&OrderedFloat(price))
            .map(|orders| orders.iter().map(|o| o.quantity).sum())
            .unwrap_or(0)
    }

    /// Insert order into book
    pub fn insert_order(&mut self, order: BookOrder) {
        match order.side {
            OrderSide::Buy => {
                self.bids
                    .entry(std::cmp::Reverse(OrderedFloat(order.price)))
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
            OrderSide::Sell => {
                self.asks
                    .entry(OrderedFloat(order.price))
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
        }
    }

    /// Remove order by ID
    pub fn remove_order(&mut self, order_id: Uuid) -> Option<BookOrder> {
        // Search bids
        for (_, queue) in self.bids.iter_mut() {
            if let Some(pos) = queue.iter().position(|o| o.order_id == order_id) {
                return queue.remove(pos);
            }
        }

        // Search asks
        for (_, queue) in self.asks.iter_mut() {
            if let Some(pos) = queue.iter().position(|o| o.order_id == order_id) {
                return queue.remove(pos);
            }
        }

        None
    }

    /// Clean up empty price levels
    pub fn cleanup_empty_levels(&mut self) {
        self.bids.retain(|_, queue| !queue.is_empty());
        self.asks.retain(|_, queue| !queue.is_empty());
    }

    /// Check if book is empty
    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    /// Get total number of orders in book
    pub fn order_count(&self) -> usize {
        self.bids.values().map(|q| q.len()).sum::<usize>()
            + self.asks.values().map(|q| q.len()).sum::<usize>()
    }

    /// Calculate total available ask quantity at or below a given price
    /// Used for FOK pre-check on buy orders
    pub fn available_ask_quantity_at_or_below(&self, max_price: f64) -> u32 {
        self.asks
            .iter()
            .take_while(|(price, _)| price.0 <= max_price)
            .flat_map(|(_, orders)| orders.iter())
            .map(|o| o.quantity)
            .sum()
    }

    /// Calculate total available bid quantity at or above a given price
    /// Used for FOK pre-check on sell orders
    pub fn available_bid_quantity_at_or_above(&self, min_price: f64) -> u32 {
        self.bids
            .iter()
            .take_while(|(price, _)| price.0 .0 >= min_price)
            .flat_map(|(_, orders)| orders.iter())
            .map(|o| o.quantity)
            .sum()
    }
}

// ============================================================================
// Trade
// ============================================================================

/// Trade represents a matched execution between two orders
///
/// CRITICAL: This is the atomic unit of execution.
/// Either fully recorded or not recorded at all.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Unique trade identifier
    pub trade_id: Uuid,
    /// Instrument being traded
    pub instrument_id: String,
    /// Order that took liquidity (aggressor)
    pub taker_order_id: Uuid,
    /// Order that provided liquidity (resting)
    pub maker_order_id: Uuid,
    /// User IDs involved
    pub buyer_id: Uuid,
    pub seller_id: Uuid,
    /// Execution price (ALWAYS the maker's price)
    pub price: f64,
    /// Number of contracts traded
    pub quantity: u32,
    /// Which side was the aggressor
    pub aggressor_side: OrderSide,
    /// Sequence number (for deterministic ordering)
    pub sequence: u64,
    /// When trade occurred
    pub timestamp: DateTime<Utc>,
}

impl Trade {
    /// Create a new trade
    pub fn new(
        instrument_id: String,
        taker_order_id: Uuid,
        maker_order_id: Uuid,
        buyer_id: Uuid,
        seller_id: Uuid,
        price: f64,
        quantity: u32,
        aggressor_side: OrderSide,
        sequence: u64,
    ) -> Self {
        Self {
            trade_id: Uuid::new_v4(),
            instrument_id,
            taker_order_id,
            maker_order_id,
            buyer_id,
            seller_id,
            price,
            quantity,
            aggressor_side,
            sequence,
            timestamp: Utc::now(),
        }
    }
}

// ============================================================================
// Price Level (for market data)
// ============================================================================

/// Price level for market data snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price
    pub price: f64,
    /// Total quantity at this price
    pub quantity: u32,
    /// Number of orders at this price
    pub order_count: usize,
}

/// Order book snapshot for market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// Instrument ID
    pub instrument_id: String,
    /// Bid price levels (best first)
    pub bids: Vec<PriceLevel>,
    /// Ask price levels (best first)
    pub asks: Vec<PriceLevel>,
    /// Sequence number
    pub sequence: u64,
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
}

impl OrderBookSnapshot {
    /// Create snapshot from order book
    pub fn from_book(book: &OrderBook, depth: usize) -> Self {
        let bids: Vec<PriceLevel> = book
            .bids
            .iter()
            .take(depth)
            .map(|(price, orders)| PriceLevel {
                price: price.0 .0,
                quantity: orders.iter().map(|o| o.quantity).sum(),
                order_count: orders.len(),
            })
            .collect();

        let asks: Vec<PriceLevel> = book
            .asks
            .iter()
            .take(depth)
            .map(|(price, orders)| PriceLevel {
                price: price.0,
                quantity: orders.iter().map(|o| o.quantity).sum(),
                order_count: orders.len(),
            })
            .collect();

        Self {
            instrument_id: book.instrument_id.clone(),
            bids,
            asks,
            sequence: book.sequence,
            timestamp: Utc::now(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_side_opposite() {
        assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    }

    #[test]
    fn test_book_order_fill() {
        let mut order = BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            100.0,
            10,
            1,
            TimeInForce::Gtc,
        );

        assert_eq!(order.quantity, 10);
        assert!(!order.is_filled());

        order.fill(5);
        assert_eq!(order.quantity, 5);
        assert!(!order.is_filled());

        order.fill(5);
        assert_eq!(order.quantity, 0);
        assert!(order.is_filled());
    }

    #[test]
    fn test_order_book_insert_and_remove() {
        let mut book = OrderBook::new("BTC-50000-C".to_string());

        let order = BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            100.0,
            10,
            1,
            TimeInForce::Gtc,
        );

        let order_id = order.order_id;
        book.insert_order(order);

        assert_eq!(book.best_bid(), Some(100.0));
        assert_eq!(book.remove_order(order_id).unwrap().quantity, 10);
        assert!(book.is_empty());
    }

    #[test]
    fn test_order_book_spread() {
        let mut book = OrderBook::new("BTC-50000-C".to_string());

        // No spread when empty
        assert!(book.spread().is_none());

        // Add bid
        book.insert_order(BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Buy,
            95.0,
            10,
            1,
            TimeInForce::Gtc,
        ));
        assert!(book.spread().is_none());

        // Add ask
        book.insert_order(BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Sell,
            105.0,
            10,
            2,
            TimeInForce::Gtc,
        ));
        assert_eq!(book.spread(), Some(10.0));
    }
}
