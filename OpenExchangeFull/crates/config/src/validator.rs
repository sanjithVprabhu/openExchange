use crate::*;
use regex::Regex;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("Exchange name is required")]
    MissingExchangeName,
    
    #[error("Exchange description is required")]
    MissingExchangeDescription,
    
    #[error("Invalid version format: {0}. Must be in format X.Y.Z (e.g., 1.0.0)")]
    InvalidVersionFormat(String),
    
    #[error("Invalid exchange mode: {0}. Must be one of: production, virtual, both")]
    InvalidExchangeMode(String),
    
    #[error("No supported assets defined")]
    NoSupportedAssets,
    
    #[error("Asset {symbol}: {message}")]
    InvalidAsset { symbol: String, message: String },
    
    #[error("At least one supported asset must be enabled")]
    NoEnabledAssets,
    
    #[error("No settlement currencies defined")]
    NoSettlementCurrencies,
    
    #[error("Settlement currency {symbol}: {message}")]
    InvalidSettlementCurrency { symbol: String, message: String },
    
    #[error("At least one settlement currency must be enabled")]
    NoEnabledCurrencies,
    
    #[error("Exactly one settlement currency must be marked as primary, found {count}")]
    InvalidPrimaryCurrencyCount { count: usize },
    
    #[error("Market data provider '{name}': {message}")]
    InvalidProvider { name: String, message: String },
    
    #[error("At least one market data provider must be enabled")]
    NoEnabledProviders,
    
    #[error("Exactly one provider must be marked as primary, found {count}")]
    InvalidPrimaryProviderCount { count: usize },
    
    #[error("Duplicate stream for same asset '{asset}' in provider '{provider}'")]
    DuplicateStream { asset: String, provider: String },
    
    #[error("Invalid fallback strategy: {0}. Must be one of: median, average, high, low")]
    InvalidFallbackStrategy(String),
    
    #[error("max_price_age_seconds must be a positive integer, got: {0}")]
    InvalidMaxPriceAge(i64),
    
    #[error("Expiry schedule '{schedule}': {message}")]
    InvalidExpirySchedule { schedule: String, message: String },
    
    #[error("Option expiry not clearly defined in config. All daily, weekly, monthly, quarterly and yearly need to be defined")]
    MissingExpirySchedules,
    
    #[error("Invalid time format '{time}': {message}")]
    InvalidTimeFormat { time: String, message: String },
    
    #[error("Storage: {message}")]
    InvalidStorage { message: String },
    
    #[error("Cache: {message}")]
    InvalidCache { message: String },
    
    #[error("OMS: {message}")]
    InvalidOms { message: String },
    
    #[error("stop_limit and stop_market are not supported in this version")]
    StopOrdersNotSupported,
    
    #[error("{field} must be a positive integer")]
    InvalidPositiveInteger { field: String },
    
    #[error("{field} must be a positive float")]
    InvalidPositiveFloat { field: String },
    
    #[error("{field} must be between 0 and 100")]
    InvalidPercentageRange { field: String },
    
    #[error("Matching engine: {message}")]
    InvalidMatchingEngine { message: String },
    
    #[error("Risk engine: {message}")]
    InvalidRiskEngine { message: String },
    
    #[error("Only simplified_span margin method is supported in this version")]
    InvalidMarginMethod,
    
    #[error("Asset symbol mismatch: instrument has '{instrument_symbol}' but risk config has '{risk_symbol}'")]
    AssetSymbolMismatch { instrument_symbol: String, risk_symbol: String },
    
    #[error("Environment variable '{var}' is missing or invalid: {message}")]
    InvalidEnvVar { var: String, message: String },
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DefaultApplied {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub defaults_applied: Vec<DefaultApplied>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            defaults_applied: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, field: &str, message: &str) {
        self.warnings.push(ValidationWarning {
            field: field.to_string(),
            message: message.to_string(),
        });
    }

    pub fn add_default(&mut self, field: &str, value: &str) {
        self.defaults_applied.push(DefaultApplied {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

pub fn validate_config(config: &MasterConfig) -> ValidationReport {
    let mut report = ValidationReport::new();

    validate_exchange(&config.exchange, &mut report);
    validate_instruments(&config.instrument, &mut report);

    report
}

fn validate_exchange(exchange: &ExchangeConfig, report: &mut ValidationReport) {
    if exchange.name.is_empty() {
        report.add_error(ValidationError::MissingExchangeName);
    }

    if exchange.description.is_empty() {
        report.add_error(ValidationError::MissingExchangeDescription);
    }

    let version_regex = Regex::new(r"^\d+\.\d+\.\d+$").unwrap();
    if !version_regex.is_match(&exchange.version) {
        report.add_error(ValidationError::InvalidVersionFormat(exchange.version.clone()));
    }

    let mode_str = match exchange.mode {
        ExchangeMode::Production => "production",
        ExchangeMode::Virtual => "virtual",
        ExchangeMode::Both => "both",
    };
    
    let valid_modes = ["production", "virtual", "both"];
    if !valid_modes.contains(&mode_str) {
        report.add_error(ValidationError::InvalidExchangeMode(mode_str.to_string()));
    }

    if exchange.trading_hours.hours_type != "24/7" {
        report.add_warning(
            "exchange.trading_hours.type",
            "Only 24/7 trading hours are supported in version 1.0.0",
        );
    }
}

fn validate_instruments(instrument: &InstrumentConfig, report: &mut ValidationReport) {
    if instrument.supported_assets.is_empty() {
        report.add_error(ValidationError::NoSupportedAssets);
        return;
    }

    let mut enabled_count = 0;
    for asset in &instrument.supported_assets {
        validate_asset(asset, report);
        if asset.enabled {
            enabled_count += 1;
        }
    }

    if enabled_count == 0 {
        report.add_error(ValidationError::NoEnabledAssets);
    }

    if instrument.settlement_currencies.is_empty() {
        report.add_error(ValidationError::NoSettlementCurrencies);
        return;
    }

    let mut enabled_currencies = 0;
    let mut primary_currencies = 0;

    for currency in &instrument.settlement_currencies {
        validate_settlement_currency(currency, report);
        if currency.enabled {
            enabled_currencies += 1;
        }
        if currency.primary {
            primary_currencies += 1;
        }
    }

    if enabled_currencies == 0 {
        report.add_error(ValidationError::NoEnabledCurrencies);
    }

    if primary_currencies != 1 {
        report.add_error(ValidationError::InvalidPrimaryCurrencyCount {
            count: primary_currencies,
        });
    }
}

fn validate_asset(asset: &Asset, report: &mut ValidationReport) {
    if asset.symbol.is_empty() {
        report.add_error(ValidationError::InvalidAsset {
            symbol: "unknown".to_string(),
            message: "Symbol is required".to_string(),
        });
    }

    if asset.name.is_empty() {
        report.add_error(ValidationError::InvalidAsset {
            symbol: asset.symbol.clone(),
            message: "Name is required".to_string(),
        });
    }

    if asset.contract_size <= 0.0 {
        report.add_error(ValidationError::InvalidAsset {
            symbol: asset.symbol.clone(),
            message: format!("contract_size must be positive, got: {}", asset.contract_size),
        });
    }

    if asset.min_order_size == 0 {
        report.add_error(ValidationError::InvalidAsset {
            symbol: asset.symbol.clone(),
            message: "min_order_size must be a positive integer".to_string(),
        });
    }

    if asset.tick_size <= 0.0 {
        report.add_error(ValidationError::InvalidAsset {
            symbol: asset.symbol.clone(),
            message: format!("tick_size must be positive, got: {}", asset.tick_size),
        });
    }

    if asset.price_decimals == 0 {
        report.add_error(ValidationError::InvalidAsset {
            symbol: asset.symbol.clone(),
            message: "price_decimals must be a positive integer".to_string(),
        });
    }
}

fn validate_settlement_currency(currency: &SettlementCurrency, report: &mut ValidationReport) {
    if currency.symbol.is_empty() {
        report.add_error(ValidationError::InvalidSettlementCurrency {
            symbol: "unknown".to_string(),
            message: "Symbol is required".to_string(),
        });
    }

    if currency.name.is_empty() {
        report.add_error(ValidationError::InvalidSettlementCurrency {
            symbol: currency.symbol.clone(),
            message: "Name is required".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_market_data(market_data: &MarketDataConfig, report: &mut ValidationReport) {
    if market_data.providers.is_empty() {
        report.add_error(ValidationError::InvalidProvider {
            name: "unknown".to_string(),
            message: "At least one provider must be defined".to_string(),
        });
        return;
    }

    let valid_strategies = ["median", "average", "high", "low"];
    if !valid_strategies.contains(&market_data.fallback_strategy.as_str()) {
        report.add_error(ValidationError::InvalidFallbackStrategy(
            market_data.fallback_strategy.clone(),
        ));
    }

    if market_data.max_price_age_seconds == 0 {
        report.add_error(ValidationError::InvalidMaxPriceAge(0));
    }

    let mut enabled_providers = 0;
    let mut primary_providers = 0;

    for provider in &market_data.providers {
        validate_provider(provider, report);
        if provider.enabled {
            enabled_providers += 1;
        }
        if provider.primary {
            primary_providers += 1;
        }
    }

    if enabled_providers == 0 {
        report.add_error(ValidationError::NoEnabledProviders);
    }

    if primary_providers != 1 {
        report.add_error(ValidationError::InvalidPrimaryProviderCount {
            count: primary_providers,
        });
    }
}

#[allow(dead_code)]
fn validate_provider(provider: &MarketDataProvider, report: &mut ValidationReport) {
    if provider.name.is_empty() {
        report.add_error(ValidationError::InvalidProvider {
            name: "unknown".to_string(),
            message: "Provider name is required".to_string(),
        });
    }

    let valid_types = ["websocket", "grpc", "rest"];
    if !valid_types.contains(&provider.provider_type.as_str()) {
        report.add_error(ValidationError::InvalidProvider {
            name: provider.name.clone(),
            message: format!(
                "Invalid provider type '{}'. Must be one of: websocket, grpc, rest",
                provider.provider_type
            ),
        });
    }

    // Check for duplicate streams
    let mut seen_assets = std::collections::HashSet::new();
    for stream in &provider.streams {
        if !seen_assets.insert(stream.name.clone()) {
            report.add_error(ValidationError::DuplicateStream {
                asset: stream.name.clone(),
                provider: provider.name.clone(),
            });
        }

        if let Some(ref endpoint) = stream.endpoint {
            if endpoint.is_empty() {
                report.add_error(ValidationError::InvalidProvider {
                    name: provider.name.clone(),
                    message: format!("Stream '{}' has empty endpoint", stream.name),
                });
            }
        }
    }

    // Validate websocket-specific fields
    if provider.provider_type == "websocket" {
        if provider.reconnect_delay_seconds.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.reconnect_delay_seconds", provider.name),
                "5",
            );
        }
        if provider.max_reconnect_attempts.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.max_reconnect_attempts", provider.name),
                "10",
            );
        }
        if provider.heartbeat_interval_seconds.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.heartbeat_interval_seconds", provider.name),
                "30",
            );
        }
        if provider.connection_timeout_seconds.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.connection_timeout_seconds", provider.name),
                "60",
            );
        }
    }

    // Validate rest-specific fields
    if provider.provider_type == "rest" {
        if provider.rate_limit_per_second.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.rate_limit_per_second", provider.name),
                "10",
            );
        }
        if provider.timeout_seconds.is_none() {
            report.add_default(
                &format!("market_data.providers.{}.timeout_seconds", provider.name),
                "5",
            );
        }
    }
}

#[allow(dead_code)]
fn validate_expiry(expiry: &ExpiryConfig, report: &mut ValidationReport) {
    validate_expiry_schedule("daily", &expiry.daily, report);
    validate_expiry_schedule("weekly", &expiry.weekly, report);
    validate_expiry_schedule("monthly", &expiry.monthly, report);
    validate_expiry_schedule("quarterly", &expiry.quarterly, report);
    validate_expiry_schedule("yearly", &expiry.yearly, report);
}

#[allow(dead_code)]
fn validate_expiry_schedule(name: &str, schedule: &ExpirySchedule, report: &mut ValidationReport) {
    // count is required for daily, weekly, monthly schedules
    if let Some(count) = schedule.count {
        if count == 0 {
            report.add_error(ValidationError::InvalidExpirySchedule {
                schedule: name.to_string(),
                message: "count must be a positive integer".to_string(),
            });
        }
    }

    // Validate time format HH:MM
    let time_regex = Regex::new(r"^([0-1]?[0-9]|2[0-3]):[0-5][0-9]$").unwrap();
    if !time_regex.is_match(&schedule.expiry_time_utc) {
        report.add_error(ValidationError::InvalidTimeFormat {
            time: schedule.expiry_time_utc.clone(),
            message: "Time must be in 24-hour HH:MM format".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_storage(storage: &StorageConfig, report: &mut ValidationReport) {
    let valid_types = ["postgres", "supabase"];
    if !valid_types.contains(&storage.storage_type.as_str()) {
        report.add_error(ValidationError::InvalidStorage {
            message: format!(
                "Invalid storage type '{}'. Must be one of: postgres, supabase",
                storage.storage_type
            ),
        });
    }

    if storage.storage_type == "postgres" {
        if let Some(ref pg) = storage.postgres {
            validate_postgres_config(pg, report);
        } else {
            report.add_error(ValidationError::InvalidStorage {
                message: "Storage type is 'postgres' but postgres configuration is missing".to_string(),
            });
        }
    }

    if storage.storage_type == "supabase" {
        if let Some(ref sb) = storage.supabase {
            validate_supabase_config(sb, report);
        } else {
            report.add_error(ValidationError::InvalidStorage {
                message: "Storage type is 'supabase' but supabase configuration is missing".to_string(),
            });
        }
    }

    if let Some(ref cache) = storage.cache {
        validate_cache_config(cache, report);
    }
}

#[allow(dead_code)]
fn validate_postgres_config(pg: &PostgresConfig, report: &mut ValidationReport) {
    if pg.port == 0 {
        report.add_error(ValidationError::InvalidStorage {
            message: "port must be a positive integer".to_string(),
        });
    }

    if pg.max_connections == 0 {
        report.add_error(ValidationError::InvalidStorage {
            message: "max_connections must be a positive integer".to_string(),
        });
    }

    if pg.connection_timeout_seconds == 0 {
        report.add_error(ValidationError::InvalidStorage {
            message: "connection_timeout_seconds must be a positive integer".to_string(),
        });
    }

    if pg.idle_timeout_seconds == 0 {
        report.add_error(ValidationError::InvalidStorage {
            message: "idle_timeout_seconds must be a positive integer".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_supabase_config(sb: &SupabaseConfig, report: &mut ValidationReport) {
    if sb.url.is_empty() || sb.url.starts_with("${") {
        report.add_error(ValidationError::InvalidEnvVar {
            var: "SUPABASE_URL".to_string(),
            message: "supabase url is missing or invalid".to_string(),
        });
    }

    if sb.anon_key.is_empty() || sb.anon_key.starts_with("${") {
        report.add_error(ValidationError::InvalidEnvVar {
            var: "SUPABASE_ANON_KEY".to_string(),
            message: "supabase anon key is missing or invalid".to_string(),
        });
    }

    if sb.service_role_key.is_empty() || sb.service_role_key.starts_with("${") {
        report.add_error(ValidationError::InvalidEnvVar {
            var: "SUPABASE_SERVICE_KEY".to_string(),
            message: "supabase service role key is missing or invalid".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_cache_config(cache: &CacheConfig, report: &mut ValidationReport) {
    if cache.ttl_seconds == 0 {
        report.add_error(ValidationError::InvalidCache {
            message: "ttl_seconds must be a positive integer".to_string(),
        });
    }

    if cache.max_entries == 0 {
        report.add_error(ValidationError::InvalidCache {
            message: "max_entries must be a positive integer".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_oms(oms: &OmsConfig, report: &mut ValidationReport) {
    // Check for unsupported order types
    if oms.order_types.stop_limit.enabled {
        report.add_error(ValidationError::StopOrdersNotSupported);
    }

    if oms.order_types.stop_market.enabled {
        report.add_error(ValidationError::StopOrdersNotSupported);
    }

    // Validate limits
    if oms.limits.max_open_orders_per_user == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "max_open_orders_per_user".to_string(),
        });
    }

    if oms.limits.max_order_size_contracts == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "max_order_size_contracts".to_string(),
        });
    }

    if oms.limits.min_order_size_contracts == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "min_order_size_contracts".to_string(),
        });
    }

    if oms.limits.max_price_deviation_percent <= 0.0 {
        report.add_error(ValidationError::InvalidPositiveFloat {
            field: "max_price_deviation_percent".to_string(),
        });
    }

    // Validate orderbook config
    if oms.orderbook.depth_levels == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "depth_levels".to_string(),
        });
    }

    if oms.orderbook.update_frequency_ms == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "update_frequency_ms".to_string(),
        });
    }

    // Validate storage
    validate_storage(&oms.storage, report);
}

#[allow(dead_code)]
fn validate_matching_engine(engine: &MatchingEngineConfig, report: &mut ValidationReport) {
    if engine.algorithm != "price_time_priority" {
        report.add_warning(
            "matching_engine.algorithm",
            &format!("Algorithm '{}' may not be supported. Using price_time_priority.", engine.algorithm),
        );
    }

    if engine.performance.matching_frequency_ms == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "matching_frequency_ms".to_string(),
        });
    }

    if engine.performance.batch_size == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "batch_size".to_string(),
        });
    }

    let valid_store_types = ["redis", "inmemory"];
    if !valid_store_types.contains(&engine.orderbook_store.store_type.as_str()) {
        report.add_error(ValidationError::InvalidMatchingEngine {
            message: format!(
                "Invalid orderbook_store type '{}'. Must be one of: redis, inmemory",
                engine.orderbook_store.store_type
            ),
        });
    }

    // Validate execution config
    if engine.execution.max_partial_fills == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "max_partial_fills".to_string(),
        });
    }

    // Validate circuit breakers
    if engine.circuit_breakers.price_movement.percent_threshold <= 0.0
        || engine.circuit_breakers.price_movement.percent_threshold > 100.0
    {
        report.add_error(ValidationError::InvalidPercentageRange {
            field: "circuit_breakers.price_movement.percent_threshold".to_string(),
        });
    }

    if engine.circuit_breakers.liquidity.max_spread_percent <= 0.0
        || engine.circuit_breakers.liquidity.max_spread_percent > 100.0
    {
        report.add_error(ValidationError::InvalidPercentageRange {
            field: "circuit_breakers.liquidity.max_spread_percent".to_string(),
        });
    }
}

#[allow(dead_code)]
fn validate_risk_engine(
    risk: &RiskEngineConfig,
    instrument: &InstrumentConfig,
    report: &mut ValidationReport,
) {
    if risk.margin_method != "simplified_span" {
        report.add_error(ValidationError::InvalidMarginMethod);
    }

    // Validate initial margin percentages
    for margin in &risk.initial_margin {
        if margin.percentage <= 0.0 || margin.percentage > 1.0 {
            report.add_error(ValidationError::InvalidRiskEngine {
                message: format!(
                    "Initial margin for {} must be between 0 and 1, got: {}",
                    margin.symbol, margin.percentage
                ),
            });
        }

        // Check if symbol exists in instruments
        if !instrument
            .supported_assets
            .iter()
            .any(|a| a.symbol == margin.symbol)
        {
            report.add_warning(
                &format!("risk_engine.initial_margin.{}", margin.symbol),
                &format!(
                    "Symbol '{}' in risk config not found in supported_assets",
                    margin.symbol
                ),
            );
        }
    }

    // Validate maintenance margin percentages
    for margin in &risk.maintenance_margin {
        if margin.percentage <= 0.0 || margin.percentage > 1.0 {
            report.add_error(ValidationError::InvalidRiskEngine {
                message: format!(
                    "Maintenance margin for {} must be between 0 and 1, got: {}",
                    margin.symbol, margin.percentage
                ),
            });
        }
    }

    // Validate position limits
    if risk.position_limits.max_contracts_per_instrument == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "max_contracts_per_instrument".to_string(),
        });
    }

    // Validate Greeks
    if risk.greeks.calculation_frequency_seconds == 0 {
        report.add_error(ValidationError::InvalidPositiveInteger {
            field: "calculation_frequency_seconds".to_string(),
        });
    }

    let valid_vol_types = ["implied", "historical", "manual"];
    if !valid_vol_types.contains(&risk.greeks.volatility.volatility_type.as_str()) {
        report.add_warning(
            "risk_engine.greeks.volatility.type",
            &format!(
                "Invalid volatility type '{}'. Using 'implied'.",
                risk.greeks.volatility.volatility_type
            ),
        );
    }

    // Validate storage
    validate_storage(&risk.storage, report);
}