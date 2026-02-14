use crate::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, info, instrument};

#[instrument(skip(path))]
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<MasterConfig> {
    let path = path.as_ref();
    info!("Loading configuration from: {:?}", path);

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    debug!("Config file content length: {} bytes", content.len());

    // Perform environment variable substitution
    let substituted = substitution::substitute_env_vars(&content)?;
    debug!("Environment variable substitution completed");

    // Parse YAML
    let config: MasterConfig = serde_yaml::from_str(&substituted)
        .with_context(|| "Failed to parse YAML configuration")?;

    info!("Configuration loaded successfully");
    Ok(config)
}

#[instrument]
pub fn generate_default_config() -> MasterConfig {
    use defaults::*;
    use std::collections::HashMap;

    MasterConfig {
        exchange: ExchangeConfig {
            name: "My Exchange".to_string(),
            description: "A white-label crypto options exchange".to_string(),
            version: "1.0.0".to_string(),
            mode: ExchangeMode::Virtual,
            trading_hours: default_trading_hours(),
        },
        instrument: InstrumentConfig {
            supported_assets: vec![
                Asset {
                    symbol: "BTC".to_string(),
                    name: "Bitcoin".to_string(),
                    decimals: 8,
                    contract_size: 0.01,
                    min_order_size: 1,
                    tick_size: 0.5,
                    price_decimals: 2,
                    enabled: true,
                },
                Asset {
                    symbol: "ETH".to_string(),
                    name: "Ethereum".to_string(),
                    decimals: 18,
                    contract_size: 0.1,
                    min_order_size: 1,
                    tick_size: 0.1,
                    price_decimals: 2,
                    enabled: true,
                },
            ],
            settlement_currencies: vec![
                SettlementCurrency {
                    symbol: "USDT".to_string(),
                    name: "Tether".to_string(),
                    decimals: 6,
                    enabled: true,
                    primary: true,
                },
            ],
        },
        market_data: MarketDataConfig {
            providers: vec![
                MarketDataProvider {
                    name: "binance".to_string(),
                    provider_type: "websocket".to_string(),
                    enabled: true,
                    primary: true,
                    streams: vec![
                        MarketDataStream {
                            name: "BTC".to_string(),
                            endpoint: "wss://stream.binance.com:9443/ws/btcusdt@ticker".to_string(),
                            symbol: "BTC".to_string(),
                            ticker: "BTCUSDT".to_string(),
                        },
                        MarketDataStream {
                            name: "ETH".to_string(),
                            endpoint: "wss://stream.binance.com:9443/ws/ethusdt@ticker".to_string(),
                            symbol: "ETH".to_string(),
                            ticker: "ETHUSDT".to_string(),
                        },
                    ],
                    reconnect_delay_seconds: Some(default_reconnect_delay_seconds()),
                    max_reconnect_attempts: Some(default_max_reconnect_attempts()),
                    heartbeat_interval_seconds: Some(default_heartbeat_interval_seconds()),
                    connection_timeout_seconds: Some(default_connection_timeout_seconds()),
                    timeout_seconds: None,
                    rate_limit_per_second: None,
                    auth: None,
                },
            ],
            fallback_strategy: default_fallback_strategy(),
            max_price_age_seconds: default_max_price_age_seconds(),
            stale_price_action: default_stale_price_action(),
        },
        expiry: ExpiryConfig {
            daily: ExpirySchedule {
                enabled: true,
                count: 7,
                expiry_time_utc: "08:00".to_string(),
            },
            weekly: ExpirySchedule {
                enabled: true,
                count: 4,
                expiry_time_utc: "08:00".to_string(),
            },
            monthly: ExpirySchedule {
                enabled: true,
                count: 6,
                expiry_time_utc: "08:00".to_string(),
            },
            quarterly: ExpirySchedule {
                enabled: true,
                count: 4,
                expiry_time_utc: "08:00".to_string(),
            },
            yearly: ExpirySchedule {
                enabled: false,
                count: 2,
                expiry_time_utc: "08:00".to_string(),
            },
        },
        storage: StorageConfig {
            storage_type: "postgres".to_string(),
            postgres: Some(PostgresConfig {
                host: "${INSTRUMENT_DB_HOST}".to_string(),
                port: default_postgres_port(),
                database: "instruments".to_string(),
                user: "${INSTRUMENT_DB_USER}".to_string(),
                password: "${INSTRUMENT_DB_PASSWORD}".to_string(),
                ssl_mode: default_ssl_mode(),
                max_connections: default_max_connections(),
                connection_timeout_seconds: default_connection_timeout(),
                idle_timeout_seconds: default_idle_timeout(),
            }),
            supabase: None,
            cache: Some(CacheConfig {
                enabled: true,
                ttl_seconds: default_ttl_seconds(),
                max_entries: default_max_entries(),
            }),
        },
        oms: OmsConfig {
            order_types: OrderTypesConfig {
                limit: OrderTypeEnabled { enabled: true },
                market: OrderTypeEnabled { enabled: true },
                stop_limit: OrderTypeEnabled { enabled: false },
                stop_market: OrderTypeEnabled { enabled: false },
            },
            time_in_force: TimeInForceConfig {
                gtc: TifEnabled { enabled: true },
                ioc: TifEnabled { enabled: true },
                fok: TifEnabled { enabled: true },
                day: TifEnabled { enabled: false },
            },
            limits: OmsLimits {
                max_open_orders_per_user: default_max_open_orders_per_user(),
                max_order_size_contracts: default_max_order_size_contracts(),
                min_order_size_contracts: default_min_order_size_contracts(),
                max_price_deviation_percent: default_max_price_deviation_percent(),
            },
            orderbook: OrderbookConfig {
                depth_levels: default_depth_levels(),
                update_frequency_ms: default_update_frequency_ms(),
            },
            storage: StorageConfig {
                storage_type: "postgres".to_string(),
                postgres: Some(PostgresConfig {
                    host: "${OMS_DB_HOST}".to_string(),
                    port: default_postgres_port(),
                    database: "orders".to_string(),
                    user: "${OMS_DB_USER}".to_string(),
                    password: "${OMS_DB_PASSWORD}".to_string(),
                    ssl_mode: default_ssl_mode(),
                    max_connections: 50,
                    connection_timeout_seconds: default_connection_timeout(),
                    idle_timeout_seconds: default_idle_timeout(),
                }),
                supabase: None,
                cache: None,
            },
        },
        matching_engine: MatchingEngineConfig {
            algorithm: "price_time_priority".to_string(),
            performance: PerformanceConfig {
                matching_frequency_ms: default_matching_frequency_ms(),
                batch_size: default_batch_size(),
            },
            orderbook_store: OrderbookStoreConfig {
                store_type: "inmemory".to_string(),
                redis: None,
            },
            execution: ExecutionConfig {
                atomic_trades: default_atomic_trades(),
                max_partial_fills: default_max_partial_fills(),
            },
            circuit_breakers: CircuitBreakersConfig {
                enabled: true,
                price_movement: PriceMovementConfig {
                    enabled: true,
                    percent_threshold: default_percent_threshold(),
                    time_window_seconds: default_time_window_seconds(),
                    halt_duration_seconds: default_halt_duration_seconds(),
                },
                liquidity: LiquidityConfig {
                    enabled: true,
                    min_bid_ask_orders: default_min_bid_ask_orders(),
                    max_spread_percent: default_max_spread_percent(),
                    halt_duration_seconds: default_liquidity_halt_duration(),
                },
            },
        },
        risk_engine: RiskEngineConfig {
            margin_method: "simplified_span".to_string(),
            initial_margin: vec![
                MarginConfig {
                    symbol: "BTC".to_string(),
                    percentage: 0.20,
                },
                MarginConfig {
                    symbol: "ETH".to_string(),
                    percentage: 0.25,
                },
            ],
            maintenance_margin: vec![
                MarginConfig {
                    symbol: "BTC".to_string(),
                    percentage: 0.15,
                },
                MarginConfig {
                    symbol: "ETH".to_string(),
                    percentage: 0.20,
                },
            ],
            liquidation: LiquidationConfig {
                enabled: true,
                threshold: 0.80,
                check_frequency_seconds: default_check_frequency_seconds(),
                partial_liquidation: true,
                insurance_fund: InsuranceFundConfig {
                    enabled: true,
                    initial_balance_usdt: default_insurance_initial_balance(),
                    replenishment_percent: default_replenishment_percent(),
                },
            },
            position_limits: PositionLimits {
                max_contracts_per_instrument: 1000,
                max_notional_per_user_usdt: 1000000.0,
                max_delta_exposure_btc: 10.0,
                max_gamma_exposure: 100.0,
            },
            greeks: GreeksConfig {
                enabled: true,
                calculation_frequency_seconds: default_calculation_frequency_seconds(),
                risk_free_rate: default_risk_free_rate(),
                volatility: VolatilityConfig {
                    volatility_type: default_volatility_type(),
                    historical_days: Some(default_historical_days()),
                    manual_values: Some({
                        let mut map = HashMap::new();
                        map.insert("BTC".to_string(), 0.80);
                        map.insert("ETH".to_string(), 0.90);
                        map
                    }),
                },
            },
            storage: StorageConfig {
                storage_type: "postgres".to_string(),
                postgres: Some(PostgresConfig {
                    host: "${RISK_DB_HOST}".to_string(),
                    port: default_postgres_port(),
                    database: "risk".to_string(),
                    user: "${RISK_DB_USER}".to_string(),
                    password: "${RISK_DB_PASSWORD}".to_string(),
                    ssl_mode: default_ssl_mode(),
                    max_connections: 30,
                    connection_timeout_seconds: default_connection_timeout(),
                    idle_timeout_seconds: default_idle_timeout(),
                }),
                supabase: None,
                cache: None,
            },
        },
    }
}

#[instrument]
pub fn save_config<P: AsRef<Path> + std::fmt::Debug>(config: &MasterConfig, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Saving configuration to: {:?}", path);

    let yaml = serde_yaml::to_string(config)
        .with_context(|| "Failed to serialize configuration to YAML")?;

    fs::write(path, yaml)
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    info!("Configuration saved successfully");
    Ok(())
}