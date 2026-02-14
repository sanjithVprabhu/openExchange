use serde::{de::{self, Deserializer, MapAccess, Visitor}, Deserialize, Serialize};
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
    pub enabled: bool,
    #[serde(default)]
    pub primary: bool,
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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketDataStream {
    pub name: String,
    pub endpoint: String,
    pub symbol: String,
    pub ticker: String,
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
    pub count: u32,
    #[serde(rename = "expiry_time_utc")]
    pub expiry_time_utc: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MasterConfig {
    pub exchange: ExchangeConfig,
    pub instrument: InstrumentConfig,
    #[serde(rename = "market_data")]
    pub market_data: MarketDataConfig,
    pub expiry: ExpiryConfig,
    pub storage: StorageConfig,
    pub oms: OmsConfig,
    #[serde(rename = "matching_engine")]
    pub matching_engine: MatchingEngineConfig,
    #[serde(rename = "risk_engine")]
    pub risk_engine: RiskEngineConfig,
}