//! Background worker service for instrument generation and maintenance.
//!
//! The `InstrumentWorker` monitors spot prices and automatically generates,
//! activates, deactivates, and expires instruments based on the grid configuration.

use crate::db::models::Environment;
use crate::db::postgres::PostgresInstrumentStore;
use crate::error::{InstrumentError, InstrumentResult};
use crate::generator::grid::GridStrikeGenerator;
use crate::generator::state::{DisplacementResult, GenState, GenerationStateManager};
use crate::generator::ExpiryGenerator;
use crate::store::InstrumentStore;
use crate::types::{
    ExerciseStyle, InstrumentId, InstrumentStatus, OptionInstrument, OptionType, Strike,
    UnderlyingAsset,
};
use chrono::Utc;
use config::{AssetGenerationConfig, InstrumentConfig, InstrumentWorkerConfig};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, error, info, instrument, warn};

/// Trait for providing spot prices.
/// Allows different implementations for prod (market data) vs static (config).
#[async_trait::async_trait]
pub trait SpotPriceProvider: Send + Sync {
    /// Get the current spot price for an asset.
    async fn get_spot_price(&self, asset_symbol: &str) -> Option<f64>;
}

/// Static spot price provider - returns fixed prices from config.
pub struct StaticSpotPriceProvider {
    prices: HashMap<String, f64>,
}

impl StaticSpotPriceProvider {
    pub fn new(prices: HashMap<String, f64>) -> Self {
        Self { prices }
    }

    pub fn from_config(config: &config::StaticPricesConfig) -> Self {
        Self {
            prices: config.prices.clone(),
        }
    }
}

#[async_trait::async_trait]
impl SpotPriceProvider for StaticSpotPriceProvider {
    async fn get_spot_price(&self, asset_symbol: &str) -> Option<f64> {
        self.prices.get(asset_symbol).copied()
    }
}

/// Background worker that manages instrument generation.
pub struct InstrumentWorker {
    store: Arc<PostgresInstrumentStore>,
    config: InstrumentConfig,
    worker_config: InstrumentWorkerConfig,
    spot_provider: Arc<dyn SpotPriceProvider>,
    environment: Environment,
}

impl InstrumentWorker {
    /// Create a new InstrumentWorker.
    pub fn new(
        store: Arc<PostgresInstrumentStore>,
        config: InstrumentConfig,
        worker_config: InstrumentWorkerConfig,
        spot_provider: Arc<dyn SpotPriceProvider>,
        environment: Environment,
    ) -> Self {
        Self {
            store,
            config,
            worker_config,
            spot_provider,
            environment,
        }
    }

    /// Run the worker. This blocks and runs forever (or until shutdown signal).
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        info!(
            "Starting InstrumentWorker for environment '{}' (interval={}s, run_on_startup={})",
            self.environment, self.worker_config.interval_seconds, self.worker_config.run_on_startup
        );

        // Run on startup if configured
        if self.worker_config.run_on_startup {
            info!("Running initial generation cycle...");
            if let Err(e) = self.run_cycle().await {
                error!("Initial generation cycle failed: {}", e);
            }
        }

        // For static environment, we only run once
        if self.environment == Environment::Static {
            info!("Static environment - worker completed initial cycle, stopping.");
            return;
        }

        // Run on interval
        let interval = Duration::from_secs(self.worker_config.interval_seconds);
        let mut timer = tokio::time::interval(interval);
        timer.tick().await; // Skip first tick (already ran on startup)

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    if let Err(e) = self.run_cycle().await {
                        error!("Generation cycle failed: {}", e);
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("InstrumentWorker shutting down.");
                        return;
                    }
                }
            }
        }
    }

    /// Run a single generation cycle.
    #[instrument(skip(self))]
    pub async fn run_cycle(&self) -> InstrumentResult<()> {
        info!("Running generation cycle for environment '{}'", self.environment);

        let generation_config = self.config.generation.as_ref().ok_or_else(|| {
            InstrumentError::ConfigError("No generation config".to_string())
        })?;

        // Process each enabled asset
        for asset in &self.config.supported_assets {
            if !asset.enabled {
                debug!("Skipping disabled asset: {}", asset.symbol);
                continue;
            }

            // Get generation config for this asset
            let asset_gen_config = match generation_config.assets.get(&asset.symbol) {
                Some(config) => config,
                None => {
                    debug!("No generation config for asset: {}", asset.symbol);
                    continue;
                }
            };

            // Get spot price
            let spot_price = match self.spot_provider.get_spot_price(&asset.symbol).await {
                Some(price) => price,
                None => {
                    warn!("No spot price available for {}", asset.symbol);
                    continue;
                }
            };

            info!("{}: spot price = {}", asset.symbol, spot_price);

            // Process this asset
            if let Err(e) = self
                .process_asset(&asset.symbol, spot_price, asset_gen_config)
                .await
            {
                error!("Error processing asset {}: {}", asset.symbol, e);
            }
        }

        // Mark expired instruments
        let expired = self.store.mark_expired_by_time().await?;
        if expired > 0 {
            info!("Marked {} instruments as expired", expired);
        }

        info!("Generation cycle complete for '{}'", self.environment);
        Ok(())
    }

    /// Process a single asset - check triggers, generate, update statuses.
    async fn process_asset(
        &self,
        asset_symbol: &str,
        spot_price: f64,
        config: &AssetGenerationConfig,
    ) -> InstrumentResult<()> {
        // Get or initialize generation state
        let db_state = self.store.get_generation_state(asset_symbol).await?;

        let state = match db_state {
            Some(s) => GenerationStateManager::from_db_state(&s),
            None => {
                info!(
                    "{}: No existing state, initializing from spot {}",
                    asset_symbol, spot_price
                );
                let state = GenerationStateManager::initialize(asset_symbol, spot_price, config);

                // Generate initial instruments
                self.generate_initial_instruments(asset_symbol, &state, config)
                    .await?;

                // Save initial state
                let db_state =
                    GenerationStateManager::to_db_state(&state, self.environment);
                self.store.upsert_generation_state(&db_state).await?;

                state
            }
        };

        // Check displacement triggers
        let displacement = GenerationStateManager::check_displacement(spot_price, &state, config);

        let updated_state = match displacement {
            DisplacementResult::UpperCrossed {
                new_reference,
                new_max_strike,
                old_max_strike,
                new_upper_trigger,
            } => {
                // Generate new extension strikes
                let new_strikes = GridStrikeGenerator::generate_extension_strikes(
                    old_max_strike,
                    new_max_strike,
                    config.grid_size,
                    2, // price_decimals - TODO: get from asset config
                    true,
                );

                // Generate instruments for new strikes
                let instrument_count = self
                    .generate_instruments_for_strikes(asset_symbol, &new_strikes)
                    .await?;

                info!(
                    "{}: Upper displacement - {} new strikes, {} instruments created",
                    asset_symbol,
                    new_strikes.len(),
                    instrument_count
                );

                GenerationStateManager::apply_upper_displacement(
                    &state,
                    new_reference,
                    new_max_strike,
                    new_upper_trigger,
                    spot_price,
                )
            }
            DisplacementResult::LowerCrossed {
                new_reference,
                new_min_strike,
                old_min_strike,
                new_lower_trigger,
            } => {
                // Generate new extension strikes
                let new_strikes = GridStrikeGenerator::generate_extension_strikes(
                    old_min_strike,
                    new_min_strike,
                    config.grid_size,
                    2,
                    false,
                );

                let instrument_count = self
                    .generate_instruments_for_strikes(asset_symbol, &new_strikes)
                    .await?;

                info!(
                    "{}: Lower displacement - {} new strikes, {} instruments created",
                    asset_symbol,
                    new_strikes.len(),
                    instrument_count
                );

                GenerationStateManager::apply_lower_displacement(
                    &state,
                    new_reference,
                    new_min_strike,
                    new_lower_trigger,
                    spot_price,
                )
            }
            DisplacementResult::NoChange => {
                debug!("{}: No displacement triggered", asset_symbol);
                GenState {
                    last_spot_price: spot_price,
                    ..state
                }
            }
        };

        // Update active/inactive status based on current spot
        let (active_min, active_max) =
            GridStrikeGenerator::calculate_active_range(spot_price, config);
        let (activated, deactivated) = self
            .store
            .update_active_range(asset_symbol, active_min, active_max)
            .await?;

        if activated > 0 || deactivated > 0 {
            info!(
                "{}: status update - {} activated, {} deactivated (range [{}, {}])",
                asset_symbol, activated, deactivated, active_min, active_max
            );
        }

        // Persist updated state
        let db_state = GenerationStateManager::to_db_state(&updated_state, self.environment);
        self.store.upsert_generation_state(&db_state).await?;

        Ok(())
    }

    /// Generate the initial set of instruments for all strikes and expiries.
    async fn generate_initial_instruments(
        &self,
        asset_symbol: &str,
        state: &GenState,
        config: &AssetGenerationConfig,
    ) -> InstrumentResult<()> {
        // Find asset config
        let asset = self
            .config
            .supported_assets
            .iter()
            .find(|a| a.symbol == asset_symbol)
            .ok_or_else(|| {
                InstrumentError::UnsupportedAsset(asset_symbol.to_string())
            })?;

        // Generate all strikes in the initial range
        let strikes = GridStrikeGenerator::generate_strikes_in_range(
            state.min_strike,
            state.max_strike,
            config.grid_size,
            asset.price_decimals,
        );

        info!(
            "{}: Generating initial instruments for {} strikes",
            asset_symbol,
            strikes.len()
        );

        // Generate instruments for all strikes
        let count = self
            .generate_instruments_for_strikes(asset_symbol, &strikes)
            .await?;

        info!(
            "{}: Generated {} initial instruments",
            asset_symbol, count
        );

        Ok(())
    }

    /// Generate instruments for a set of strikes across all expiries.
    async fn generate_instruments_for_strikes(
        &self,
        asset_symbol: &str,
        strikes: &[Strike],
    ) -> InstrumentResult<usize> {
        if strikes.is_empty() {
            return Ok(0);
        }

        // Find asset config
        let asset = self
            .config
            .supported_assets
            .iter()
            .find(|a| a.symbol == asset_symbol)
            .ok_or_else(|| {
                InstrumentError::UnsupportedAsset(asset_symbol.to_string())
            })?;

        // Get expiry dates
        let expiry_config = self.config.expiry_schedule.as_ref().ok_or_else(|| {
            InstrumentError::ConfigError("No expiry schedule config".to_string())
        })?;
        let expiries = ExpiryGenerator::generate_expiries(expiry_config);

        if expiries.is_empty() {
            warn!("{}: No expiry dates generated", asset_symbol);
            return Ok(0);
        }

        // Get settlement currency
        let settlement_currency = self
            .config
            .settlement_currencies
            .iter()
            .find(|c| c.primary)
            .map(|c| c.symbol.as_str())
            .unwrap_or("USDT");

        // Generate all instruments
        let mut instruments = Vec::new();
        let now = Utc::now();

        for expiry in &expiries {
            for strike in strikes {
                for option_type in [OptionType::Call, OptionType::Put] {
                    let symbol = OptionInstrument::generate_symbol(
                        &asset.symbol,
                        *expiry,
                        strike,
                        option_type,
                    );

                    instruments.push(OptionInstrument {
                        id: InstrumentId::generate(),
                        symbol,
                        underlying: UnderlyingAsset::from_config(asset),
                        option_type,
                        strike: *strike,
                        expiry: *expiry,
                        exercise_style: ExerciseStyle::European,
                        settlement_currency: settlement_currency.to_string(),
                        contract_size: asset.contract_size,
                        tick_size: asset.tick_size,
                        min_order_size: asset.min_order_size,
                        status: InstrumentStatus::Active,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }
        }

        let count = instruments.len();

        // Batch save (duplicates are skipped via ON CONFLICT DO NOTHING)
        self.store.save_batch(instruments).await?;

        Ok(count)
    }

    /// Force a regeneration for a specific asset at a given spot price.
    /// Used by admin API for manual override.
    pub async fn force_regenerate(
        &self,
        asset_symbol: &str,
        spot_price: f64,
    ) -> InstrumentResult<(usize, u64)> {
        let generation_config = self.config.generation.as_ref().ok_or_else(|| {
            InstrumentError::ConfigError("No generation config".to_string())
        })?;

        let asset_gen_config = generation_config.assets.get(asset_symbol).ok_or_else(|| {
            InstrumentError::ConfigError(format!(
                "No generation config for asset: {}",
                asset_symbol
            ))
        })?;

        // Re-initialize state
        let state = GenerationStateManager::initialize(asset_symbol, spot_price, asset_gen_config);

        // Generate instruments
        self.generate_initial_instruments(asset_symbol, &state, asset_gen_config)
            .await?;

        // Update active range
        let (active_min, active_max) =
            GridStrikeGenerator::calculate_active_range(spot_price, asset_gen_config);
        let (activated, deactivated) = self
            .store
            .update_active_range(asset_symbol, active_min, active_max)
            .await?;

        // Save state
        let db_state = GenerationStateManager::to_db_state(&state, self.environment);
        self.store.upsert_generation_state(&db_state).await?;

        let total_updated = activated + deactivated;
        Ok((0, total_updated)) // instruments_created is tracked by save_batch
    }
}
