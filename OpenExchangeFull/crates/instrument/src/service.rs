//! Instrument service - high-level API for instrument management.

use crate::error::InstrumentResult;
use crate::generator::InstrumentGenerator;
use crate::store::{InstrumentQuery, InstrumentStore};
use crate::types::{InstrumentId, InstrumentStatus, OptionInstrument};
use chrono::{DateTime, Utc};
use config::InstrumentConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// High-level service for managing instruments.
///
/// This service provides the main API for:
/// - Generating instruments from configuration
/// - Querying instruments
/// - Managing instrument lifecycle (expiry, settlement)
pub struct InstrumentService<S: InstrumentStore> {
    store: Arc<S>,
    config: InstrumentConfig,
}

impl<S: InstrumentStore> InstrumentService<S> {
    /// Create a new instrument service.
    pub fn new(store: Arc<S>, config: InstrumentConfig) -> Self {
        Self { store, config }
    }

    /// Generate and store instruments based on configuration.
    ///
    /// # Arguments
    /// * `spot_prices` - Current spot prices for each asset
    /// * `settlement_currency` - Settlement currency symbol
    #[instrument(skip(self, spot_prices))]
    pub async fn generate_and_store_instruments(
        &self,
        spot_prices: &HashMap<String, f64>,
        settlement_currency: &str,
    ) -> InstrumentResult<usize> {
        info!("Generating instruments from configuration");

        // Generate instruments
        let instruments =
            InstrumentGenerator::generate_instruments(&self.config, spot_prices, settlement_currency)?;

        let count = instruments.len();
        info!("Generated {} instruments, saving to store", count);

        // Save to store (batch insert)
        self.store.save_batch(instruments).await?;

        info!("Successfully stored {} instruments", count);
        Ok(count)
    }

    /// Get an instrument by its ID.
    pub async fn get(&self, id: &InstrumentId) -> InstrumentResult<Option<OptionInstrument>> {
        self.store.get(id).await
    }

    /// Get an instrument by its symbol.
    pub async fn get_by_symbol(&self, symbol: &str) -> InstrumentResult<Option<OptionInstrument>> {
        self.store.get_by_symbol(symbol).await
    }

    /// List all active instruments for an underlying asset.
    pub async fn list_active_for_underlying(
        &self,
        underlying: &str,
    ) -> InstrumentResult<Vec<OptionInstrument>> {
        self.store.get_active_by_underlying(underlying).await
    }

    /// List instruments matching a query.
    pub async fn list(&self, query: &InstrumentQuery) -> InstrumentResult<Vec<OptionInstrument>> {
        self.store.list(query).await
    }

    /// Count instruments matching a query.
    pub async fn count(&self, query: &InstrumentQuery) -> InstrumentResult<usize> {
        self.store.count(query).await
    }

    /// Get all instruments expiring between two dates.
    pub async fn get_expiring_between(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> InstrumentResult<Vec<OptionInstrument>> {
        let query = InstrumentQuery::new()
            .with_expiry_range(Some(start), Some(end))
            .with_status(InstrumentStatus::Active);
        self.store.list(&query).await
    }

    /// Get all expired instruments that haven't been settled.
    pub async fn get_pending_settlement(&self) -> InstrumentResult<Vec<OptionInstrument>> {
        self.store.get_expired_unsettled().await
    }

    /// Update an instrument's status to expired.
    #[instrument(skip(self))]
    pub async fn mark_expired(&self, id: &InstrumentId) -> InstrumentResult<()> {
        info!("Marking instrument {} as expired", id);
        self.store.update_status(id, InstrumentStatus::Expired).await
    }

    /// Update an instrument's status to settled.
    #[instrument(skip(self))]
    pub async fn mark_settled(&self, id: &InstrumentId) -> InstrumentResult<()> {
        info!("Marking instrument {} as settled", id);
        self.store.update_status(id, InstrumentStatus::Settled).await
    }

    /// Suspend trading on an instrument.
    #[instrument(skip(self))]
    pub async fn suspend(&self, id: &InstrumentId) -> InstrumentResult<()> {
        warn!("Suspending instrument {}", id);
        self.store.update_status(id, InstrumentStatus::Suspended).await
    }

    /// Resume trading on a suspended instrument.
    #[instrument(skip(self))]
    pub async fn resume(&self, id: &InstrumentId) -> InstrumentResult<()> {
        info!("Resuming instrument {}", id);
        self.store.update_status(id, InstrumentStatus::Active).await
    }

    /// Process expired instruments - mark them as expired.
    #[instrument(skip(self))]
    pub async fn process_expirations(&self) -> InstrumentResult<usize> {
        let now = Utc::now();
        let query = InstrumentQuery::new()
            .with_status(InstrumentStatus::Active)
            .with_expiry_range(None, Some(now));

        let expired = self.store.list(&query).await?;
        let count = expired.len();

        for instrument in expired {
            debug!("Marking {} as expired", instrument.symbol);
            self.store
                .update_status(&instrument.id, InstrumentStatus::Expired)
                .await?;
        }

        if count > 0 {
            info!("Processed {} expired instruments", count);
        }

        Ok(count)
    }

    /// Get statistics about instruments.
    pub async fn get_stats(&self) -> InstrumentResult<InstrumentStats> {
        let total = self.store.count(&InstrumentQuery::new()).await?;
        let active = self
            .store
            .count(&InstrumentQuery::new().with_status(InstrumentStatus::Active))
            .await?;
        let expired = self
            .store
            .count(&InstrumentQuery::new().with_status(InstrumentStatus::Expired))
            .await?;
        let suspended = self
            .store
            .count(&InstrumentQuery::new().with_status(InstrumentStatus::Suspended))
            .await?;
        let settled = self
            .store
            .count(&InstrumentQuery::new().with_status(InstrumentStatus::Settled))
            .await?;

        // Count by underlying
        let mut by_underlying = HashMap::new();
        for asset in &self.config.supported_assets {
            if asset.enabled {
                let count = self
                    .store
                    .count(&InstrumentQuery::new().with_underlying(&asset.symbol))
                    .await?;
                by_underlying.insert(asset.symbol.clone(), count);
            }
        }

        Ok(InstrumentStats {
            total,
            active,
            expired,
            suspended,
            settled,
            by_underlying,
        })
    }

    /// Get the underlying store.
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Get the configuration.
    pub fn config(&self) -> &InstrumentConfig {
        &self.config
    }
}

/// Statistics about instruments in the store.
#[derive(Debug, Clone)]
pub struct InstrumentStats {
    /// Total number of instruments.
    pub total: usize,
    /// Number of active instruments.
    pub active: usize,
    /// Number of expired instruments.
    pub expired: usize,
    /// Number of suspended instruments.
    pub suspended: usize,
    /// Number of settled instruments.
    pub settled: usize,
    /// Count by underlying asset.
    pub by_underlying: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryInstrumentStore;
    use config::{Asset, ExpiryConfig, ExpirySchedule, SettlementCurrency};

    fn create_test_config() -> InstrumentConfig {
        InstrumentConfig {
            supported_assets: vec![Asset {
                symbol: "BTC".to_string(),
                name: "Bitcoin".to_string(),
                decimals: 8,
                contract_size: 0.01,
                min_order_size: 1,
                tick_size: 0.5,
                price_decimals: 2,
                enabled: true,
            }],
            settlement_currencies: vec![SettlementCurrency {
                symbol: "USDT".to_string(),
                name: "Tether".to_string(),
                decimals: 6,
                enabled: true,
                primary: true,
                chains: vec![],
            }],
            market_data: None,
            expiry_schedule: Some(ExpiryConfig {
                daily: ExpirySchedule {
                    enabled: true,
                    count: Some(3),
                    expiry_time_utc: "08:00".to_string(),
                    day_of_week: None,
                    day_type: None,
                    months: None,
                    month: None,
                },
                weekly: ExpirySchedule {
                    enabled: false,
                    count: None,
                    expiry_time_utc: "08:00".to_string(),
                    day_of_week: None,
                    day_type: None,
                    months: None,
                    month: None,
                },
                monthly: ExpirySchedule {
                    enabled: false,
                    count: None,
                    expiry_time_utc: "08:00".to_string(),
                    day_of_week: None,
                    day_type: None,
                    months: None,
                    month: None,
                },
                quarterly: ExpirySchedule {
                    enabled: false,
                    count: None,
                    expiry_time_utc: "08:00".to_string(),
                    day_of_week: None,
                    day_type: None,
                    months: None,
                    month: None,
                },
                yearly: ExpirySchedule {
                    enabled: false,
                    count: None,
                    expiry_time_utc: "08:00".to_string(),
                    day_of_week: None,
                    day_type: None,
                    months: None,
                    month: None,
                },
            }),
            storage: None,
            cache: None,
            generation: None,
            worker: None,
            static_prices: None,
        }
    }

    #[tokio::test]
    async fn test_service_generate_and_store() {
        let store = Arc::new(InMemoryInstrumentStore::new());
        let config = create_test_config();
        let service = InstrumentService::new(store.clone(), config);

        let mut spot_prices = HashMap::new();
        spot_prices.insert("BTC".to_string(), 50000.0);

        let count = service
            .generate_and_store_instruments(&spot_prices, "USDT")
            .await
            .unwrap();

        assert!(count > 0);
        assert_eq!(store.len(), count);
    }

    #[tokio::test]
    async fn test_service_list_active() {
        let store = Arc::new(InMemoryInstrumentStore::new());
        let config = create_test_config();
        let service = InstrumentService::new(store.clone(), config);

        let mut spot_prices = HashMap::new();
        spot_prices.insert("BTC".to_string(), 50000.0);

        service
            .generate_and_store_instruments(&spot_prices, "USDT")
            .await
            .unwrap();

        let active = service.list_active_for_underlying("BTC").await.unwrap();
        assert!(!active.is_empty());

        // All should be active
        for instrument in &active {
            assert_eq!(instrument.status, InstrumentStatus::Active);
        }
    }

    #[tokio::test]
    async fn test_service_stats() {
        let store = Arc::new(InMemoryInstrumentStore::new());
        let config = create_test_config();
        let service = InstrumentService::new(store.clone(), config);

        let mut spot_prices = HashMap::new();
        spot_prices.insert("BTC".to_string(), 50000.0);

        let count = service
            .generate_and_store_instruments(&spot_prices, "USDT")
            .await
            .unwrap();

        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats.total, count);
        assert_eq!(stats.active, count);
        assert_eq!(stats.expired, 0);
        assert!(stats.by_underlying.contains_key("BTC"));
    }
}
