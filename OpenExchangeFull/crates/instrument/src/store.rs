//! Instrument storage traits and implementations.
//!
//! This module defines the `InstrumentStore` trait that abstracts away storage details.
//! Adapters (Postgres, Redis, etc.) implement this trait externally.

use crate::error::{InstrumentError, InstrumentResult};
use crate::types::{InstrumentId, InstrumentStatus, OptionInstrument, OptionType};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Query filters for listing instruments.
#[derive(Debug, Clone, Default)]
pub struct InstrumentQuery {
    /// Filter by underlying asset symbol.
    pub underlying: Option<String>,
    /// Filter by option type.
    pub option_type: Option<OptionType>,
    /// Filter by status.
    pub status: Option<InstrumentStatus>,
    /// Filter by expiry after this date.
    pub expiry_after: Option<DateTime<Utc>>,
    /// Filter by expiry before this date.
    pub expiry_before: Option<DateTime<Utc>>,
    /// Filter by minimum strike price.
    pub strike_min: Option<f64>,
    /// Filter by maximum strike price.
    pub strike_max: Option<f64>,
    /// Limit number of results.
    pub limit: Option<usize>,
    /// Offset for pagination.
    pub offset: Option<usize>,
}

impl InstrumentQuery {
    /// Create a new empty query.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by underlying asset.
    pub fn with_underlying(mut self, underlying: impl Into<String>) -> Self {
        self.underlying = Some(underlying.into());
        self
    }

    /// Filter by option type.
    pub fn with_option_type(mut self, option_type: OptionType) -> Self {
        self.option_type = Some(option_type);
        self
    }

    /// Filter by status.
    pub fn with_status(mut self, status: InstrumentStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Filter by expiry range.
    pub fn with_expiry_range(
        mut self,
        after: Option<DateTime<Utc>>,
        before: Option<DateTime<Utc>>,
    ) -> Self {
        self.expiry_after = after;
        self.expiry_before = before;
        self
    }

    /// Filter by strike range.
    pub fn with_strike_range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.strike_min = min;
        self.strike_max = max;
        self
    }

    /// Set pagination.
    pub fn with_pagination(mut self, limit: usize, offset: usize) -> Self {
        self.limit = Some(limit);
        self.offset = Some(offset);
        self
    }

    /// Check if an instrument matches this query.
    pub fn matches(&self, instrument: &OptionInstrument) -> bool {
        if let Some(ref underlying) = self.underlying {
            if instrument.underlying.symbol != *underlying {
                return false;
            }
        }

        if let Some(option_type) = self.option_type {
            if instrument.option_type != option_type {
                return false;
            }
        }

        if let Some(status) = self.status {
            if instrument.status != status {
                return false;
            }
        }

        if let Some(expiry_after) = self.expiry_after {
            if instrument.expiry <= expiry_after {
                return false;
            }
        }

        if let Some(expiry_before) = self.expiry_before {
            if instrument.expiry >= expiry_before {
                return false;
            }
        }

        if let Some(strike_min) = self.strike_min {
            if instrument.strike.value() < strike_min {
                return false;
            }
        }

        if let Some(strike_max) = self.strike_max {
            if instrument.strike.value() > strike_max {
                return false;
            }
        }

        true
    }
}

/// Trait for instrument storage.
///
/// This trait defines the interface for storing and retrieving instruments.
/// Implementations can use any storage backend (Postgres, Redis, in-memory, etc.).
///
/// # Example
///
/// ```ignore
/// use instrument::{InstrumentStore, OptionInstrument};
///
/// async fn example(store: &dyn InstrumentStore) {
///     let instrument = store.get_by_symbol("BTC-20240315-50000-C").await?;
///     println!("Found: {}", instrument.symbol);
/// }
/// ```
#[async_trait]
pub trait InstrumentStore: Send + Sync {
    /// Get an instrument by its unique ID.
    async fn get(&self, id: &InstrumentId) -> InstrumentResult<Option<OptionInstrument>>;

    /// Get an instrument by its symbol (e.g., "BTC-20240315-50000-C").
    async fn get_by_symbol(&self, symbol: &str) -> InstrumentResult<Option<OptionInstrument>>;

    /// List instruments matching the given query.
    async fn list(&self, query: &InstrumentQuery) -> InstrumentResult<Vec<OptionInstrument>>;

    /// Count instruments matching the given query.
    async fn count(&self, query: &InstrumentQuery) -> InstrumentResult<usize>;

    /// Save a new instrument.
    async fn save(&self, instrument: OptionInstrument) -> InstrumentResult<()>;

    /// Save multiple instruments (batch insert).
    async fn save_batch(&self, instruments: Vec<OptionInstrument>) -> InstrumentResult<()>;

    /// Update an existing instrument.
    async fn update(&self, instrument: OptionInstrument) -> InstrumentResult<()>;

    /// Update the status of an instrument.
    async fn update_status(
        &self,
        id: &InstrumentId,
        status: InstrumentStatus,
    ) -> InstrumentResult<()>;

    /// Delete an instrument by ID.
    async fn delete(&self, id: &InstrumentId) -> InstrumentResult<()>;

    /// Get all active instruments for a given underlying.
    async fn get_active_by_underlying(
        &self,
        underlying: &str,
    ) -> InstrumentResult<Vec<OptionInstrument>> {
        let query = InstrumentQuery::new()
            .with_underlying(underlying)
            .with_status(InstrumentStatus::Active);
        self.list(&query).await
    }

    /// Get all expired instruments that need settlement.
    async fn get_expired_unsettled(&self) -> InstrumentResult<Vec<OptionInstrument>> {
        let query = InstrumentQuery::new().with_status(InstrumentStatus::Expired);
        self.list(&query).await
    }

    /// Check if a symbol already exists.
    async fn symbol_exists(&self, symbol: &str) -> InstrumentResult<bool> {
        Ok(self.get_by_symbol(symbol).await?.is_some())
    }
}

/// In-memory implementation of InstrumentStore.
///
/// This is useful for testing and development. For production,
/// use a persistent store like PostgresInstrumentStore.
#[derive(Debug)]
pub struct InMemoryInstrumentStore {
    instruments: Arc<RwLock<HashMap<InstrumentId, OptionInstrument>>>,
    by_symbol: Arc<RwLock<HashMap<String, InstrumentId>>>,
}

impl InMemoryInstrumentStore {
    /// Create a new in-memory store.
    pub fn new() -> Self {
        Self {
            instruments: Arc::new(RwLock::new(HashMap::new())),
            by_symbol: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the number of instruments in the store.
    pub fn len(&self) -> usize {
        self.instruments.read().len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.instruments.read().is_empty()
    }

    /// Clear all instruments from the store.
    pub fn clear(&self) {
        self.instruments.write().clear();
        self.by_symbol.write().clear();
    }
}

impl Default for InMemoryInstrumentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for InMemoryInstrumentStore {
    fn clone(&self) -> Self {
        Self {
            instruments: Arc::clone(&self.instruments),
            by_symbol: Arc::clone(&self.by_symbol),
        }
    }
}

#[async_trait]
impl InstrumentStore for InMemoryInstrumentStore {
    async fn get(&self, id: &InstrumentId) -> InstrumentResult<Option<OptionInstrument>> {
        Ok(self.instruments.read().get(id).cloned())
    }

    async fn get_by_symbol(&self, symbol: &str) -> InstrumentResult<Option<OptionInstrument>> {
        let by_symbol = self.by_symbol.read();
        if let Some(id) = by_symbol.get(symbol) {
            Ok(self.instruments.read().get(id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn list(&self, query: &InstrumentQuery) -> InstrumentResult<Vec<OptionInstrument>> {
        let instruments = self.instruments.read();
        let mut results: Vec<OptionInstrument> = instruments
            .values()
            .filter(|i| query.matches(i))
            .cloned()
            .collect();

        // Sort by expiry, then by strike
        results.sort_by(|a, b| {
            a.expiry
                .cmp(&b.expiry)
                .then_with(|| a.strike.value().partial_cmp(&b.strike.value()).unwrap())
                .then_with(|| a.option_type.code().cmp(b.option_type.code()))
        });

        // Apply pagination
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(usize::MAX);

        Ok(results.into_iter().skip(offset).take(limit).collect())
    }

    async fn count(&self, query: &InstrumentQuery) -> InstrumentResult<usize> {
        let instruments = self.instruments.read();
        Ok(instruments.values().filter(|i| query.matches(i)).count())
    }

    async fn save(&self, instrument: OptionInstrument) -> InstrumentResult<()> {
        let id = instrument.id.clone();
        let symbol = instrument.symbol.clone();

        // Check if symbol already exists
        if self.by_symbol.read().contains_key(&symbol) {
            return Err(InstrumentError::AlreadyExists(symbol));
        }

        self.instruments.write().insert(id.clone(), instrument);
        self.by_symbol.write().insert(symbol, id);
        Ok(())
    }

    async fn save_batch(&self, instruments: Vec<OptionInstrument>) -> InstrumentResult<()> {
        let mut instruments_map = self.instruments.write();
        let mut by_symbol_map = self.by_symbol.write();

        for instrument in instruments {
            let id = instrument.id.clone();
            let symbol = instrument.symbol.clone();

            if by_symbol_map.contains_key(&symbol) {
                // Skip duplicates in batch insert
                continue;
            }

            instruments_map.insert(id.clone(), instrument);
            by_symbol_map.insert(symbol, id);
        }

        Ok(())
    }

    async fn update(&self, instrument: OptionInstrument) -> InstrumentResult<()> {
        let id = instrument.id.clone();

        if !self.instruments.read().contains_key(&id) {
            return Err(InstrumentError::NotFound(id.to_string()));
        }

        self.instruments.write().insert(id, instrument);
        Ok(())
    }

    async fn update_status(
        &self,
        id: &InstrumentId,
        status: InstrumentStatus,
    ) -> InstrumentResult<()> {
        let mut instruments = self.instruments.write();
        if let Some(instrument) = instruments.get_mut(id) {
            instrument.status = status;
            instrument.updated_at = Utc::now();
            Ok(())
        } else {
            Err(InstrumentError::NotFound(id.to_string()))
        }
    }

    async fn delete(&self, id: &InstrumentId) -> InstrumentResult<()> {
        let mut instruments = self.instruments.write();
        if let Some(instrument) = instruments.remove(id) {
            self.by_symbol.write().remove(&instrument.symbol);
            Ok(())
        } else {
            Err(InstrumentError::NotFound(id.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ExerciseStyle, Strike, UnderlyingAsset};
    use chrono::TimeZone;

    fn create_test_instrument(symbol: &str, underlying: &str, strike: f64) -> OptionInstrument {
        let expiry = Utc.with_ymd_and_hms(2024, 12, 31, 8, 0, 0).unwrap();
        OptionInstrument {
            id: InstrumentId::generate(),
            symbol: symbol.to_string(),
            underlying: UnderlyingAsset {
                symbol: underlying.to_string(),
                name: underlying.to_string(),
                decimals: 8,
                contract_size: 0.01,
                tick_size: 0.5,
                price_decimals: 2,
            },
            option_type: OptionType::Call,
            strike: Strike::new(strike, 2),
            expiry,
            exercise_style: ExerciseStyle::European,
            settlement_currency: "USDT".to_string(),
            contract_size: 0.01,
            tick_size: 0.5,
            min_order_size: 1,
            status: InstrumentStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_inmemory_store_save_and_get() {
        let store = InMemoryInstrumentStore::new();
        let instrument = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);
        let id = instrument.id.clone();

        store.save(instrument.clone()).await.unwrap();

        let retrieved = store.get(&id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().symbol, "BTC-20241231-50000-C");
    }

    #[tokio::test]
    async fn test_inmemory_store_get_by_symbol() {
        let store = InMemoryInstrumentStore::new();
        let instrument = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);

        store.save(instrument).await.unwrap();

        let retrieved = store.get_by_symbol("BTC-20241231-50000-C").await.unwrap();
        assert!(retrieved.is_some());

        let not_found = store.get_by_symbol("INVALID").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_inmemory_store_list_with_query() {
        let store = InMemoryInstrumentStore::new();

        // Add some instruments
        store
            .save(create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0))
            .await
            .unwrap();
        store
            .save(create_test_instrument("BTC-20241231-55000-C", "BTC", 55000.0))
            .await
            .unwrap();
        store
            .save(create_test_instrument("ETH-20241231-3000-C", "ETH", 3000.0))
            .await
            .unwrap();

        // Query by underlying
        let btc_instruments = store
            .list(&InstrumentQuery::new().with_underlying("BTC"))
            .await
            .unwrap();
        assert_eq!(btc_instruments.len(), 2);

        // Query by strike range
        let high_strike = store
            .list(&InstrumentQuery::new().with_strike_range(Some(52000.0), None))
            .await
            .unwrap();
        assert_eq!(high_strike.len(), 1);
        assert_eq!(high_strike[0].strike.value(), 55000.0);
    }

    #[tokio::test]
    async fn test_inmemory_store_duplicate_symbol() {
        let store = InMemoryInstrumentStore::new();
        let instrument1 = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);
        let instrument2 = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);

        store.save(instrument1).await.unwrap();
        let result = store.save(instrument2).await;

        assert!(matches!(result, Err(InstrumentError::AlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_inmemory_store_update_status() {
        let store = InMemoryInstrumentStore::new();
        let instrument = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);
        let id = instrument.id.clone();

        store.save(instrument).await.unwrap();
        store
            .update_status(&id, InstrumentStatus::Expired)
            .await
            .unwrap();

        let updated = store.get(&id).await.unwrap().unwrap();
        assert_eq!(updated.status, InstrumentStatus::Expired);
    }

    #[tokio::test]
    async fn test_inmemory_store_delete() {
        let store = InMemoryInstrumentStore::new();
        let instrument = create_test_instrument("BTC-20241231-50000-C", "BTC", 50000.0);
        let id = instrument.id.clone();

        store.save(instrument).await.unwrap();
        assert_eq!(store.len(), 1);

        store.delete(&id).await.unwrap();
        assert_eq!(store.len(), 0);
        assert!(store.get(&id).await.unwrap().is_none());
    }
}
