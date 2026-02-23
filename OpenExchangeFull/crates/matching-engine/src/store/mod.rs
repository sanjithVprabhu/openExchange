//! Store module for the Matching Engine
//!
//! This module provides the store trait and implementations.

mod traits;
mod memory;
mod redis;

pub use traits::*;
pub use memory::InMemoryStore;
pub use redis::RedisStore;

// Re-export for convenience
use crate::event::MatchingEvent;
use crate::log::SharedEventLog;
use tracing::info;

/// Store type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreType {
    /// In-memory store (fast, non-persistent)
    InMemory,
    /// Redis store (persistent)
    Redis,
}

impl StoreType {
    /// Parse store type from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "inmemory" | "in_memory" | "memory" => Some(StoreType::InMemory),
            "redis" => Some(StoreType::Redis),
            _ => None,
        }
    }
}

/// Create a store based on configuration
pub async fn create_store(
    store_type: StoreType,
    redis_config: Option<&config::RedisConfig>,
) -> Result<Box<dyn MatchingStore>, String> {
    match store_type {
        StoreType::InMemory => {
            info!("Creating in-memory store");
            Ok(Box::new(InMemoryStore::new()))
        }
        StoreType::Redis => {
            let config = redis_config.ok_or("Redis config required for Redis store")?;
            info!("Creating Redis store");
            let store = RedisStore::new(config)
                .await
                .map_err(|e| format!("Failed to create Redis store: {}", e))?;
            Ok(Box::new(store))
        }
    }
}

/// Create store from OrderbookStoreConfig
pub async fn create_store_from_config(
    config: &config::OrderbookStoreConfig,
) -> Result<Box<dyn MatchingStore>, String> {
    let store_type = StoreType::from_str(&config.store_type)
        .unwrap_or(StoreType::InMemory);
    
    create_store(store_type, config.redis.as_ref()).await
}
