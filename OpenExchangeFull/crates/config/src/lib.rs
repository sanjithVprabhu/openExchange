use serde::{Deserialize, Serialize};
use serde::de::{Deserializer, MapAccess, Visitor};
use std::collections::HashMap;
use std::fmt;

pub mod defaults;
pub mod parser;
pub mod substitution;
pub mod validator;

pub use defaults::*;
pub use parser::*;
pub use substitution::*;
pub use validator::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub description: String,
    pub version: String,
    pub mode: ExchangeMode,
    #[serde(default = "default_trading_hours")]
    pub trading_hours: TradingHours,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeMode {
    Production,
    Virtual,
    Both,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradingHours {
    #[serde(rename = "type")]
    pub hours_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstrumentConfig {
    #[serde(rename = "supported_assets")]
    pub supported_assets: Vec<Asset>,
    #[serde(rename = "settlement_currencies")]
    pub settlement_currencies: Vec<SettlementCurrency>,
    #[serde(rename = "market_data")]
    #[serde(default)]
    pub market_data: Option<MarketDataConfig>,
    #[serde(rename = "expiry_schedule")]
    #[serde(default)]
    pub expiry_schedule: Option<ExpiryConfig>,
    #[serde(default)]
    pub storage: Option<StorageConfig>,
    #[serde(default)]
    pub cache: Option<CacheConfig>,
    /// Strike generation rules per asset
    #[serde(default)]
    pub generation: Option<GenerationRulesConfig>,
    /// Background worker configuration
    #[serde(default)]
    pub worker: Option<InstrumentWorkerConfig>,
    /// Static prices for testing (used in static environment)
    #[serde(default)]
    pub static_prices: Option<StaticPricesConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Asset {
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    #[serde(rename = "contract_size")]
    pub contract_size: f64,
    #[serde(rename = "min_order_size")]
    pub min_order_size: u64,
    #[serde(rename = "tick_size")]
    pub tick_size: f64,
    #[serde(rename = "price_decimals")]
    pub price_decimals: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementCurrency {
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    pub enabled: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub chains: Vec<ChainConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainConfig {
    pub chain: String,
    pub chain_id: u64,
    pub contract_address: String,
    pub rpc_url: String,
    #[serde(default = "default_gas_limit")]
    pub gas_limit: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDataConfig {
    pub providers: Vec<MarketDataProvider>,
    #[serde(rename = "fallback_strategy")]
    pub fallback_strategy: String,
    #[serde(rename = "max_price_age_seconds")]
    pub max_price_age_seconds: u64,
    #[serde(rename = "stale_price_action")]
    pub stale_price_action: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDataProvider {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub streams: Vec<MarketDataStream>,
    #[serde(rename = "reconnect_delay_seconds")]
    #[serde(default)]
    pub reconnect_delay_seconds: Option<u64>,
    #[serde(rename = "max_reconnect_attempts")]
    #[serde(default)]
    pub max_reconnect_attempts: Option<u64>,
    #[serde(rename = "heartbeat_interval_seconds")]
    #[serde(default)]
    pub heartbeat_interval_seconds: Option<u64>,
    #[serde(rename = "connection_timeout_seconds")]
    #[serde(default)]
    pub connection_timeout_seconds: Option<u64>,
    #[serde(rename = "timeout_seconds")]
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(rename = "rate_limit_per_second")]
    #[serde(default)]
    pub rate_limit_per_second: Option<u64>,
    #[serde(default)]
    pub auth: Option<AuthConfig>,
    /// TLS settings for gRPC providers
    #[serde(rename = "tls_enabled")]
    #[serde(default)]
    pub tls_enabled: Option<bool>,
    #[serde(rename = "cert_path")]
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(rename = "key_path")]
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDataStream {
    pub name: String,
    pub symbol: String,
    pub ticker: String,
    /// Optional endpoint override for this specific stream
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(rename = "api_key")]
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(rename = "api_secret")]
    #[serde(default)]
    pub api_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpiryConfig {
    pub daily: ExpirySchedule,
    pub weekly: ExpirySchedule,
    pub monthly: ExpirySchedule,
    pub quarterly: ExpirySchedule,
    pub yearly: ExpirySchedule,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExpirySchedule {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Number of expiries to generate (daily/weekly/monthly)
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(rename = "expiry_time_utc")]
    pub expiry_time_utc: String,
    /// Day of week for weekly expiries (e.g., "Friday")
    #[serde(rename = "day_of_week")]
    #[serde(default)]
    pub day_of_week: Option<String>,
    /// Day type for monthly/quarterly/yearly (e.g., "last_friday")
    #[serde(rename = "day_type")]
    #[serde(default)]
    pub day_type: Option<String>,
    /// Specific months for quarterly expiries (e.g., [3, 6, 9, 12])
    #[serde(default)]
    pub months: Option<Vec<u32>>,
    /// Specific month for yearly expiries (e.g., 12 for December)
    #[serde(default)]
    pub month: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    #[serde(default)]
    pub postgres: Option<PostgresConfig>,
    #[serde(default)]
    pub supabase: Option<SupabaseConfig>,
    #[serde(default)]
    pub cache: Option<CacheConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostgresConfig {
    pub host: String,
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    #[serde(rename = "ssl_mode")]
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    #[serde(rename = "max_connections")]
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(rename = "connection_timeout_seconds")]
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_seconds: u64,
    #[serde(rename = "idle_timeout_seconds")]
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupabaseConfig {
    pub url: String,
    #[serde(rename = "anon_key")]
    pub anon_key: String,
    #[serde(rename = "service_role_key")]
    pub service_role_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "ttl_seconds")]
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: u64,
    #[serde(rename = "max_entries")]
    #[serde(default = "default_max_entries")]
    pub max_entries: u64,
}

// ==================================================================================
// INSTRUMENT GENERATION CONFIG
// ==================================================================================

/// Strike generation rules configuration - maps asset symbol to generation parameters
/// Example YAML:
/// ```yaml
/// generation:
///   BTC:
///     grid_size: 1000
///     upper_bound: 20000
///     lower_bound: 20000
///     upper_disp: 15000
///     lower_disp: 15000
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GenerationRulesConfig {
    /// Generation rules keyed by asset symbol (e.g., "BTC", "ETH", "SOL")
    #[serde(flatten)]
    pub assets: HashMap<String, AssetGenerationConfig>,
}

/// Generation parameters for a single asset
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetGenerationConfig {
    /// Strike price increment (e.g., $1000 for BTC)
    pub grid_size: f64,
    /// Maximum distance above reference point for strike generation
    pub upper_bound: f64,
    /// Maximum distance below reference point for strike generation
    pub lower_bound: f64,
    /// Upper displacement trigger - when spot crosses this, generate new instruments
    pub upper_disp: f64,
    /// Lower displacement trigger - when spot crosses this, generate new instruments
    pub lower_disp: f64,
}

/// Background worker configuration for instrument generation
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InstrumentWorkerConfig {
    /// Whether the worker is enabled
    #[serde(default = "default_worker_enabled")]
    pub enabled: bool,
    /// Interval between worker runs in seconds
    #[serde(default = "default_worker_interval")]
    pub interval_seconds: u64,
    /// Whether to run generation cycle immediately on startup
    #[serde(default = "default_run_on_startup")]
    pub run_on_startup: bool,
}

impl Default for InstrumentWorkerConfig {
    fn default() -> Self {
        Self {
            enabled: default_worker_enabled(),
            interval_seconds: default_worker_interval(),
            run_on_startup: default_run_on_startup(),
        }
    }
}

/// Static prices configuration for testing/static environment
/// Maps asset symbol to fixed price value
/// Example YAML:
/// ```yaml
/// static_prices:
///   BTC: 50000.0
///   ETH: 3000.0
///   SOL: 100.0
/// ```
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StaticPricesConfig {
    /// Static prices keyed by asset symbol
    #[serde(flatten)]
    pub prices: HashMap<String, f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OmsConfig {
    #[serde(rename = "order_types")]
    pub order_types: OrderTypesConfig,
    #[serde(rename = "time_in_force")]
    pub time_in_force: TimeInForceConfig,
    pub limits: OmsLimits,
    pub orderbook: OrderbookConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderTypesConfig {
    pub limit: OrderTypeEnabled,
    pub market: OrderTypeEnabled,
    #[serde(rename = "stop_limit")]
    pub stop_limit: OrderTypeEnabled,
    #[serde(rename = "stop_market")]
    pub stop_market: OrderTypeEnabled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderTypeEnabled {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeInForceConfig {
    pub gtc: TifEnabled,
    pub ioc: TifEnabled,
    pub fok: TifEnabled,
    pub day: TifEnabled,
}

impl<'de> Deserialize<'de> for TimeInForceConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimeInForceVisitor;

        impl<'de> Visitor<'de> for TimeInForceVisitor {
            type Value = TimeInForceConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map with time-in-force settings (gtc, ioc, fok, day or GTC, IOC, FOK, DAY)")
            }

            fn visit_map<V>(self, mut map: V) -> Result<TimeInForceConfig, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut gtc = None;
                let mut ioc = None;
                let mut fok = None;
                let mut day = None;

                while let Some(key) = map.next_key::<String>()? {
                    let key_lower = key.to_lowercase();
                    match key_lower.as_str() {
                        "gtc" => {
                            if gtc.is_some() {
                                return Err(serde::de::Error::duplicate_field("gtc/GTC"));
                            }
                            gtc = Some(map.next_value()?);
                        }
                        "ioc" => {
                            if ioc.is_some() {
                                return Err(serde::de::Error::duplicate_field("ioc/IOC"));
                            }
                            ioc = Some(map.next_value()?);
                        }
                        "fok" => {
                            if fok.is_some() {
                                return Err(serde::de::Error::duplicate_field("fok/FOK"));
                            }
                            fok = Some(map.next_value()?);
                        }
                        "day" => {
                            if day.is_some() {
                                return Err(serde::de::Error::duplicate_field("day/DAY"));
                            }
                            day = Some(map.next_value()?);
                        }
                        _ => {
                            // Skip unknown fields
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                Ok(TimeInForceConfig {
                    gtc: gtc.ok_or_else(|| serde::de::Error::missing_field("gtc/GTC"))?,
                    ioc: ioc.ok_or_else(|| serde::de::Error::missing_field("ioc/IOC"))?,
                    fok: fok.ok_or_else(|| serde::de::Error::missing_field("fok/FOK"))?,
                    day: day.ok_or_else(|| serde::de::Error::missing_field("day/DAY"))?,
                })
            }
        }

        deserializer.deserialize_map(TimeInForceVisitor)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TifEnabled {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OmsLimits {
    #[serde(rename = "max_open_orders_per_user")]
    pub max_open_orders_per_user: u64,
    #[serde(rename = "max_order_size_contracts")]
    pub max_order_size_contracts: u64,
    #[serde(rename = "min_order_size_contracts")]
    pub min_order_size_contracts: u64,
    #[serde(rename = "max_price_deviation_percent")]
    pub max_price_deviation_percent: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderbookConfig {
    #[serde(rename = "depth_levels")]
    pub depth_levels: u32,
    #[serde(rename = "update_frequency_ms")]
    pub update_frequency_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchingEngineConfig {
    pub algorithm: String,
    pub performance: PerformanceConfig,
    #[serde(rename = "orderbook_store")]
    pub orderbook_store: OrderbookStoreConfig,
    pub execution: ExecutionConfig,
    #[serde(rename = "circuit_breakers")]
    pub circuit_breakers: CircuitBreakersConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    #[serde(rename = "matching_frequency_ms")]
    pub matching_frequency_ms: u64,
    #[serde(rename = "batch_size")]
    pub batch_size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderbookStoreConfig {
    #[serde(rename = "type")]
    pub store_type: String,
    #[serde(default)]
    pub redis: Option<RedisConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedisConfig {
    pub host: String,
    #[serde(default = "default_redis_port")]
    pub port: u16,
    pub password: String,
    #[serde(rename = "cluster_mode")]
    #[serde(default)]
    pub cluster_mode: bool,
    #[serde(rename = "db_index")]
    #[serde(default)]
    pub db_index: u8,
    #[serde(rename = "persistence_enabled")]
    #[serde(default = "default_persistence_enabled")]
    pub persistence_enabled: bool,
    #[serde(rename = "snapshot_interval_seconds")]
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionConfig {
    #[serde(rename = "atomic_trades")]
    #[serde(default = "default_atomic_trades")]
    pub atomic_trades: bool,
    #[serde(rename = "max_partial_fills")]
    #[serde(default = "default_max_partial_fills")]
    pub max_partial_fills: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakersConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "price_movement")]
    pub price_movement: PriceMovementConfig,
    pub liquidity: LiquidityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PriceMovementConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "percent_threshold")]
    pub percent_threshold: f64,
    #[serde(rename = "time_window_seconds")]
    pub time_window_seconds: u64,
    #[serde(rename = "halt_duration_seconds")]
    pub halt_duration_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LiquidityConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "min_bid_ask_orders")]
    pub min_bid_ask_orders: u64,
    #[serde(rename = "max_spread_percent")]
    pub max_spread_percent: f64,
    #[serde(rename = "halt_duration_seconds")]
    pub halt_duration_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskEngineConfig {
    #[serde(rename = "margin_method")]
    pub margin_method: String,
    #[serde(rename = "initial_margin")]
    pub initial_margin: Vec<MarginConfig>,
    #[serde(rename = "maintenance_margin")]
    pub maintenance_margin: Vec<MarginConfig>,
    pub liquidation: LiquidationConfig,
    #[serde(rename = "position_limits")]
    pub position_limits: PositionLimits,
    pub greeks: GreeksConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarginConfig {
    pub symbol: String,
    pub percentage: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LiquidationConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub threshold: f64,
    #[serde(rename = "check_frequency_seconds")]
    pub check_frequency_seconds: u64,
    #[serde(rename = "partial_liquidation")]
    pub partial_liquidation: bool,
    #[serde(rename = "insurance_fund")]
    pub insurance_fund: InsuranceFundConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InsuranceFundConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "initial_balance_usdt")]
    pub initial_balance_usdt: f64,
    #[serde(rename = "replenishment_percent")]
    pub replenishment_percent: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PositionLimits {
    #[serde(rename = "max_contracts_per_instrument")]
    pub max_contracts_per_instrument: u64,
    #[serde(rename = "max_notional_per_user_usdt")]
    pub max_notional_per_user_usdt: f64,
    #[serde(rename = "max_delta_exposure_btc")]
    pub max_delta_exposure_btc: f64,
    #[serde(rename = "max_gamma_exposure")]
    pub max_gamma_exposure: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GreeksConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "calculation_frequency_seconds")]
    pub calculation_frequency_seconds: u64,
    #[serde(rename = "risk_free_rate")]
    pub risk_free_rate: f64,
    pub volatility: VolatilityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VolatilityConfig {
    #[serde(rename = "type")]
    pub volatility_type: String,
    #[serde(rename = "historical_days")]
    #[serde(default)]
    pub historical_days: Option<u32>,
    #[serde(default)]
    pub manual_values: Option<HashMap<String, f64>>,
}

// ==================================================================================
// SETTLEMENT CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementConfig {
    pub timing: SettlementTiming,
    pub method: String,
    #[serde(rename = "mark_price")]
    pub mark_price: MarkPriceConfig,
    #[serde(default)]
    pub blockchain: Option<BlockchainConfig>,
    #[serde(default)]
    pub storage: Option<StorageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementTiming {
    #[serde(rename = "trade_settlement_delay_seconds")]
    #[serde(default = "default_trade_settlement_delay")]
    pub trade_settlement_delay_seconds: u64,
    #[serde(rename = "expiry_settlement_delay_seconds")]
    #[serde(default = "default_expiry_settlement_delay")]
    pub expiry_settlement_delay_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarkPriceConfig {
    #[serde(rename = "calculation_method")]
    pub calculation_method: String,
    #[serde(default)]
    pub twap: Option<TwapConfig>,
    #[serde(default)]
    pub index: Option<IndexConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TwapConfig {
    #[serde(rename = "window_seconds")]
    pub window_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexConfig {
    pub sources: Vec<IndexSource>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexSource {
    pub exchange: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockchainConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "primary_chain")]
    pub primary_chain: String,
    #[serde(rename = "settlement_wallet")]
    #[serde(default)]
    pub settlement_wallet: Option<SettlementWalletConfig>,
    #[serde(default)]
    pub gas: Option<GasConfig>,
    #[serde(default)]
    pub confirmations: Option<ConfirmationsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementWalletConfig {
    #[serde(rename = "type")]
    pub wallet_type: String,
    #[serde(default)]
    pub multisig: Option<MultisigConfig>,
    #[serde(rename = "key_management")]
    #[serde(default)]
    pub key_management: Option<KeyManagementConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultisigConfig {
    pub threshold: String,
    pub signers: Vec<SignerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SignerConfig {
    pub address: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeyManagementConfig {
    #[serde(rename = "type")]
    pub management_type: String,
    #[serde(default)]
    pub aws_kms: Option<AwsKmsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AwsKmsConfig {
    pub region: String,
    pub key_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GasConfig {
    pub strategy: String,
    #[serde(rename = "max_gas_price_gwei")]
    pub max_gas_price_gwei: u64,
    #[serde(rename = "gas_limit_multiplier")]
    pub gas_limit_multiplier: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfirmationsConfig {
    pub deposits: u32,
    pub withdrawals: u32,
}

// ==================================================================================
// WALLET CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletConfig {
    pub types: WalletTypesConfig,
    pub deposits: DepositsConfig,
    pub withdrawals: WithdrawalsConfig,
    pub collateral: CollateralConfig,
    #[serde(default)]
    pub storage: Option<WalletStorageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletTypesConfig {
    pub hot_wallet: HotWalletConfig,
    pub cold_wallet: ColdWalletConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HotWalletConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "max_balance_usdt")]
    pub max_balance_usdt: f64,
    #[serde(rename = "auto_sweep_threshold")]
    pub auto_sweep_threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ColdWalletConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "withdrawal_delay_hours")]
    pub withdrawal_delay_hours: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DepositsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "min_deposit_usdt")]
    pub min_deposit_usdt: f64,
    #[serde(rename = "max_deposit_usdt")]
    pub max_deposit_usdt: f64,
    #[serde(rename = "auto_credit")]
    #[serde(default = "default_enabled")]
    pub auto_credit: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WithdrawalsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "min_withdrawal_usdt")]
    pub min_withdrawal_usdt: f64,
    #[serde(rename = "max_withdrawal_usdt")]
    pub max_withdrawal_usdt: f64,
    pub limits: WithdrawalLimits,
    pub approval: WithdrawalApproval,
    pub fees: WithdrawalFees,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WithdrawalLimits {
    #[serde(rename = "daily_limit_usdt")]
    pub daily_limit_usdt: f64,
    #[serde(rename = "monthly_limit_usdt")]
    pub monthly_limit_usdt: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WithdrawalApproval {
    #[serde(rename = "auto_approve_under_usdt")]
    pub auto_approve_under_usdt: f64,
    #[serde(rename = "manual_review_over_usdt")]
    pub manual_review_over_usdt: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WithdrawalFees {
    #[serde(rename = "type")]
    pub fee_type: String,
    #[serde(rename = "flat_fee_usdt")]
    #[serde(default)]
    pub flat_fee_usdt: Option<f64>,
    #[serde(rename = "min_fee_usdt")]
    pub min_fee_usdt: f64,
    #[serde(rename = "max_fee_usdt")]
    pub max_fee_usdt: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CollateralConfig {
    pub allocation: AllocationConfig,
    pub ratios: RatiosConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AllocationConfig {
    #[serde(rename = "auto_lock")]
    pub auto_lock: bool,
    #[serde(rename = "release_on_close")]
    pub release_on_close: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RatiosConfig {
    #[serde(rename = "over_collateralization")]
    pub over_collateralization: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletStorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    #[serde(default)]
    pub postgres: Option<WalletPostgresConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WalletPostgresConfig {
    pub host: String,
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    #[serde(rename = "ssl_mode")]
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    #[serde(rename = "max_connections")]
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(rename = "isolation_level")]
    #[serde(default)]
    pub isolation_level: Option<String>,
    #[serde(rename = "audit_log_enabled")]
    #[serde(default)]
    pub audit_log_enabled: bool,
}

// ==================================================================================
// MARKET DATA SERVICE CONFIG (Module 7)
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDataServiceConfig {
    pub feeds: FeedsConfig,
    pub channels: ChannelsConfig,
    pub historical: HistoricalConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeedsConfig {
    pub orderbook: OrderbookFeedConfig,
    pub trades: TradesFeedConfig,
    pub ticker: TickerFeedConfig,
    pub greeks: GreeksFeedConfig,
    #[serde(rename = "mark_price")]
    pub mark_price: MarkPriceFeedConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderbookFeedConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "depth_levels")]
    pub depth_levels: u32,
    #[serde(rename = "update_frequency_ms")]
    pub update_frequency_ms: u64,
    #[serde(default)]
    pub aggregation: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradesFeedConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "publish_all_trades")]
    pub publish_all_trades: bool,
    #[serde(rename = "buffer_size")]
    pub buffer_size: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TickerFeedConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "update_frequency_ms")]
    pub update_frequency_ms: u64,
    #[serde(rename = "include_24h_stats")]
    pub include_24h_stats: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GreeksFeedConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "update_frequency_seconds")]
    pub update_frequency_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarkPriceFeedConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "update_frequency_seconds")]
    pub update_frequency_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelsConfig {
    pub websocket: WebSocketChannelConfig,
    pub rest_api: RestApiChannelConfig,
    pub grpc: GrpcChannelConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketChannelConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "max_connections")]
    pub max_connections: u64,
    #[serde(rename = "message_rate_limit")]
    pub message_rate_limit: u64,
    #[serde(default)]
    pub compression: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RestApiChannelConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "rate_limit_per_minute")]
    pub rate_limit_per_minute: u64,
    #[serde(rename = "cache_ttl_seconds")]
    pub cache_ttl_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrpcChannelConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoricalConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "retention_days")]
    pub retention_days: u32,
    #[serde(default)]
    pub storage: Option<HistoricalStorageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoricalStorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    #[serde(default)]
    pub postgres: Option<HistoricalPostgresConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoricalPostgresConfig {
    pub host: String,
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    #[serde(rename = "ssl_mode")]
    #[serde(default = "default_ssl_mode")]
    pub ssl_mode: String,
    #[serde(rename = "partition_by")]
    #[serde(default)]
    pub partition_by: Option<String>,
    #[serde(default)]
    pub indexes: Option<Vec<String>>,
}

// ==================================================================================
// SERVICES CONFIG (Service Discovery)
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServicesConfig {
    pub registry: RegistryConfig,
    pub communication: CommunicationConfig,
    pub auth: ServiceAuthConfig,
    #[serde(default)]
    pub routing: Option<RoutingConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryConfig {
    #[serde(rename = "type")]
    pub registry_type: String,
    #[serde(default)]
    pub static_config: Option<StaticRegistryConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticRegistryConfig {
    #[serde(rename = "instrument_service")]
    #[serde(default)]
    pub instrument_service: Option<ServiceEndpoint>,
    #[serde(rename = "oms_service")]
    #[serde(default)]
    pub oms_service: Option<ServiceEndpoint>,
    #[serde(rename = "matching_service")]
    #[serde(default)]
    pub matching_service: Option<ServiceEndpoint>,
    #[serde(rename = "risk_service")]
    #[serde(default)]
    pub risk_service: Option<ServiceEndpoint>,
    #[serde(rename = "settlement_service")]
    #[serde(default)]
    pub settlement_service: Option<ServiceEndpoint>,
    #[serde(rename = "wallet_service")]
    #[serde(default)]
    pub wallet_service: Option<ServiceEndpoint>,
    #[serde(rename = "market_data_service")]
    #[serde(default)]
    pub market_data_service: Option<ServiceEndpoint>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceEndpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommunicationConfig {
    #[serde(rename = "default_protocol")]
    pub default_protocol: String,
    #[serde(default)]
    pub grpc: Option<GrpcCommunicationConfig>,
    #[serde(default)]
    pub http: Option<HttpCommunicationConfig>,
    #[serde(default)]
    pub websocket: Option<WebSocketCommunicationConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrpcCommunicationConfig {
    #[serde(rename = "tls_enabled")]
    pub tls_enabled: bool,
    #[serde(rename = "cert_path")]
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(rename = "key_path")]
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(rename = "max_message_size_mb")]
    #[serde(default)]
    pub max_message_size_mb: Option<u32>,
    #[serde(rename = "keepalive_interval_seconds")]
    #[serde(default)]
    pub keepalive_interval_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpCommunicationConfig {
    #[serde(rename = "tls_enabled")]
    pub tls_enabled: bool,
    #[serde(rename = "cert_path")]
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(rename = "key_path")]
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(rename = "timeout_seconds")]
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketCommunicationConfig {
    #[serde(rename = "tls_enabled")]
    pub tls_enabled: bool,
    #[serde(rename = "ping_interval_seconds")]
    #[serde(default)]
    pub ping_interval_seconds: Option<u64>,
    #[serde(rename = "max_frame_size_mb")]
    #[serde(default)]
    pub max_frame_size_mb: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceAuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(rename = "bearer_token")]
    #[serde(default)]
    pub bearer_token: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoutingConfig {
    #[serde(rename = "order_submission")]
    #[serde(default)]
    pub order_submission: Option<Vec<RouteConfig>>,
    #[serde(rename = "trade_execution")]
    #[serde(default)]
    pub trade_execution: Option<Vec<RouteConfig>>,
    #[serde(rename = "market_data_broadcast")]
    #[serde(default)]
    pub market_data_broadcast: Option<Vec<RouteConfig>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteConfig {
    pub from: String,
    pub to: String,
    pub endpoint: String,
    pub protocol: String,
    #[serde(rename = "timeout_ms")]
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub async_route: Option<bool>,
    #[serde(default)]
    pub retry: Option<RetryConfig>,
    #[serde(default)]
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RetryConfig {
    #[serde(rename = "max_attempts")]
    pub max_attempts: u32,
    #[serde(rename = "backoff_ms")]
    #[serde(default)]
    pub backoff_ms: Option<u64>,
}

// ==================================================================================
// API CONFIG (User-facing APIs)
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub rest: RestApiConfig,
    pub websocket: WebSocketApiConfig,
    #[serde(default)]
    pub grpc: Option<GrpcApiConfig>,
    pub authentication: AuthenticationConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RestApiConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    #[serde(rename = "rate_limit")]
    #[serde(default)]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub cors: Option<CorsConfig>,
    #[serde(default)]
    pub versioning: Option<VersioningConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "cert_path")]
    #[serde(default)]
    pub cert_path: Option<String>,
    #[serde(rename = "key_path")]
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub authenticated: Option<RateLimitTier>,
    #[serde(default)]
    pub anonymous: Option<RateLimitTier>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitTier {
    #[serde(rename = "requests_per_minute")]
    pub requests_per_minute: u64,
    pub burst: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "allowed_origins")]
    pub allowed_origins: Vec<String>,
    #[serde(rename = "allowed_methods")]
    pub allowed_methods: Vec<String>,
    #[serde(rename = "max_age_seconds")]
    pub max_age_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersioningConfig {
    #[serde(rename = "current_version")]
    pub current_version: String,
    #[serde(rename = "supported_versions")]
    pub supported_versions: Vec<String>,
    #[serde(rename = "deprecated_versions")]
    #[serde(default)]
    pub deprecated_versions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketApiConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
    #[serde(rename = "max_connections")]
    pub max_connections: u64,
    #[serde(rename = "max_connections_per_ip")]
    pub max_connections_per_ip: u64,
    #[serde(rename = "max_message_size_kb")]
    pub max_message_size_kb: u64,
    #[serde(rename = "messages_per_second")]
    pub messages_per_second: u64,
    #[serde(rename = "ping_interval_seconds")]
    pub ping_interval_seconds: u64,
    #[serde(rename = "pong_timeout_seconds")]
    pub pong_timeout_seconds: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrpcApiConfig {
    #[serde(default)]
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthenticationConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    #[serde(default)]
    pub jwt: Option<JwtConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    #[serde(rename = "expiry_seconds")]
    pub expiry_seconds: u64,
    #[serde(rename = "refresh_enabled")]
    pub refresh_enabled: bool,
    #[serde(rename = "refresh_expiry_seconds")]
    pub refresh_expiry_seconds: u64,
}

// ==================================================================================
// FEES CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeesConfig {
    pub trading: TradingFeesConfig,
    pub settlement: SettlementFeesConfig,
    pub collection: FeeCollectionConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradingFeesConfig {
    #[serde(rename = "maker_fee_bps")]
    pub maker_fee_bps: u32,
    #[serde(rename = "taker_fee_bps")]
    pub taker_fee_bps: u32,
    #[serde(rename = "volume_tiers")]
    #[serde(default)]
    pub volume_tiers: Option<VolumeTiersConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VolumeTiersConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tiers: Option<Vec<VolumeTier>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VolumeTier {
    #[serde(rename = "volume_30d_usdt")]
    pub volume_30d_usdt: f64,
    #[serde(rename = "maker_fee_bps")]
    pub maker_fee_bps: u32,
    #[serde(rename = "taker_fee_bps")]
    pub taker_fee_bps: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SettlementFeesConfig {
    #[serde(rename = "type")]
    pub fee_type: String,
    #[serde(rename = "flat_fee_usdt")]
    #[serde(default)]
    pub flat_fee_usdt: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeeCollectionConfig {
    #[serde(rename = "wallet_address")]
    pub wallet_address: String,
    #[serde(rename = "auto_collect")]
    pub auto_collect: bool,
    #[serde(rename = "collection_frequency_hours")]
    pub collection_frequency_hours: u32,
}

// ==================================================================================
// VIRTUAL TRADING CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualTradingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "port_offset")]
    #[serde(default)]
    pub port_offset: u16,
    #[serde(default)]
    pub settlement: Option<VirtualSettlementConfig>,
    #[serde(default)]
    pub storage: Option<VirtualStorageConfig>,
    #[serde(rename = "virtual_users")]
    #[serde(default)]
    pub virtual_users: Option<VirtualUsersConfig>,
    #[serde(rename = "market_data")]
    #[serde(default)]
    pub market_data: Option<VirtualMarketDataConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualSettlementConfig {
    #[serde(rename = "blockchain_enabled")]
    pub blockchain_enabled: bool,
    #[serde(rename = "real_money")]
    pub real_money: bool,
    #[serde(rename = "mock_settlement_delay_ms")]
    pub mock_settlement_delay_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualStorageConfig {
    #[serde(rename = "instrument_db")]
    #[serde(default)]
    pub instrument_db: Option<VirtualDbConfig>,
    #[serde(rename = "oms_db")]
    #[serde(default)]
    pub oms_db: Option<VirtualDbConfig>,
    #[serde(rename = "orderbook_store")]
    #[serde(default)]
    pub orderbook_store: Option<VirtualRedisConfig>,
    #[serde(rename = "wallet_db")]
    #[serde(default)]
    pub wallet_db: Option<VirtualDbConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualDbConfig {
    pub database: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualRedisConfig {
    #[serde(rename = "db_index")]
    pub db_index: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualUsersConfig {
    #[serde(rename = "initial_balance_usdt")]
    pub initial_balance_usdt: f64,
    #[serde(rename = "reset_balance_daily")]
    pub reset_balance_daily: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VirtualMarketDataConfig {
    pub mode: String,
    #[serde(default)]
    pub replay: Option<ReplayConfig>,
    #[serde(default)]
    pub synthetic: Option<SyntheticConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplayConfig {
    #[serde(rename = "start_timestamp")]
    pub start_timestamp: String,
    #[serde(rename = "speed_multiplier")]
    pub speed_multiplier: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyntheticConfig {
    pub volatility: f64,
    pub drift: f64,
}

// ==================================================================================
// COMPLIANCE CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComplianceConfig {
    pub kyc: KycConfig,
    #[serde(rename = "geo_restrictions")]
    pub geo_restrictions: GeoRestrictionsConfig,
    pub audit: AuditConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KycConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "required_for_trading")]
    pub required_for_trading: bool,
    pub provider: String,
    #[serde(default)]
    pub sumsub: Option<SumsubConfig>,
    #[serde(default)]
    pub levels: Option<KycLevelsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SumsubConfig {
    #[serde(rename = "app_token")]
    pub app_token: String,
    #[serde(rename = "secret_key")]
    pub secret_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KycLevelsConfig {
    pub tier1: KycTier,
    pub tier2: KycTier,
    pub tier3: KycTier,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KycTier {
    #[serde(rename = "max_daily_volume_usdt")]
    pub max_daily_volume_usdt: Option<f64>,
    #[serde(rename = "max_withdrawal_usdt")]
    pub max_withdrawal_usdt: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoRestrictionsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "blocked_countries")]
    #[serde(default)]
    pub blocked_countries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(rename = "log_all_trades")]
    pub log_all_trades: bool,
    #[serde(rename = "log_all_withdrawals")]
    pub log_all_withdrawals: bool,
    #[serde(rename = "log_all_admin_actions")]
    pub log_all_admin_actions: bool,
    #[serde(default)]
    pub storage: Option<AuditStorageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuditStorageConfig {
    #[serde(rename = "type")]
    pub storage_type: String,
    #[serde(rename = "retention_years")]
    pub retention_years: u32,
}

// ==================================================================================
// MONITORING CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitoringConfig {
    pub metrics: MetricsConfig,
    pub logging: LoggingConfig,
    pub tracing: TracingConfig,
    pub alerting: AlertingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub prometheus: Option<PrometheusConfig>,
    #[serde(default)]
    pub track: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrometheusConfig {
    pub port: u16,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub outputs: LogOutputsConfig,
    #[serde(default)]
    pub file: Option<FileLogConfig>,
    #[serde(default)]
    pub centralized: Option<CentralizedLogConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogOutputsConfig {
    pub console: bool,
    pub file: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileLogConfig {
    pub path: String,
    #[serde(rename = "max_size_mb")]
    pub max_size_mb: u64,
    #[serde(rename = "max_backups")]
    pub max_backups: u32,
    #[serde(rename = "max_age_days")]
    pub max_age_days: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CentralizedLogConfig {
    #[serde(default)]
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub elasticsearch: Option<ElasticsearchConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ElasticsearchConfig {
    pub url: String,
    #[serde(rename = "index_prefix")]
    pub index_prefix: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub jaeger: Option<JaegerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JaegerConfig {
    pub endpoint: String,
    #[serde(rename = "sample_rate")]
    pub sample_rate: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlertingConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub pagerduty: Option<PagerdutyConfig>,
    #[serde(default)]
    pub rules: Option<Vec<AlertRule>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PagerdutyConfig {
    #[serde(rename = "integration_key")]
    pub integration_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AlertRule {
    pub name: String,
    pub condition: String,
    pub severity: String,
}

// ==================================================================================
// SECURITY CONFIG
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub encryption: EncryptionConfig,
    pub secrets: SecretsConfig,
    pub ddos: DdosConfig,
    pub firewall: FirewallConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EncryptionConfig {
    #[serde(rename = "at_rest")]
    pub at_rest: AtRestEncryption,
    #[serde(rename = "in_transit")]
    pub in_transit: InTransitEncryption,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AtRestEncryption {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub algorithm: String,
    #[serde(rename = "key_rotation_days")]
    pub key_rotation_days: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InTransitEncryption {
    #[serde(rename = "tls_version")]
    pub tls_version: String,
    #[serde(rename = "min_tls_version")]
    pub min_tls_version: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecretsConfig {
    pub provider: String,
    #[serde(rename = "aws_secrets_manager")]
    #[serde(default)]
    pub aws_secrets_manager: Option<AwsSecretsManagerConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AwsSecretsManagerConfig {
    pub region: String,
    #[serde(rename = "secret_prefix")]
    pub secret_prefix: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DdosConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub cloudflare: bool,
    #[serde(rename = "rate_limiting")]
    #[serde(default = "default_enabled")]
    pub rate_limiting: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FirewallConfig {
    #[serde(rename = "whitelist_ips")]
    #[serde(default)]
    pub whitelist_ips: Vec<String>,
    #[serde(rename = "blacklist_ips")]
    #[serde(default)]
    pub blacklist_ips: Vec<String>,
}

// ==================================================================================
// FEATURES CONFIG (Feature Flags)
// ==================================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeaturesConfig {
    #[serde(rename = "advanced_order_types")]
    #[serde(default)]
    pub advanced_order_types: bool,
    #[serde(rename = "portfolio_margin")]
    #[serde(default)]
    pub portfolio_margin: bool,
    #[serde(rename = "options_strategies")]
    #[serde(default)]
    pub options_strategies: bool,
    #[serde(rename = "social_trading")]
    #[serde(default)]
    pub social_trading: bool,
    #[serde(rename = "api_trading_bots")]
    #[serde(default = "default_enabled")]
    pub api_trading_bots: bool,
    #[serde(rename = "mobile_app")]
    #[serde(default)]
    pub mobile_app: bool,
    #[serde(rename = "institutional_api")]
    #[serde(default)]
    pub institutional_api: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MasterConfig {
    pub exchange: ExchangeConfig,
    pub instrument: InstrumentConfig,
    #[serde(default)]
    pub deployment: DeploymentConfig,
    #[serde(default)]
    pub oms: Option<OmsConfig>,
    #[serde(rename = "matching_engine")]
    #[serde(default)]
    pub matching_engine: Option<MatchingEngineConfig>,
    #[serde(rename = "risk_engine")]
    #[serde(default)]
    pub risk_engine: Option<RiskEngineConfig>,
    #[serde(default)]
    pub settlement: Option<SettlementConfig>,
    #[serde(default)]
    pub wallet: Option<WalletConfig>,
    #[serde(rename = "market_data")]
    #[serde(default)]
    pub market_data: Option<MarketDataServiceConfig>,
    #[serde(default)]
    pub services: Option<ServicesConfig>,
    #[serde(default)]
    pub api: Option<ApiConfig>,
    #[serde(default)]
    pub fees: Option<FeesConfig>,
    #[serde(rename = "virtual_trading")]
    #[serde(default)]
    pub virtual_trading: Option<VirtualTradingConfig>,
    #[serde(default)]
    pub compliance: Option<ComplianceConfig>,
    #[serde(default)]
    pub monitoring: Option<MonitoringConfig>,
    #[serde(default)]
    pub security: Option<SecurityConfig>,
    #[serde(default)]
    pub features: Option<FeaturesConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DeploymentConfig {
    #[serde(default)]
    pub gateway: ServiceDeployment,
    #[serde(default)]
    pub instrument: ServiceDeployment,
    #[serde(default)]
    pub oms: ServiceDeployment,
    #[serde(default)]
    pub matching: ServiceDeployment,
    #[serde(default)]
    pub wallet: ServiceDeployment,
    #[serde(default)]
    pub settlement: ServiceDeployment,
    #[serde(default)]
    pub risk: ServiceDeployment,
}

impl DeploymentConfig {
    /// Resolve the actual host address by checking environment variables
    pub fn resolve_host(&self, service_name: &str) -> String {
        let service = match service_name {
            "gateway" => &self.gateway,
            "instrument" => &self.instrument,
            "oms" => &self.oms,
            "matching" => &self.matching,
            "wallet" => &self.wallet,
            "settlement" => &self.settlement,
            "risk" => &self.risk,
            _ => return "0.0.0.0".to_string(),
        };
        
        service.resolve_host()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ServiceDeployment {
    /// Host address - can be env var placeholder like "[GATEWAY_IP_ADDRESS]"
    #[serde(default = "default_service_host")]
    pub host: String,
    /// Fallback address if env var is not set
    #[serde(default = "default_host")]
    pub fallback: String,
    /// HTTP port
    #[serde(default)]
    pub http: u16,
    /// gRPC port
    #[serde(default)]
    pub grpc: u16,
    /// WebSocket port
    #[serde(default)]
    pub ws: u16,
}

impl ServiceDeployment {
    /// Resolve the host by extracting env var name from brackets and looking it up
    pub fn resolve_host(&self) -> String {
        // Check if host is in format [ENV_VAR_NAME]
        if self.host.starts_with('[') && self.host.ends_with(']') {
            let env_var = &self.host[1..self.host.len()-1];
            // Try to get from environment
            match std::env::var(env_var) {
                Ok(value) => value,
                Err(_) => self.fallback.clone(),
            }
        } else {
            // Host is a direct value, return as-is
            self.host.clone()
        }
    }
    
    /// Get the full HTTP address (host:http_port)
    pub fn http_address(&self) -> String {
        format!("{}:{}", self.resolve_host(), self.http)
    }
    
    /// Get the full gRPC address (host:grpc_port)
    pub fn grpc_address(&self) -> String {
        format!("{}:{}", self.resolve_host(), self.grpc)
    }
    
    /// Get the full WebSocket address (host:ws_port)
    pub fn ws_address(&self) -> String {
        format!("{}:{}", self.resolve_host(), self.ws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_master_config() {
        // This test verifies that the full master_config.yaml can be parsed
        let yaml = include_str!("../../../master_config/master_config.yaml");
        
        // Parse the YAML
        let config: Result<MasterConfig, _> = serde_yaml::from_str(yaml);
        
        match config {
            Ok(cfg) => {
                // Verify exchange section
                assert_eq!(cfg.exchange.name, "Acme Options Exchange");
                assert_eq!(cfg.exchange.version, "1.0.0");
                
                // Verify instrument section
                assert_eq!(cfg.instrument.supported_assets.len(), 3);
                assert_eq!(cfg.instrument.supported_assets[0].symbol, "BTC");
                assert_eq!(cfg.instrument.supported_assets[1].symbol, "ETH");
                assert_eq!(cfg.instrument.supported_assets[2].symbol, "SOL");
                
                // Verify settlement currencies with chains
                assert_eq!(cfg.instrument.settlement_currencies.len(), 2);
                assert_eq!(cfg.instrument.settlement_currencies[0].symbol, "USDT");
                assert!(cfg.instrument.settlement_currencies[0].chains.len() > 0);
                
                // Verify market_data in instrument section
                assert!(cfg.instrument.market_data.is_some());
                let md = cfg.instrument.market_data.as_ref().unwrap();
                assert!(md.providers.len() > 0);
                
                // Verify expiry_schedule
                assert!(cfg.instrument.expiry_schedule.is_some());
                let expiry = cfg.instrument.expiry_schedule.as_ref().unwrap();
                assert!(expiry.daily.enabled);
                assert_eq!(expiry.daily.count, Some(7));
                
                // Verify OMS
                assert!(cfg.oms.is_some());
                let oms = cfg.oms.as_ref().unwrap();
                assert!(oms.order_types.limit.enabled);
                assert!(oms.order_types.market.enabled);
                
                // Verify Matching Engine
                assert!(cfg.matching_engine.is_some());
                let me = cfg.matching_engine.as_ref().unwrap();
                assert_eq!(me.algorithm, "price_time_priority");
                
                // Verify Risk Engine
                assert!(cfg.risk_engine.is_some());
                let risk = cfg.risk_engine.as_ref().unwrap();
                assert_eq!(risk.margin_method, "simplified_span");
                
                // Verify Settlement
                assert!(cfg.settlement.is_some());
                let settlement = cfg.settlement.as_ref().unwrap();
                assert_eq!(settlement.method, "cash_settled");
                
                // Verify Wallet
                assert!(cfg.wallet.is_some());
                
                // Verify Market Data Service
                assert!(cfg.market_data.is_some());
                
                // Verify Services
                assert!(cfg.services.is_some());
                
                // Verify API
                assert!(cfg.api.is_some());
                let api = cfg.api.as_ref().unwrap();
                assert!(api.rest.enabled);
                assert_eq!(api.rest.port, 8080);
                
                // Verify Fees
                assert!(cfg.fees.is_some());
                let fees = cfg.fees.as_ref().unwrap();
                assert_eq!(fees.trading.maker_fee_bps, 10);
                assert_eq!(fees.trading.taker_fee_bps, 20);
                
                // Verify Virtual Trading
                assert!(cfg.virtual_trading.is_some());
                
                // Verify Compliance
                assert!(cfg.compliance.is_some());
                
                // Verify Monitoring
                assert!(cfg.monitoring.is_some());
                
                // Verify Security
                assert!(cfg.security.is_some());
                
                // Verify Features
                assert!(cfg.features.is_some());
                let features = cfg.features.as_ref().unwrap();
                assert!(features.api_trading_bots);
                
                // Verify Deployment
                assert_eq!(cfg.deployment.gateway.http, 8080);
                assert_eq!(cfg.deployment.instrument.http, 8081);
                
                println!("Successfully parsed full master_config.yaml!");
            }
            Err(e) => {
                panic!("Failed to parse master_config.yaml: {}", e);
            }
        }
    }

    #[test]
    fn test_generate_default_config() {
        let config = parser::generate_default_config();
        
        assert_eq!(config.exchange.name, "My Exchange");
        assert_eq!(config.instrument.supported_assets.len(), 2);
        assert_eq!(config.instrument.settlement_currencies.len(), 1);
        
        // All optional sections should be None
        assert!(config.oms.is_none());
        assert!(config.matching_engine.is_none());
        assert!(config.risk_engine.is_none());
        assert!(config.settlement.is_none());
        assert!(config.wallet.is_none());
        assert!(config.market_data.is_none());
    }

    #[test]
    fn test_chain_config_parsing() {
        let yaml = r#"
exchange:
  name: "Test"
  description: "Test exchange"
  version: "1.0.0"
  mode: "virtual"
  trading_hours:
    type: "24/7"
instrument:
  supported_assets:
    - symbol: "BTC"
      name: "Bitcoin"
      decimals: 8
      contract_size: 0.01
      min_order_size: 1
      tick_size: 0.5
      price_decimals: 2
      enabled: true
  settlement_currencies:
    - symbol: "USDT"
      name: "Tether"
      decimals: 6
      enabled: true
      primary: true
      chains:
        - chain: "ethereum"
          chain_id: 1
          contract_address: "0xdac17f958d2ee523a2206206994597c13d831ec7"
          rpc_url: "https://eth.example.com"
          gas_limit: 100000
          enabled: true
        - chain: "polygon"
          chain_id: 137
          contract_address: "0xc2132d05d31c914a87c6611c10748aeb04b58e8f"
          rpc_url: "https://polygon.example.com"
          gas_limit: 80000
          enabled: true
"#;
        
        let config: MasterConfig = serde_yaml::from_str(yaml).expect("Failed to parse YAML");
        
        assert_eq!(config.instrument.settlement_currencies.len(), 1);
        let usdt = &config.instrument.settlement_currencies[0];
        assert_eq!(usdt.chains.len(), 2);
        assert_eq!(usdt.chains[0].chain, "ethereum");
        assert_eq!(usdt.chains[0].chain_id, 1);
        assert_eq!(usdt.chains[1].chain, "polygon");
        assert_eq!(usdt.chains[1].chain_id, 137);
    }
}