use crate::black_scholes::{black_scholes_greeks, black_scholes_price, intrinsic_value};
use crate::types::{Greeks, OptionType};
use crate::vol_surface::VolSurface;
use chrono::Utc;
use std::collections::HashMap;

pub const DEFAULT_ALPHA: f64 = 0.15;
pub const NEAR_EXPIRY_THRESHOLD: f64 = 0.001;

#[derive(Debug, Clone)]
pub struct MarkPriceEngine {
    index_prices: HashMap<String, f64>,
    vol_surfaces: HashMap<String, VolSurface>,
    mark_prices: HashMap<String, f64>,
    alpha: f64,
    mark_timestamps: HashMap<String, chrono::DateTime<Utc>>,
}

impl MarkPriceEngine {
    pub fn new() -> Self {
        Self {
            index_prices: HashMap::new(),
            vol_surfaces: HashMap::new(),
            mark_prices: HashMap::new(),
            alpha: DEFAULT_ALPHA,
            mark_timestamps: HashMap::new(),
        }
    }
    
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            index_prices: HashMap::new(),
            vol_surfaces: HashMap::new(),
            mark_prices: HashMap::new(),
            alpha: alpha.clamp(0.01, 0.5),
            mark_timestamps: HashMap::new(),
        }
    }
    
    pub fn update_index_price(&mut self, underlying: String, price: f64) {
        self.index_prices.insert(underlying, price);
    }
    
    pub fn get_index_price(&self, underlying: &str) -> Option<f64> {
        self.index_prices.get(underlying).copied()
    }
    
    pub fn set_vol_surface(&mut self, underlying: String, surface: VolSurface) {
        self.vol_surfaces.insert(underlying, surface);
    }
    
    fn get_or_create_surface(&mut self, underlying: &str) -> &mut VolSurface {
        self.vol_surfaces
            .entry(underlying.to_string())
            .or_insert_with(|| VolSurface::new(underlying.to_string()))
    }
    
    pub fn calculate_mark_price(
        &mut self,
        instrument_id: &str,
        underlying_symbol: &str,
        strike_price: f64,
        expiry_timestamp: i64,
        option_type: OptionType,
    ) -> f64 {
        let index_price = self.index_prices
            .get(underlying_symbol)
            .copied()
            .unwrap_or(strike_price);
        
        let now = Utc::now().timestamp();
        let time_to_expiry = (expiry_timestamp - now) as f64 / (365.25 * 24.0 * 3600.0);
        
        if time_to_expiry < NEAR_EXPIRY_THRESHOLD {
            return intrinsic_value(index_price, strike_price, option_type);
        }
        
        let surface = self.get_or_create_surface(underlying_symbol);
        let days_to_expiry = (time_to_expiry * 365.25) as u32;
        let vol = surface.get_vol(days_to_expiry, index_price, strike_price);
        
        let input = crate::types::BSInputs {
            spot: index_price,
            strike: strike_price,
            time: time_to_expiry,
            vol,
            rate: 0.0,
            option_type,
        };
        
        let theoretical_price = black_scholes_price(input);
        
        let prev_mark = self.mark_prices
            .get(instrument_id)
            .copied()
            .unwrap_or(theoretical_price);
        
        let smoothed = self.alpha * theoretical_price + (1.0 - self.alpha) * prev_mark;
        
        self.mark_prices.insert(instrument_id.to_string(), smoothed);
        self.mark_timestamps.insert(instrument_id.to_string(), Utc::now());
        
        smoothed
    }
    
    pub fn get_mark_price(&self, instrument_id: &str) -> Option<f64> {
        self.mark_prices.get(instrument_id).copied()
    }
    
    pub fn get_mark_timestamp(&self, instrument_id: &str) -> Option<chrono::DateTime<Utc>> {
        self.mark_timestamps.get(instrument_id).copied()
    }
    
    pub fn calculate_greeks(
        &mut self,
        underlying_symbol: &str,
        strike_price: f64,
        expiry_timestamp: i64,
        option_type: OptionType,
    ) -> Option<Greeks> {
        let index_price = *self.index_prices.get(underlying_symbol)?;
        
        let now = Utc::now().timestamp();
        let time_to_expiry = (expiry_timestamp - now) as f64 / (365.25 * 24.0 * 3600.0);
        
        if time_to_expiry <= 0.0 {
            return None;
        }
        
        let surface = self.get_or_create_surface(underlying_symbol);
        let days_to_expiry = (time_to_expiry * 365.25) as u32;
        let vol = surface.get_vol(days_to_expiry, index_price, strike_price);
        
        let input = crate::types::BSInputs {
            spot: index_price,
            strike: strike_price,
            time: time_to_expiry,
            vol,
            rate: 0.0,
            option_type,
        };
        
        Some(black_scholes_greeks(input))
    }
    
    pub fn get_vol_surface(&self, underlying: &str) -> Option<&VolSurface> {
        self.vol_surfaces.get(underlying)
    }
    
    pub fn set_vol_for_testing(&mut self, underlying: &str, days_to_expiry: u32, spot: f64, strike: f64, vol: f64) {
        let surface = self.get_or_create_surface(underlying);
        surface.set_vol(days_to_expiry, spot, strike, vol);
    }
    
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha.clamp(0.01, 0.5);
    }
    
    pub fn clear(&mut self) {
        self.mark_prices.clear();
        self.mark_timestamps.clear();
    }
}

impl Default for MarkPriceEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_expiry(days: i64) -> i64 {
        (Utc::now() + chrono::Duration::days(days)).timestamp()
    }
    
    #[test]
    fn test_mark_price_calculation() {
        let mut engine = MarkPriceEngine::new();
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        engine.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5);
        
        let mark_price = engine.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        assert!(mark_price > 0.0);
    }
    
    #[test]
    fn test_mark_price_smoothing() {
        let mut engine = MarkPriceEngine::with_alpha(0.15);
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        engine.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5);
        
        let price1 = engine.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        engine.update_index_price("BTC".to_string(), 60000.0);
        
        let price2 = engine.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        let jump = (price2 - price1).abs();
        let index_jump = 10000.0;
        
        assert!(jump < index_jump);
    }
    
    #[test]
    fn test_cached_mark_price() {
        let mut engine = MarkPriceEngine::new();
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        engine.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5);
        
        engine.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        let cached = engine.get_mark_price("BTC-20240315-50000-C");
        assert!(cached.is_some());
    }
    
    #[test]
    fn test_greeks_calculation() {
        let mut engine = MarkPriceEngine::new();
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        engine.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5);
        
        let greeks = engine.calculate_greeks(
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        assert!(greeks.is_some());
        let g = greeks.unwrap();
        assert!(g.delta > 0.0);
        assert!(g.gamma > 0.0);
    }
    
    #[test]
    fn test_index_price_update() {
        let mut engine = MarkPriceEngine::new();
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        
        assert_eq!(engine.get_index_price("BTC"), Some(50000.0));
        
        engine.update_index_price("BTC".to_string(), 55000.0);
        
        assert_eq!(engine.get_index_price("BTC"), Some(55000.0));
    }
    
    #[test]
    fn test_vol_surface_access() {
        let mut engine = MarkPriceEngine::new();
        
        engine.update_index_price("BTC".to_string(), 50000.0);
        engine.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5);
        
        let surface = engine.get_vol_surface("BTC");
        assert!(surface.is_some());
    }
    
    #[test]
    fn test_fallback_when_no_index() {
        let mut engine = MarkPriceEngine::new();
        
        let mark_price = engine.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            create_test_expiry(30),
            OptionType::Call,
        );
        
        assert!(mark_price > 0.0);
    }
}
