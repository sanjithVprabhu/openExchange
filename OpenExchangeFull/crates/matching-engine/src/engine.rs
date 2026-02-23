//! Core Matching Engine
//!
//! This module implements the deterministic price-time priority matching algorithm.

use crate::circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager, CircuitBreakerStatus};
use crate::domain::{BookOrder, OrderBook, OrderSide, TimeInForce, Trade};
use crate::metrics::{MatchingEngineMetrics, MetricsSnapshot};
use crate::result::MatchResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Matching Engine - The heart of the exchange
///
/// CRITICAL PROPERTIES:
/// 1. Deterministic (same inputs → same outputs, always)
/// 2. Pure function (no external state, no side effects)
/// 3. Price-time priority (strictly enforced)
/// 4. Per-instrument isolation (books never interact)
pub struct MatchingEngine {
    /// Order books per instrument
    books: HashMap<String, OrderBook>,
    /// Global sequence counter
    sequence: u64,
    /// Circuit breaker manager
    circuit_breakers: Option<CircuitBreakerManager>,
    /// Metrics collection
    metrics: Option<Arc<MatchingEngineMetrics>>,
}

impl MatchingEngine {
    /// Create a new matching engine with default settings (no circuit breakers)
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
            sequence: 0,
            circuit_breakers: None,
            metrics: None,
        }
    }

    /// Create a new matching engine with circuit breakers
    pub fn new_with_circuit_breakers(config: CircuitBreakerConfig) -> Self {
        Self {
            books: HashMap::new(),
            sequence: 0,
            circuit_breakers: Some(CircuitBreakerManager::new(config)),
            metrics: None,
        }
    }

    /// Create a new matching engine with metrics enabled
    pub fn new_with_metrics() -> Self {
        Self {
            books: HashMap::new(),
            sequence: 0,
            circuit_breakers: None,
            metrics: Some(Arc::new(MatchingEngineMetrics::new())),
        }
    }

    /// Create a new matching engine with both circuit breakers and metrics
    pub fn new_with_all(config: CircuitBreakerConfig) -> Self {
        Self {
            books: HashMap::new(),
            sequence: 0,
            circuit_breakers: Some(CircuitBreakerManager::new(config)),
            metrics: Some(Arc::new(MatchingEngineMetrics::new())),
        }
    }

    /// Enable metrics collection
    pub fn enable_metrics(&mut self) {
        self.metrics = Some(Arc::new(MatchingEngineMetrics::new()));
    }

    /// Get metrics snapshot
    pub fn metrics(&self) -> Option<MetricsSnapshot> {
        self.metrics.as_ref().map(|m| m.snapshot())
    }

    /// Enable circuit breakers with the given configuration
    pub fn enable_circuit_breakers(&mut self, config: CircuitBreakerConfig) {
        if config.enabled {
            self.circuit_breakers = Some(CircuitBreakerManager::new(config));
            info!("Circuit breakers enabled");
        }
    }

    /// Disable circuit breakers
    pub fn disable_circuit_breakers(&mut self) {
        self.circuit_breakers = None;
        info!("Circuit breakers disabled");
    }

    /// Check if an instrument is halted due to circuit breaker
    pub fn is_halted(&self, instrument_id: &str) -> bool {
        self.circuit_breakers
            .as_ref()
            .map(|cb| cb.is_halted(instrument_id))
            .unwrap_or(false)
    }

    /// Get circuit breaker status for all instruments
    pub fn circuit_breaker_status(&self) -> Vec<CircuitBreakerStatus> {
        self.circuit_breakers
            .as_ref()
            .map(|cb| cb.status())
            .unwrap_or_default()
    }

    /// Clear circuit breaker for an instrument
    pub fn clear_circuit_breaker(&mut self, instrument_id: &str) {
        if let Some(cb) = &mut self.circuit_breakers {
            cb.clear(instrument_id);
        }
    }

    /// Get or create order book for instrument
    fn get_or_create_book(&mut self, instrument_id: &str) -> &mut OrderBook {
        self.books
            .entry(instrument_id.to_string())
            .or_insert_with(|| OrderBook::new(instrument_id.to_string()))
    }

    /// Get next sequence number
    fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    /// Set sequence number (for replay)
    pub fn set_sequence(&mut self, seq: u64) {
        self.sequence = seq;
    }

    /// Get current sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Match a new order against the book
    ///
    /// This is the core matching algorithm:
    /// 1. Check FOK preconditions (liquidity check)
    /// 2. Check if order crosses with opposite side
    /// 3. Match greedily at best prices (FIFO within price level)
    /// 4. Generate trades
    /// 5. Update book
    /// 6. Handle remainder based on time-in-force
    pub fn match_order(&mut self, mut order: BookOrder) -> MatchResult {
        let start_time = Instant::now();
        
        // Record order received
        if let Some(ref metrics) = self.metrics {
            metrics.record_order_received();
        }

        info!(
            order_id = %order.order_id,
            instrument = %order.instrument_id,
            side = ?order.side,
            price = order.price,
            quantity = order.quantity,
            "Matching order"
        );

        let instrument_id = order.instrument_id.clone();

        // Circuit breaker check - reject if halted
        if let Some(ref mut cb) = self.circuit_breakers {
            if cb.is_halted(&instrument_id) {
                warn!(
                    order_id = %order.order_id,
                    instrument = %instrument_id,
                    "Order rejected: circuit breaker halted"
                );
                return MatchResult::cancelled(order);
            }
        }

        // FOK pre-check: validate liquidity BEFORE touching the book
        // Need to get book temporarily for FOK check
        if order.time_in_force == TimeInForce::Fok {
            let book = self.get_or_create_book(&instrument_id);
            let available = match order.side {
                OrderSide::Buy => book.available_ask_quantity_at_or_below(order.price),
                OrderSide::Sell => book.available_bid_quantity_at_or_above(order.price),
            };

            if available < order.quantity {
                info!(
                    order_id = %order.order_id,
                    available = available,
                    required = order.quantity,
                    "FOK order rejected: insufficient liquidity"
                );
                return MatchResult::cancelled(order);
            }
        }

        // Get sequence number first, then get book
        let sequence = self.next_sequence();
        order.sequence = sequence;

        let result = match order.side {
            OrderSide::Buy => self.match_buy(instrument_id.clone(), order),
            OrderSide::Sell => self.match_sell(instrument_id.clone(), order),
        };

        // Check circuit breakers after trades
        self.check_circuit_breakers(&instrument_id, &result.trades);

        // Record metrics
        if let Some(ref metrics) = self.metrics {
            // Record latency
            let elapsed = start_time.elapsed();
            metrics.record_latency(elapsed);
            
            // Record trades
            for trade in &result.trades {
                metrics.record_trade(trade.quantity);
            }
            
            // Record order status
            if result.has_trades() {
                metrics.record_order_matched();
            } else {
                metrics.record_order_rejected();
            }
            
            // Update book depth
            if let Some(book) = self.books.get(&instrument_id) {
                let depth = book.bids.values().map(|v| v.len()).sum::<usize>() 
                    + book.asks.values().map(|v| v.len()).sum::<usize>();
                metrics.set_order_book_depth(depth as u64);
                
                if let Some(spread) = book.spread() {
                    // Convert spread to basis points
                    let spread_bps = if spread > 0.0 && book.best_bid().is_some() {
                        (spread / book.best_bid().unwrap() * 10000.0) as u64
                    } else {
                        0
                    };
                    metrics.set_spread(spread_bps);
                }
            }
        }

        result
    }

    /// Match a buy order against asks
    fn match_buy(&mut self, instrument_id: String, mut order: BookOrder) -> MatchResult {
        let mut trades = Vec::new();
        
        // Collect match data first, then create trades to avoid borrow issues
        let mut matches = Vec::new();
        {
            let book = self.books.get_mut(&instrument_id)
                .expect("Book should exist after get_or_create_book");

            // Match against asks (sell side)
            loop {
                // Check if we have any asks
                let best_ask_price = match book.asks.keys().next() {
                    Some(price) => price.0,
                    None => break, // No sellers
                };

                // Check if price crosses
                // Buy crosses if: best_ask <= buy_price
                if best_ask_price > order.price {
                    break; // No more matches at acceptable price
                }

                // Get orders at this price level (FIFO)
                let price_key = ordered_float::OrderedFloat(best_ask_price);
                let ask_queue = match book.asks.get_mut(&price_key) {
                    Some(q) => q,
                    None => break,
                };

                // Match with first order in queue (FIFO = time priority)
                if let Some(mut ask_order) = ask_queue.pop_front() {
                    // Calculate trade quantity
                    let trade_qty = order.quantity.min(ask_order.quantity);

                    // Store match data
                    matches.push((
                        ask_order.order_id,
                        ask_order.user_id,
                        ask_order.price,
                        trade_qty,
                    ));

                    // Update quantities
                    order.fill(trade_qty);
                    ask_order.fill(trade_qty);

                    // If ask order not fully filled, put it back at front
                    if !ask_order.is_filled() {
                        ask_queue.push_front(ask_order);
                    }

                    // If our order is fully filled, we're done
                    if order.is_filled() {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Clean up empty price levels
            book.cleanup_empty_levels();
        }

        // Now create trades with sequence numbers (no borrow conflict)
        for (maker_order_id, maker_user_id, price, qty) in matches {
            let trade = Trade::new(
                instrument_id.clone(),
                order.order_id,       // Taker (aggressor)
                maker_order_id,       // Maker (resting)
                order.user_id,        // Buyer
                maker_user_id,        // Seller
                price,                // MAKER PRICE (critical!)
                qty,
                OrderSide::Buy,       // Aggressor side
                self.next_sequence(),
            );

            debug!(
                trade_id = %trade.trade_id,
                price = trade.price,
                quantity = trade.quantity,
                "Trade executed"
            );

            trades.push(trade);
        }

        // Handle remainder based on time-in-force
        if order.is_filled() {
            MatchResult::fully_matched(trades)
        } else {
            match order.time_in_force {
                TimeInForce::Gtc => {
                    // Good Till Cancel - insert into book
                    MatchResult::partial_match(trades, order, true)
                }
                TimeInForce::Ioc => {
                    // Immediate or Cancel - cancel remainder
                    MatchResult::partial_match(trades, order, false)
                }
                TimeInForce::Fok => {
                    // FOK pre-check ensures we have enough liquidity
                    // If we get here with remainder, reject (shouldn't happen in single-threaded)
                    MatchResult::cancelled(order)
                }
            }
        }
    }

    /// Match a sell order against bids
    fn match_sell(&mut self, instrument_id: String, mut order: BookOrder) -> MatchResult {
        let mut trades = Vec::new();
        
        // Collect match data first, then create trades to avoid borrow issues
        let mut matches = Vec::new();
        {
            let book = self.books.get_mut(&instrument_id)
                .expect("Book should exist after get_or_create_book");

            // Match against bids (buy side)
            loop {
                // Check if we have any bids
                let best_bid_price = match book.bids.keys().next() {
                    Some(reverse_price) => reverse_price.0 .0,
                    None => break, // No buyers
                };

                // Check if price crosses
                // Sell crosses if: best_bid >= sell_price
                if best_bid_price < order.price {
                    break; // No more matches at acceptable price
                }

                // Get orders at this price level (FIFO)
                let price_key = std::cmp::Reverse(ordered_float::OrderedFloat(best_bid_price));
                let bid_queue = match book.bids.get_mut(&price_key) {
                    Some(q) => q,
                    None => break,
                };

                // Match with first order in queue (FIFO = time priority)
                if let Some(mut bid_order) = bid_queue.pop_front() {
                    // Calculate trade quantity
                    let trade_qty = order.quantity.min(bid_order.quantity);

                    // Store match data
                    matches.push((
                        bid_order.order_id,
                        bid_order.user_id,
                        bid_order.price,
                        trade_qty,
                    ));

                    // Update quantities
                    order.fill(trade_qty);
                    bid_order.fill(trade_qty);

                    // If bid order not fully filled, put it back at front
                    if !bid_order.is_filled() {
                        bid_queue.push_front(bid_order);
                    }

                    // If our order is fully filled, we're done
                    if order.is_filled() {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Clean up empty price levels
            book.cleanup_empty_levels();
        }

        // Now create trades with sequence numbers (no borrow conflict)
        for (maker_order_id, maker_user_id, price, qty) in matches {
            let trade = Trade::new(
                instrument_id.clone(),
                order.order_id,       // Taker (aggressor)
                maker_order_id,       // Maker (resting)
                maker_user_id,        // Buyer
                order.user_id,        // Seller
                price,                // MAKER PRICE (critical!)
                qty,
                OrderSide::Sell,      // Aggressor side
                self.next_sequence(),
            );

            debug!(
                trade_id = %trade.trade_id,
                price = trade.price,
                quantity = trade.quantity,
                "Trade executed"
            );

            trades.push(trade);
        }

        // Handle remainder based on time-in-force
        if order.is_filled() {
            MatchResult::fully_matched(trades)
        } else {
            match order.time_in_force {
                TimeInForce::Gtc => {
                    // Good Till Cancel - insert into book
                    MatchResult::partial_match(trades, order, true)
                }
                TimeInForce::Ioc => {
                    // Immediate or Cancel - cancel remainder
                    MatchResult::partial_match(trades, order, false)
                }
                TimeInForce::Fok => {
                    // FOK pre-check ensures we have enough liquidity
                    // If we get here with remainder, reject (shouldn't happen in single-threaded)
                    MatchResult::cancelled(order)
                }
            }
        }
    }

    /// Cancel an order from the book
    pub fn cancel_order(&mut self, instrument_id: &str, order_id: Uuid) -> Option<BookOrder> {
        let book = self.get_or_create_book(instrument_id);
        let removed = book.remove_order(order_id);
        
        if removed.is_some() {
            info!(order_id = %order_id, instrument = %instrument_id, "Order cancelled");
        }
        
        book.cleanup_empty_levels();
        removed
    }

    /// Get order book for an instrument
    pub fn get_book(&self, instrument_id: &str) -> Option<&OrderBook> {
        self.books.get(instrument_id)
    }

    /// Get mutable order book for an instrument
    pub fn get_book_mut(&mut self, instrument_id: &str) -> Option<&mut OrderBook> {
        self.books.get_mut(instrument_id)
    }

    /// Check if an instrument has an order book
    pub fn has_book(&self, instrument_id: &str) -> bool {
        self.books.contains_key(instrument_id)
    }

    /// Get list of all instruments with order books
    pub fn instruments(&self) -> Vec<String> {
        self.books.keys().cloned().collect()
    }

    /// Remove empty order book for an instrument
    pub fn remove_empty_book(&mut self, instrument_id: &str) -> bool {
        if let Some(book) = self.books.get(instrument_id) {
            if book.is_empty() {
                self.books.remove(instrument_id);
                return true;
            }
        }
        false
    }

    /// Check and trigger circuit breakers after trades
    /// Call this after match_order completes
    fn check_circuit_breakers(&mut self, instrument_id: &str, trades: &[Trade]) {
        if trades.is_empty() {
            return;
        }

        if let Some(ref mut cb) = self.circuit_breakers {
            // Get book stats for liquidity check
            if let Some(book) = self.books.get(instrument_id) {
                let bid_count = book.bids.values().map(|v| v.len()).sum();
                let ask_count = book.asks.values().map(|v| v.len()).sum();
                let spread = book.spread();

                // Check liquidity breaker
                cb.check_liquidity(instrument_id, bid_count, ask_count, spread);
            }

            // Check price movement breaker using last trade price
            if let Some(last_trade) = trades.last() {
                cb.check_price_movement(instrument_id, last_trade.price);
            }
        }
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_order(
        side: OrderSide,
        price: f64,
        quantity: u32,
        tif: TimeInForce,
    ) -> BookOrder {
        BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            side,
            price,
            quantity,
            0,
            tif,
        )
    }

    #[test]
    fn test_basic_match() {
        let mut engine = MatchingEngine::new();

        // Sell order at 100
        let sell = create_test_order(OrderSide::Sell, 100.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(sell);

        // No match (no buyers)
        assert_eq!(result.trades.len(), 0);
        assert!(result.should_insert);

        // Buy order at 100 (should match)
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(buy);

        // Should produce 1 trade
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 10);
        assert_eq!(result.trades[0].price, 100.0);
        assert!(result.remaining_order.is_none());
    }

    #[test]
    fn test_partial_fill() {
        let mut engine = MatchingEngine::new();

        // Sell 5 @ 100
        let sell = create_test_order(OrderSide::Sell, 100.0, 5, TimeInForce::Gtc);
        engine.match_order(sell);

        // Buy 10 @ 100 (only 5 available)
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(buy);

        // Should match 5, leave 5 remaining
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 5);
        assert!(result.remaining_order.is_some());
        assert_eq!(result.remaining_order.unwrap().quantity, 5);
    }

    #[test]
    fn test_price_time_priority() {
        let mut engine = MatchingEngine::new();

        // Add 3 sell orders at same price, different sequences
        let sell1 = BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Sell,
            100.0,
            10,
            1, // First
            TimeInForce::Gtc,
        );
        let sell2 = BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Sell,
            100.0,
            10,
            2, // Second
            TimeInForce::Gtc,
        );
        let sell3 = BookOrder::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            OrderSide::Sell,
            100.0,
            10,
            3, // Third
            TimeInForce::Gtc,
        );

        engine.match_order(sell1);
        engine.match_order(sell2);
        engine.match_order(sell3);

        // Buy 15 @ 100 (should match sell1 completely, sell2 partially)
        let buy = create_test_order(OrderSide::Buy, 100.0, 15, TimeInForce::Gtc);
        let result = engine.match_order(buy);

        // Should have 2 trades
        assert_eq!(result.trades.len(), 2);

        // First trade should be with sell1 (earliest)
        assert_eq!(result.trades[0].maker_order_id.sequence, 1);
        assert_eq!(result.trades[0].quantity, 10);

        // Second trade should be with sell2
        assert_eq!(result.trades[1].maker_order_id.sequence, 2);
        assert_eq!(result.trades[1].quantity, 5);
    }

    #[test]
    fn test_ioc_order() {
        let mut engine = MatchingEngine::new();

        // Sell 5 @ 100
        let sell = create_test_order(OrderSide::Sell, 100.0, 5, TimeInForce::Gtc);
        engine.match_order(sell);

        // IOC Buy 10 @ 100 (only 5 available)
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Ioc);
        let result = engine.match_order(buy);

        // Should match 5, cancel 5 (not inserted)
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 5);
        assert!(!result.should_insert);
    }

    #[test]
    fn test_fok_order_success() {
        let mut engine = MatchingEngine::new();

        // Sell 10 @ 100
        let sell = create_test_order(OrderSide::Sell, 100.0, 10, TimeInForce::Gtc);
        engine.match_order(sell);

        // FOK Buy 10 @ 100 (can fill completely)
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Fok);
        let result = engine.match_order(buy);

        // Should fill completely
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 10);
        assert!(result.remaining_order.is_none());
    }

    #[test]
    fn test_fok_exact_liquidity_at_multiple_levels() {
        let mut engine = MatchingEngine::new();

        // Sell 4 @ 99, Sell 7 @ 100 (total 11 available)
        engine.match_order(create_test_order(OrderSide::Sell, 99.0, 4, TimeInForce::Gtc));
        engine.match_order(create_test_order(OrderSide::Sell, 100.0, 7, TimeInForce::Gtc));

        // FOK Buy 11 @ 100 (exact liquidity across multiple levels)
        let buy = create_test_order(OrderSide::Buy, 100.0, 11, TimeInForce::Fok);
        let result = engine.match_order(buy);

        // Should fully fill
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].quantity, 4);
        assert_eq!(result.trades[1].quantity, 7);
        assert!(result.remaining_order.is_none());
    }

    #[test]
    fn test_fok_order_failure() {
        let mut engine = MatchingEngine::new();

        // Sell 5 @ 100
        let sell = create_test_order(OrderSide::Sell, 100.0, 5, TimeInForce::Gtc);
        engine.match_order(sell);

        // FOK Buy 10 @ 100 (can't fill completely)
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Fok);
        let result = engine.match_order(buy);

        // Should cancel entire order (FOK semantics)
        assert_eq!(result.trades.len(), 0);
        assert!(!result.should_insert);
    }

    #[test]
    fn test_fok_sell_insufficient_liquidity() {
        let mut engine = MatchingEngine::new();

        // Buy 5 @ 100
        let buy = create_test_order(OrderSide::Buy, 100.0, 5, TimeInForce::Gtc);
        engine.match_order(buy);

        // FOK Sell 10 @ 100 (not enough bids)
        let sell = create_test_order(OrderSide::Sell, 100.0, 10, TimeInForce::Fok);
        let result = engine.match_order(sell);

        // Should cancel (insufficient liquidity)
        assert_eq!(result.trades.len(), 0);
        assert!(!result.should_insert);

        // Original buy should still be in book (untouched)
        assert_eq!(engine.get_book("test").unwrap().bid_quantity_at(100.0), 5);
    }

    #[test]
    fn test_fok_no_liquidity() {
        let mut engine = MatchingEngine::new();

        // FOK Buy with no sellers -> cancelled
        let buy = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Fok);
        let result = engine.match_order(buy);

        assert_eq!(result.trades.len(), 0);
        assert!(!result.should_insert);
    }

    #[test]
    fn test_determinism() {
        // Run same sequence twice, must get identical results
        let orders = vec![
            create_test_order(OrderSide::Sell, 100.0, 10, TimeInForce::Gtc),
            create_test_order(OrderSide::Sell, 99.0, 5, TimeInForce::Gtc),
            create_test_order(OrderSide::Buy, 100.0, 12, TimeInForce::Gtc),
        ];

        // Run 1
        let mut engine1 = MatchingEngine::new();
        let mut results1 = Vec::new();
        for order in orders.clone() {
            results1.push(engine1.match_order(order));
        }

        // Run 2
        let mut engine2 = MatchingEngine::new();
        let mut results2 = Vec::new();
        for order in orders {
            results2.push(engine2.match_order(order));
        }

        // Must be identical
        assert_eq!(results1.len(), results2.len());
        for (r1, r2) in results1.iter().zip(results2.iter()) {
            assert_eq!(r1.trades.len(), r2.trades.len());
            for (t1, t2) in r1.trades.iter().zip(r2.trades.iter()) {
                assert_eq!(t1.price, t2.price);
                assert_eq!(t1.quantity, t2.quantity);
            }
        }
    }

    #[test]
    fn test_cancel_order() {
        let mut engine = MatchingEngine::new();

        // Add an order
        let order = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Gtc);
        let order_id = order.order_id;
        engine.match_order(order);

        // Verify it's in the book
        assert!(engine.get_book("test").is_some());

        // Cancel it
        let cancelled = engine.cancel_order("test", order_id);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().quantity, 10);
    }

    #[test]
    fn test_no_crossing() {
        let mut engine = MatchingEngine::new();

        // Bid at 95
        let bid = create_test_order(OrderSide::Buy, 95.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(bid);

        // No match (ask is higher at 100)
        assert_eq!(result.trades.len(), 0);
        assert!(result.should_insert);

        // Ask at 100
        let ask = create_test_order(OrderSide::Sell, 100.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(ask);

        // No match (bid is lower at 95)
        assert_eq!(result.trades.len(), 0);
        assert!(result.should_insert);
    }

    #[test]
    fn test_crossing() {
        let mut engine = MatchingEngine::new();

        // Bid at 100
        let bid = create_test_order(OrderSide::Buy, 100.0, 10, TimeInForce::Gtc);
        engine.match_order(bid);

        // Ask at 95 (crosses!)
        let ask = create_test_order(OrderSide::Sell, 95.0, 10, TimeInForce::Gtc);
        let result = engine.match_order(ask);

        // Should match at 100 (maker's price)
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].price, 100.0);
    }
}
