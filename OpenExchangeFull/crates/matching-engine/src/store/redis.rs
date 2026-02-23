//! Redis store implementation for the Matching Engine
//!
//! This implementation stores order books and trades in Redis for persistence.

use async_trait::async_trait;
use redis::{AsyncCommands, JsonAsyncCommands};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::domain::{BookOrder, OrderBook, Trade};
use crate::engine::MatchingEngine;
use crate::event::MatchingEvent;
use crate::log::create_event_log;
use crate::result::MatchResult;
use crate::store::traits::{MatchingStore, StoreError, StoreResult};
use config::RedisConfig;

/// Redis store for order matching
///
/// This implementation persists order books and trades to Redis.
/// It also maintains an in-memory cache for fast access.
pub struct RedisStore {
    /// Redis connection pool (wrapped in Mutex for mutable access)
    redis: Arc<tokio::sync::Mutex<redis::aio::ConnectionManager>>,
    /// In-memory cache for fast reads
    cache: RwLock<std::collections::HashMap<String, OrderBook>>,
    /// In-memory cache for trades
    trades: RwLock<std::collections::HashMap<String, Vec<Trade>>>,
    /// Event log (in-memory for now, could be Redis streams)
    event_log: crate::log::SharedEventLog,
    /// Engine for matching (in-memory, synced to Redis)
    engine: RwLock<MatchingEngine>,
    /// Redis key prefix
    key_prefix: String,
    /// Max trades per instrument
    max_trades: usize,
}

impl RedisStore {
    /// Create a new Redis store
    pub async fn new(config: &RedisConfig) -> StoreResult<Self> {
        let connection_string = format!(
            "redis://{}:{}@{}:{}/{}",
            if config.password.is_empty() { "" } else { &config.password },
            if config.password.is_empty() { "" } else { ":" },
            config.host,
            config.port,
            config.db_index
        );

        info!(host = %config.host, port = config.port, db = config.db_index, "Connecting to Redis");

        let client = redis::Client::open(connection_string.as_str())
            .map_err(|e| StoreError::RedisError(e.to_string()))?;

        let connection_manager = client
            .get_connection_manager()
            .await
            .map_err(|e| StoreError::RedisError(e.to_string()))?;

        Ok(Self {
            redis: Arc::new(tokio::sync::Mutex::new(connection_manager)),
            cache: RwLock::new(std::collections::HashMap::new()),
            trades: RwLock::new(std::collections::HashMap::new()),
            event_log: create_event_log(),
            engine: RwLock::new(MatchingEngine::new()),
            key_prefix: "matching".to_string(),
            max_trades: 1000,
        })
    }

    /// Generate Redis key for an instrument's order book
    fn book_key(&self, instrument_id: &str) -> String {
        format!("{}:book:{}", self.key_prefix, instrument_id)
    }

    /// Generate Redis key for an instrument's trades
    fn trades_key(&self, instrument_id: &str) -> String {
        format!("{}:trades:{}", self.key_prefix, instrument_id)
    }

    /// Generate Redis key for sequence
    fn sequence_key(&self) -> String {
        format!("{}:sequence", self.key_prefix)
    }

    /// Sync engine state to Redis
    async fn sync_book_to_redis(&self, instrument_id: &str, book: &OrderBook) -> StoreResult<()> {
        let key = self.book_key(instrument_id);
        
        let json = serde_json::to_string(book)
            .map_err(|e| StoreError::SerializationError(e.to_string()))?;

        let mut redis = self.redis.lock().await;
        redis
            .set::<_, _, ()>(&key, json)
            .await
            .map_err(|e| StoreError::RedisError(e.to_string()))?;

        Ok(())
    }

    /// Load book from Redis into cache
    async fn load_book_from_redis(&self, instrument_id: &str) -> StoreResult<Option<OrderBook>> {
        let key = self.book_key(instrument_id);
        
        let mut redis = self.redis.lock().await;
        let result: Option<String> = redis
            .get(&key)
            .await
            .map_err(|e| StoreError::RedisError(e.to_string()))?;

        match result {
            Some(json) => {
                let book: OrderBook = serde_json::from_str(&json)
                    .map_err(|e| StoreError::SerializationError(e.to_string()))?;
                Ok(Some(book))
            }
            None => Ok(None),
        }
    }

    /// Delete book from Redis
    async fn delete_book_from_redis(&self, instrument_id: &str) -> StoreResult<()> {
        let key = self.book_key(instrument_id);
        
        let mut redis = self.redis.lock().await;
        redis
            .del::<_, ()>(&key)
            .await
            .map_err(|e| StoreError::RedisError(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl MatchingStore for RedisStore {
    async fn submit_order(&self, order: BookOrder) -> StoreResult<MatchResult> {
        let instrument_id = order.instrument_id.clone();
        
        // Load book from cache or Redis
        {
            let mut cache = self.cache.write().await;
            if !cache.contains_key(&instrument_id) {
                if let Some(book) = self.load_book_from_redis(&instrument_id).await? {
                    cache.insert(instrument_id.clone(), book);
                }
            }
        }
        
        // Run matching
        let result = {
            let mut engine = self.engine.write().await;
            engine.match_order(order)
        };
        
        // Update cache with new book state
        if result.should_insert {
            if let Some(remaining) = &result.remaining_order {
                let mut cache = self.cache.write().await;
                let book = cache.entry(instrument_id.clone()).or_insert_with(|| {
                    OrderBook::new(instrument_id.clone())
                });
                book.insert_order(remaining.clone());
                
                // Sync to Redis
                if let Err(e) = self.sync_book_to_redis(&instrument_id, book).await {
                    warn!(error = %e, "Failed to sync book to Redis");
                }
            }
        } else if result.trades.is_empty() && result.remaining_order.is_none() {
            // Order was fully filled, update cache
            let mut cache = self.cache.write().await;
            if let Some(book) = cache.get_mut(&instrument_id) {
                book.cleanup_empty_levels();
                if book.is_empty() {
                    cache.remove(&instrument_id);
                    let _ = self.delete_book_from_redis(&instrument_id).await;
                } else {
                    if let Err(e) = self.sync_book_to_redis(&instrument_id, book).await {
                        warn!(error = %e, "Failed to sync book to Redis");
                    }
                }
            }
        }
        
        // Store trades
        if result.has_trades() {
            let mut trades = self.trades.write().await;
            let instrument_trades = trades.entry(instrument_id.clone()).or_insert_with(Vec::new);
            
            for trade in &result.trades {
                instrument_trades.push(trade.clone());
                
                // Trim to max
                while instrument_trades.len() > self.max_trades {
                    instrument_trades.remove(0);
                }
            }
            
            // TODO: Persist trades to Redis
        }
        
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
        // Load book from cache or Redis
        {
            let mut cache = self.cache.write().await;
            if !cache.contains_key(instrument_id) {
                if let Some(book) = self.load_book_from_redis(instrument_id).await? {
                    cache.insert(instrument_id.to_string(), book);
                }
            }
        }
        
        let cancelled = {
            let mut engine = self.engine.write().await;
            engine.cancel_order(instrument_id, order_id)
        };
        
        // Update cache
        if cancelled.is_some() {
            let mut cache = self.cache.write().await;
            if let Some(book) = cache.get_mut(instrument_id) {
                book.remove_order(order_id);
                book.cleanup_empty_levels();
                
                if book.is_empty() {
                    cache.remove(instrument_id);
                    let _ = self.delete_book_from_redis(instrument_id).await;
                } else {
                    if let Err(e) = self.sync_book_to_redis(instrument_id, book).await {
                        warn!(error = %e, "Failed to sync book to Redis");
                    }
                }
            }
        }
        
        if cancelled.is_some() {
            info!(order_id = %order_id, instrument_id = %instrument_id, "Order cancelled");
        }
        
        Ok(cancelled)
    }

    async fn get_book(&self, instrument_id: &str) -> StoreResult<Option<OrderBook>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(book) = cache.get(instrument_id) {
                return Ok(Some(book.clone()));
            }
        }
        
        // Load from Redis
        let book = self.load_book_from_redis(instrument_id).await?;
        
        if let Some(ref b) = book {
            let mut cache = self.cache.write().await;
            cache.insert(instrument_id.to_string(), b.clone());
        }
        
        Ok(book)
    }

    async fn get_best_bid(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let book = self.get_book(instrument_id).await?;
        Ok(book.and_then(|b| b.best_bid()))
    }

    async fn get_best_ask(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let book = self.get_book(instrument_id).await?;
        Ok(book.and_then(|b| b.best_ask()))
    }

    async fn get_spread(&self, instrument_id: &str) -> StoreResult<Option<f64>> {
        let book = self.get_book(instrument_id).await?;
        Ok(book.and_then(|b| b.spread()))
    }

    async fn has_book(&self, instrument_id: &str) -> StoreResult<bool> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if cache.contains_key(instrument_id) {
                return Ok(true);
            }
        }
        
        // Check Redis
        let book = self.load_book_from_redis(instrument_id).await?;
        Ok(book.is_some())
    }

    async fn instruments(&self) -> StoreResult<Vec<String>> {
        // For now, return from cache
        // A production implementation would scan Redis keys
        let cache = self.cache.read().await;
        Ok(cache.keys().cloned().collect())
    }

    async fn get_trades(&self, instrument_id: &str, limit: u32) -> StoreResult<Vec<Trade>> {
        let trades = self.trades.read().await;
        
        if let Some(instrument_trades) = trades.get(instrument_id) {
            let start = instrument_trades.len().saturating_sub(limit as usize);
            Ok(instrument_trades[start..].to_vec())
        } else {
            Ok(Vec::new())
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
        panic!("Use query methods for Redis store")
    }

    fn engine_ref(&self) -> &MatchingEngine {
        panic!("Use query methods for Redis store")
    }
}
