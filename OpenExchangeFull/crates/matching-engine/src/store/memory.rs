//! In-memory store implementation for the Matching Engine

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

use crate::domain::{BookOrder, OrderBook, Trade};
use crate::engine::MatchingEngine;
use crate::event::MatchingEvent;
use crate::log::create_event_log;
use crate::result::MatchResult;
use crate::store::traits::{MatchingStore, StoreError, StoreResult};

/// In-memory store for order matching
///
/// This implementation stores all order books and trades in memory.
/// It's fast but non-persistent - data is lost on restart.
pub struct InMemoryStore {
    /// The matching engine
    engine: RwLock<MatchingEngine>,
    /// Recent trades per instrument
    trades: RwLock<std::collections::HashMap<String, Vec<Trade>>>,
    /// Event log for determinism
    event_log: crate::log::SharedEventLog,
    /// Max trades to keep per instrument
    max_trades_per_instrument: usize,
}

impl InMemoryStore {
    /// Create a new in-memory store
    pub fn new() -> Self {
        Self {
            engine: RwLock::new(MatchingEngine::new()),
            trades: RwLock::new(std::collections::HashMap::new()),
            event_log: create_event_log(),
            max_trades_per_instrument: 1000,
        }
    }

    /// Create a new store with custom settings
    pub fn with_max_trades(max_trades: usize) -> Self {
        Self {
            engine: RwLock::new(MatchingEngine::new()),
            trades: RwLock::new(std::collections::HashMap::new()),
            event_log: create_event_log(),
            max_trades_per_instrument: max_trades,
        }
    }

    /// Add a trade to the trade history
    async fn add_trade(&self, trade: Trade) {
        let mut trades = self.trades.write().await;
        let instrument = trade.instrument_id.clone();
        
        let instrument_trades = trades.entry(instrument).or_insert_with(Vec::new);
        instrument_trades.push(trade);
        
        // Trim to max size
        while instrument_trades.len() > self.max_trades_per_instrument {
            instrument_trades.remove(0);
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MatchingStore for InMemoryStore {
    async fn submit_order(&self, order: BookOrder) -> StoreResult<MatchResult> {
        let instrument_id = order.instrument_id.clone();
        let sequence = {
            let engine = self.engine.read().await;
            engine.sequence()
        };
        
        // Log the order acceptance
        {
            let mut log = self.event_log.write().await;
            log.append(MatchingEvent::OrderAccepted {
                order_id: order.order_id,
                instrument_id: instrument_id.clone(),
                sequence,
            });
        }
        
        // Run matching
        let result = {
            let mut engine = self.engine.write().await;
            engine.match_order(order)
        };
        
        // If trades were generated, log them and store
        if result.has_trades() {
            for trade in &result.trades {
                self.add_trade(trade.clone()).await;
                
                // Log trade event
                let mut log = self.event_log.write().await;
                log.append(MatchingEvent::TradeExecuted {
                    trade: trade.clone(),
                    sequence: trade.sequence,
                });
            }
        }
        
        // If remaining order should be inserted, it's already in the engine
        debug!(
            instrument_id = %instrument_id,
            trades = result.trades.len(),
            "Order matched"
        );
        
        Ok(result)
    }

    async fn cancel_order(
        &self,
        instrument_id: &str,
        order_id: Uuid,
    ) -> StoreResult<Option<BookOrder>> {
        let sequence = {
            let engine = self.engine.read().await;
            engine.sequence()
        };
        
        let cancelled = {
            let mut engine = self.engine.write().await;
            engine.cancel_order(instrument_id, order_id)
        };
        
        if cancelled.is_some() {
            // Log cancellation
            let mut log = self.event_log.write().await;
            log.append(MatchingEvent::OrderCancelled {
                order_id,
                instrument_id: instrument_id.to_string(),
                sequence,
            });
            
            info!(order_id = %order_id, instrument_id = %instrument_id, "Order cancelled");
        }
        
        Ok(cancelled)
    }

    async fn get_book(&self, instrument_id: &str) -> StoreResult<Option<OrderBook>> {
        let engine = self.engine.read().await;
        Ok(engine.get_book(instrument_id).cloned())
    }

    async fn get_best_bid(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let engine = self.engine.read().await;
        Ok(engine.get_book(instrument_id).and_then(|b| b.best_bid()))
    }

    async fn get_best_ask(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let engine = self.engine.read().await;
        Ok(engine.get_book(instrument_id).and_then(|b| b.best_ask()))
    }

    async fn get_spread(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let engine = self.engine.read().await;
        Ok(engine.get_book(instrument_id).and_then(|b| b.spread()))
    }

    async fn has_book(&self, instrument_id: &str) -> StoreResult<bool> {
        let engine = self.engine.read().await;
        Ok(engine.has_book(instrument_id))
    }

    async fn instruments(&self) -> StoreResult<Vec<String>> {
        let engine = self.engine.read().await;
        Ok(engine.instruments())
    }

    async fn get_trades(&self, instrument_id: &str, limit: u32) -> StoreResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        let instrument_trades = trades.get(instrument_id);
        
        match instrument_trades {
            Some(t) => {
                let start = t.len().saturating_sub(limit as usize);
                Ok(t[start..].to_vec())
            }
            None => Ok(Vec::new()),
        }
    }

    async fn append_event(&self, event: MatchingEvent) -> StoreResult<()> {
        let mut log = self.event_log.write().await;
        log.append(event);
        Ok(())
    }

    async fn get_events(&self, from_sequence: u64) -> StoreResult<Vec<MatchingEvent>> {
        let log = self.event_log.read().await;
        Ok(log.get_from(from_sequence))
    }

    async fn get_sequence(&self) -> StoreResult<u64> {
        let log = self.event_log.read().await;
        Ok(log.sequence())
    }

    fn engine(&self) -> MatchingEngine {
        // Note: This clones the engine state
        // For in-memory, this is acceptable since we're cloning the entire state
        // In production, you might want to use Arc for shared access
        // For now, we need to get a clone through the RwLock
        // This is a limitation of the current design
        // TODO: Consider using Arc<MatchingEngine> internally
        panic!("Use engine_ref() for in-memory store")
    }

    fn engine_ref(&self) -> &MatchingEngine {
        // This is unsafe if the RwLock is held elsewhere
        // But we use it only for read operations in queries
        // The proper fix would be to redesign the trait
        panic!("Use get_book() and other query methods instead")
    }
}

// Provide a way to get engine for advanced operations
impl InMemoryStore {
    /// Get a read guard to the engine (for advanced operations)
    pub async fn engine_read(&self) -> tokio::sync::RwLockReadGuard<'_, MatchingEngine> {
        self.engine.read().await
    }

    /// Get a write guard to the engine (for advanced operations)
    pub async fn engine_write(&self) -> tokio::sync::RwLockWriteGuard<'_, MatchingEngine> {
        self.engine.write().await
    }
}
