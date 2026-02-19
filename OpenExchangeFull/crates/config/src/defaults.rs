pub fn default_enabled() -> bool {
    true
}

pub fn default_trading_hours() -> super::TradingHours {
    super::TradingHours {
        hours_type: "24/7".to_string(),
    }
}

pub fn default_postgres_port() -> u16 {
    5432
}

pub fn default_ssl_mode() -> String {
    "require".to_string()
}

pub fn default_max_connections() -> u32 {
    20
}

pub fn default_connection_timeout() -> u64 {
    30
}

pub fn default_idle_timeout() -> u64 {
    600
}

pub fn default_ttl_seconds() -> u64 {
    300
}

pub fn default_max_entries() -> u64 {
    10000
}

pub fn default_redis_port() -> u16 {
    6379
}

pub fn default_persistence_enabled() -> bool {
    true
}

pub fn default_snapshot_interval() -> u64 {
    60
}

pub fn default_atomic_trades() -> bool {
    true
}

pub fn default_max_partial_fills() -> u64 {
    10
}

pub fn default_matching_frequency_ms() -> u64 {
    10
}

pub fn default_batch_size() -> u64 {
    100
}

pub fn default_depth_levels() -> u32 {
    50
}

pub fn default_update_frequency_ms() -> u64 {
    100
}

pub fn default_max_price_age_seconds() -> u64 {
    10
}

pub fn default_stale_price_action() -> String {
    "halt_trading".to_string()
}

pub fn default_fallback_strategy() -> String {
    "median".to_string()
}

pub fn default_reconnect_delay_seconds() -> u64 {
    5
}

pub fn default_max_reconnect_attempts() -> u64 {
    10
}

pub fn default_heartbeat_interval_seconds() -> u64 {
    30
}

pub fn default_connection_timeout_seconds() -> u64 {
    60
}

pub fn default_timeout_seconds() -> u64 {
    5
}

pub fn default_rate_limit_per_second() -> u64 {
    10
}

pub fn default_max_open_orders_per_user() -> u64 {
    100
}

pub fn default_max_order_size_contracts() -> u64 {
    10000
}

pub fn default_min_order_size_contracts() -> u64 {
    1
}

pub fn default_max_price_deviation_percent() -> f64 {
    20.0
}

pub fn default_percent_threshold() -> f64 {
    10.0
}

pub fn default_time_window_seconds() -> u64 {
    60
}

pub fn default_halt_duration_seconds() -> u64 {
    300
}

pub fn default_min_bid_ask_orders() -> u64 {
    5
}

pub fn default_max_spread_percent() -> f64 {
    5.0
}

pub fn default_liquidity_halt_duration() -> u64 {
    60
}

pub fn default_check_frequency_seconds() -> u64 {
    5
}

pub fn default_insurance_initial_balance() -> f64 {
    100000.0
}

pub fn default_replenishment_percent() -> f64 {
    0.10
}

pub fn default_calculation_frequency_seconds() -> u64 {
    10
}

pub fn default_risk_free_rate() -> f64 {
    0.05
}

pub fn default_volatility_type() -> String {
    "implied".to_string()
}

pub fn default_historical_days() -> u32 {
    30
}

// Deployment defaults
pub fn default_host() -> String {
    "0.0.0.0".to_string()
}

pub fn default_service_host() -> String {
    "[SERVICE_IP_ADDRESS]".to_string()
}