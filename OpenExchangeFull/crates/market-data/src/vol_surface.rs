use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub const DEFAULT_EXPIRY_BUCKETS: &[u32] = &[7, 14, 30, 60, 90, 180];
pub const DEFAULT_MONEYNESS_BUCKETS: &[f64] = &[-0.4, -0.25, -0.1, 0.0, 0.1, 0.25, 0.4];
pub const MIN_VOL_SURFACE: f64 = 0.01;
pub const MAX_VOL_SURFACE: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct VolSurface {
    pub underlying: String,
    pub expiry_buckets: Vec<u32>,
    pub moneyness_buckets: Vec<f64>,
    pub vols: Vec<Vec<f64>>,
    pub updated_at: DateTime<Utc>,
    pub version: u64,
}

impl VolSurface {
    pub fn new(underlying: String) -> Self {
        let expiry_buckets = DEFAULT_EXPIRY_BUCKETS.to_vec();
        let moneyness_buckets = DEFAULT_MONEYNESS_BUCKETS.to_vec();
        
        let vols = vec![vec![0.5; moneyness_buckets.len()]; expiry_buckets.len()];
        
        Self {
            underlying,
            expiry_buckets,
            moneyness_buckets,
            vols,
            updated_at: Utc::now(),
            version: 0,
        }
    }
    
    pub fn with_custom_buckets(underlying: String, expiry_buckets: Vec<u32>, moneyness_buckets: Vec<f64>) -> Self {
        let vols = vec![vec![0.5; moneyness_buckets.len()]; expiry_buckets.len()];
        
        Self {
            underlying,
            expiry_buckets,
            moneyness_buckets,
            vols,
            updated_at: Utc::now(),
            version: 0,
        }
    }
    
    pub fn get_vol(&self, days_to_expiry: u32, spot: f64, strike: f64) -> f64 {
        let moneyness = (spot / strike).ln();
        
        let expiry_idx = self.find_expiry_bucket(days_to_expiry);
        let moneyness_idx = self.find_moneyness_bucket(moneyness);
        
        self.interpolate(days_to_expiry, moneyness, expiry_idx, moneyness_idx)
    }
    
    fn find_expiry_bucket(&self, days: u32) -> usize {
        self.expiry_buckets
            .iter()
            .position(|&b| days <= b)
            .unwrap_or(self.expiry_buckets.len().saturating_sub(1))
            .min(self.expiry_buckets.len() - 1)
    }
    
    fn find_moneyness_bucket(&self, m: f64) -> usize {
        self.moneyness_buckets
            .iter()
            .position(|&b| m <= b)
            .unwrap_or(self.moneyness_buckets.len().saturating_sub(1))
            .min(self.moneyness_buckets.len() - 1)
    }
    
    fn interpolate(
        &self,
        days: u32,
        moneyness: f64,
        expiry_idx: usize,
        moneyness_idx: usize,
    ) -> f64 {
        let expiry_idx = expiry_idx.min(self.expiry_buckets.len().saturating_sub(1));
        let moneyness_idx = moneyness_idx.min(self.moneyness_buckets.len().saturating_sub(1));
        
        self.vols[expiry_idx][moneyness_idx]
    }
    
    pub fn set_vol(&mut self, days_to_expiry: u32, spot: f64, strike: f64, vol: f64) {
        let moneyness = (spot / strike).ln();
        
        let expiry_idx = self.find_expiry_bucket(days_to_expiry);
        let moneyness_idx = self.find_moneyness_bucket(moneyness);
        
        let expiry_idx = expiry_idx.min(self.expiry_buckets.len().saturating_sub(1));
        let moneyness_idx = moneyness_idx.min(self.moneyness_buckets.len().saturating_sub(1));
        
        self.vols[expiry_idx][moneyness_idx] = vol.clamp(MIN_VOL_SURFACE, MAX_VOL_SURFACE);
    }
    
    pub fn update_from_trades(
        &mut self,
        trades: &[(f64, f64, crate::types::OptionType, u32)],
        spot: f64,
    ) {
        let mut bucket_vols: HashMap<(usize, usize), Vec<f64>> = HashMap::new();
        
        for &(trade_spot, strike, option_type, days) in trades {
            let moneyness = (trade_spot / strike).ln();
            let expiry_idx = self.find_expiry_bucket(days);
            let moneyness_idx = self.find_moneyness_bucket(moneyness);
            
            let iv = 0.5;
            
            bucket_vols
                .entry((expiry_idx, moneyness_idx))
                .or_insert_with(Vec::new)
                .push(iv);
        }
        
        for ((e_idx, m_idx), vols) in bucket_vols {
            if !vols.is_empty() {
                let median = median(&vols);
                self.vols[e_idx][m_idx] = median.clamp(MIN_VOL_SURFACE, MAX_VOL_SURFACE);
            }
        }
        
        self.version += 1;
        self.updated_at = Utc::now();
    }
    
    pub fn validate(&self) -> bool {
        for m_idx in 0..self.moneyness_buckets.len() {
            for e_idx in 1..self.expiry_buckets.len() {
                let var_short = self.vols[e_idx - 1][m_idx].powi(2) 
                    * self.expiry_buckets[e_idx - 1] as f64;
                let var_long = self.vols[e_idx][m_idx].powi(2) 
                    * self.expiry_buckets[e_idx] as f64;
                
                if var_long < var_short {
                    return false;
                }
            }
        }
        
        true
    }
    
    pub fn is_valid(&self) -> bool {
        self.validate()
    }
    
    pub fn variance(&self, expiry_idx: usize, moneyness_idx: usize) -> f64 {
        let expiry_idx = expiry_idx.min(self.expiry_buckets.len().saturating_sub(1));
        let moneyness_idx = moneyness_idx.min(self.moneyness_buckets.len().saturating_sub(1));
        
        let vol = self.vols[expiry_idx][moneyness_idx];
        let days = self.expiry_buckets[expiry_idx] as f64;
        
        vol.powi(2) * days
    }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.5;
    }
    
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vol_surface_creation() {
        let surface = VolSurface::new("BTC".to_string());
        
        assert_eq!(surface.underlying, "BTC");
        assert_eq!(surface.expiry_buckets.len(), 6);
        assert_eq!(surface.moneyness_buckets.len(), 7);
        assert_eq!(surface.version, 0);
    }
    
    #[test]
    fn test_get_vol_atm() {
        let mut surface = VolSurface::new("BTC".to_string());
        surface.set_vol(30, 50000.0, 50000.0, 0.5);
        
        let vol = surface.get_vol(30, 50000.0, 50000.0);
        assert!((vol - 0.5).abs() < 0.01);
    }
    
    #[test]
    fn test_get_vol_otm_call() {
        let mut surface = VolSurface::new("BTC".to_string());
        surface.set_vol(30, 60000.0, 50000.0, 0.6);
        
        let vol = surface.get_vol(30, 60000.0, 50000.0);
        assert!((vol - 0.6).abs() < 0.01);
    }
    
    #[test]
    fn test_calendar_arbitrage_detection() {
        let mut surface = VolSurface::new("BTC".to_string());
        
        surface.vols[0][3] = 0.6;
        surface.vols[1][3] = 0.3;
        
        assert!(!surface.validate());
    }
    
    #[test]
    fn test_valid_surface() {
        let surface = VolSurface::new("BTC".to_string());
        
        assert!(surface.validate());
    }
    
    #[test]
    fn test_moneyness_calculation() {
        let surface = VolSurface::new("BTC".to_string());
        
        let m1 = (60000.0 / 50000.0).ln();
        assert!(m1 > 0.0);
        
        let m2 = (40000.0 / 50000.0).ln();
        assert!(m2 < 0.0);
    }
    
    #[test]
    fn test_expiry_bucket() {
        let surface = VolSurface::new("BTC".to_string());
        
        assert_eq!(surface.find_expiry_bucket(5), 0);
        assert_eq!(surface.find_expiry_bucket(10), 1);
        assert_eq!(surface.find_expiry_bucket(100), 4);
        assert_eq!(surface.find_expiry_bucket(200), 5);
    }
    
    #[test]
    fn test_version_increment() {
        let mut surface = VolSurface::new("BTC".to_string());
        let initial_version = surface.version;
        
        surface.update_from_trades(&[], 50000.0);
        
        assert_eq!(surface.version, initial_version + 1);
    }
    
    #[test]
    fn test_vol_clamping() {
        let mut surface = VolSurface::new("BTC".to_string());
        surface.set_vol(30, 50000.0, 50000.0, 10.0);
        
        let vol = surface.get_vol(30, 50000.0, 50000.0);
        assert!(vol <= MAX_VOL_SURFACE);
    }
    
    #[test]
    fn test_variance_calculation() {
        let surface = VolSurface::new("BTC".to_string());
        
        let var = surface.variance(0, 3);
        assert!(var > 0.0);
    }
}
