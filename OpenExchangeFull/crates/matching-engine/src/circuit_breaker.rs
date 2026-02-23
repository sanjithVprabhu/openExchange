//! Circuit Breakers for the Matching Engine
//!
//! Circuit breakers halt trading when market conditions become unsafe:
//! - Price Movement: Detect large price swings
//! - Liquidity: Detect thin order books

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Circuit breaker state for a single instrument
#[derive(Debug, Clone)]
pub struct InstrumentCircuitBreaker {
    pub instrument_id: String,
    pub price_movement_triggered: bool,
    pub liquidity_triggered: bool,
    pub halted_until: Option<Instant>,
    pub last_trade_price: Option<f64>,
    pub price_history: Vec<(Instant, f64)>,
}

impl InstrumentCircuitBreaker {
    pub fn new(instrument_id: String) -> Self {
        Self {
            instrument_id,
            price_movement_triggered: false,
            liquidity_triggered: false,
            halted_until: None,
            last_trade_price: None,
            price_history: Vec::new(),
        }
    }

    pub fn is_halted(&self) -> bool {
        if let Some(until) = self.halted_until {
            if Instant::now() < until {
                return true;
            }
        }
        false
    }

    pub fn clear_halt(&mut self) {
        self.halted_until = None;
        self.price_movement_triggered = false;
        self.liquidity_triggered = false;
    }

    pub fn record_trade(&mut self, price: f64) {
        let now = Instant::now();
        
        // Update last trade price
        self.last_trade_price = Some(price);
        
        // Add to price history
        self.price_history.push((now, price));
        
        // Keep only recent prices (last 60 seconds)
        let cutoff = now - Duration::from_secs(60);
        self.price_history.retain(|(time, _)| *time > cutoff);
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub price_movement_enabled: bool,
    pub price_movement_threshold_percent: f64,
    pub price_movement_window_seconds: u64,
    pub price_movement_halt_duration_seconds: u64,
    pub liquidity_enabled: bool,
    pub min_bid_ask_orders: u64,
    pub max_spread_percent: f64,
    pub liquidity_halt_duration_seconds: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            price_movement_enabled: false,
            price_movement_threshold_percent: 10.0,
            price_movement_window_seconds: 60,
            price_movement_halt_duration_seconds: 300,
            liquidity_enabled: false,
            min_bid_ask_orders: 5,
            max_spread_percent: 5.0,
            liquidity_halt_duration_seconds: 60,
        }
    }
}

impl From<&config::CircuitBreakersConfig> for CircuitBreakerConfig {
    fn from(config: &config::CircuitBreakersConfig) -> Self {
        Self {
            enabled: config.enabled,
            price_movement_enabled: config.price_movement.enabled,
            price_movement_threshold_percent: config.price_movement.percent_threshold,
            price_movement_window_seconds: config.price_movement.time_window_seconds,
            price_movement_halt_duration_seconds: config.price_movement.halt_duration_seconds,
            liquidity_enabled: config.liquidity.enabled,
            min_bid_ask_orders: config.liquidity.min_bid_ask_orders as u64,
            max_spread_percent: config.liquidity.max_spread_percent,
            liquidity_halt_duration_seconds: config.liquidity.halt_duration_seconds,
        }
    }
}

/// Circuit breaker manager for all instruments
#[derive(Debug)]
pub struct CircuitBreakerManager {
    config: CircuitBreakerConfig,
    breakers: HashMap<String, InstrumentCircuitBreaker>,
}

impl CircuitBreakerManager {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            breakers: HashMap::new(),
        }
    }

    pub fn get_or_create(&mut self, instrument_id: &str) -> &mut InstrumentCircuitBreaker {
        self.breakers
            .entry(instrument_id.to_string())
            .or_insert_with(|| InstrumentCircuitBreaker::new(instrument_id.to_string()))
    }

    /// Check if trading is halted for an instrument
    pub fn is_halted(&self, instrument_id: &str) -> bool {
        self.breakers
            .get(instrument_id)
            .map(|b| b.is_halted())
            .unwrap_or(false)
    }

    /// Check price movement circuit breaker
    /// Returns true if halt should be triggered
    pub fn check_price_movement(
        &mut self,
        instrument_id: &str,
        new_price: f64,
    ) -> bool {
        if !self.config.enabled || !self.config.price_movement_enabled {
            return false;
        }

        // Extract config values first to avoid borrow issues
        let threshold_percent = self.config.price_movement_threshold_percent;
        let halt_duration = self.config.price_movement_halt_duration_seconds;
        
        let breaker = self.get_or_create(instrument_id);
        
        // Record the new trade
        breaker.record_trade(new_price);

        // Need at least 2 prices to compare
        if breaker.price_history.len() < 2 {
            return false;
        }

        // Get oldest and newest prices in window
        let oldest = breaker.price_history.first().map(|(_, p)| *p);
        let current = breaker.last_trade_price;

        match (oldest, current) {
            (Some(old_price), Some(curr)) if old_price > 0.0 => {
                let percent_change = ((curr - old_price) / old_price).abs() * 100.0;
                
                if percent_change > threshold_percent {
                    warn!(
                        instrument = instrument_id,
                        old_price,
                        current_price = curr,
                        percent_change,
                        threshold = threshold_percent,
                        "Price movement circuit breaker triggered"
                    );
                    
                    // Trigger halt
                    breaker.price_movement_triggered = true;
                    breaker.halted_until = Some(
                        Instant::now() + Duration::from_secs(halt_duration)
                    );
                    
                    return true;
                }
            }
            _ => {}
        }

        false
    }

    /// Check liquidity circuit breaker
    /// Returns true if halt should be triggered
    pub fn check_liquidity(
        &mut self,
        instrument_id: &str,
        bid_count: usize,
        ask_count: usize,
        spread_percent: Option<f64>,
    ) -> bool {
        if !self.config.enabled || !self.config.liquidity_enabled {
            return false;
        }

        // Extract config values first to avoid borrow issues
        let min_orders = self.config.min_bid_ask_orders as usize;
        let max_spread = self.config.max_spread_percent;
        let halt_duration = self.config.liquidity_halt_duration_seconds;
        
        let breaker = self.get_or_create(instrument_id);
        
        // Check order count
        if bid_count < min_orders || ask_count < min_orders {
            warn!(
                instrument = instrument_id,
                bid_count,
                ask_count,
                required = min_orders,
                "Liquidity circuit breaker triggered: insufficient orders"
            );
            
            breaker.liquidity_triggered = true;
            breaker.halted_until = Some(
                Instant::now() + Duration::from_secs(halt_duration)
            );
            
            return true;
        }

        // Check spread
        if let Some(spread) = spread_percent {
            if spread > max_spread {
                warn!(
                    instrument = instrument_id,
                    spread_percent = spread,
                    max_allowed = max_spread,
                    "Liquidity circuit breaker triggered: spread too wide"
                );
                
                breaker.liquidity_triggered = true;
                breaker.halted_until = Some(
                    Instant::now() + Duration::from_secs(halt_duration)
                );
                
                return true;
            }
        }

        false
    }

    /// Clear circuit breakers for an instrument (admin action)
    pub fn clear(&mut self, instrument_id: &str) {
        if let Some(breaker) = self.breakers.get_mut(instrument_id) {
            breaker.clear_halt();
            info!(instrument = instrument_id, "Circuit breakers cleared");
        }
    }

    /// Get status of all circuit breakers
    pub fn status(&self) -> Vec<CircuitBreakerStatus> {
        self.breakers
            .values()
            .map(|b| CircuitBreakerStatus {
                instrument_id: b.instrument_id.clone(),
                halted: b.is_halted(),
                price_movement_triggered: b.price_movement_triggered,
                liquidity_triggered: b.liquidity_triggered,
            })
            .collect()
    }
}

/// Status information for circuit breakers
#[derive(Debug, serde::Serialize)]
pub struct CircuitBreakerStatus {
    pub instrument_id: String,
    pub halted: bool,
    pub price_movement_triggered: bool,
    pub liquidity_triggered: bool,
}
