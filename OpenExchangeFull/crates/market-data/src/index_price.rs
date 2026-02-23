use crate::types::IndexPrice;
use chrono::Utc;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PriceSource {
    pub name: String,
    pub price: f64,
    pub timestamp: chrono::DateTime<Utc>,
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct IndexPriceAggregator {
    sources: HashMap<String, Vec<PriceSource>>,
    latest_index_prices: HashMap<String, IndexPrice>,
    outlier_threshold: f64,
}

impl IndexPriceAggregator {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            latest_index_prices: HashMap::new(),
            outlier_threshold: 3.0,
        }
    }
    
    pub fn with_outlier_threshold(threshold: f64) -> Self {
        Self {
            sources: HashMap::new(),
            latest_index_prices: HashMap::new(),
            outlier_threshold: threshold,
        }
    }
    
    pub fn add_source(&mut self, asset: String, _source_name: String) {
        self.sources
            .entry(asset)
            .or_insert_with(Vec::new);
    }
    
    pub fn update_price(
        &mut self,
        asset: &str,
        source_name: &str,
        price: f64,
    ) {
        let source = PriceSource {
            name: source_name.to_string(),
            price,
            timestamp: Utc::now(),
            weight: 1.0,
        };
        
        let sources = self.sources.entry(asset.to_string()).or_insert_with(Vec::new);
        
        if let Some(existing) = sources.iter_mut().find(|s| s.name == source_name) {
            *existing = source;
        } else {
            sources.push(source);
        }
        
        self.recalculate_index(asset);
    }
    
    fn recalculate_index(&mut self, asset: &str) {
        let sources = match self.sources.get(asset) {
            Some(s) => s,
            None => return,
        };
        
        if sources.is_empty() {
            return;
        }
        
        let prices: Vec<f64> = sources.iter().map(|s| s.price).collect();
        
        if prices.is_empty() {
            return;
        }
        
        let median_price = median(&prices);
        
        let std_dev = if prices.len() > 1 {
            let mean = prices.iter().sum::<f64>() / prices.len() as f64;
            let variance = prices.iter()
                .map(|p| (p - mean).powi(2))
                .sum::<f64>() / prices.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };
        
        let filtered_prices: Vec<f64> = if std_dev > 0.0 {
            sources.iter()
                .filter(|s| {
                    let z_score = (s.price - median_price).abs() / std_dev;
                    z_score < self.outlier_threshold
                })
                .map(|s| s.price)
                .collect()
        } else {
            prices
        };
        
        if filtered_prices.is_empty() {
            return;
        }
        
        let final_price = median(&filtered_prices);
        
        let (min_price, max_price) = filtered_prices.iter()
            .fold((f64::MAX, f64::MIN), |(min, max), &p| {
                (min.min(p), max.max(p))
            });
        
        let spread = max_price - min_price;
        let confidence = if final_price > 0.0 {
            (1.0 - (spread / final_price).min(1.0)).max(0.0)
        } else {
            0.0
        };
        
        let index_price = IndexPrice {
            asset: asset.to_string(),
            price: final_price,
            timestamp: Utc::now(),
            confidence,
        };
        
        self.latest_index_prices.insert(asset.to_string(), index_price);
    }
    
    pub fn get_index_price(&self, asset: &str) -> Option<IndexPrice> {
        self.latest_index_prices.get(asset).cloned()
    }
    
    pub fn get_sources(&self, asset: &str) -> Option<&Vec<PriceSource>> {
        self.sources.get(asset)
    }
    
    pub fn remove_source(&mut self, asset: &str, source_name: &str) -> bool {
        if let Some(sources) = self.sources.get_mut(asset) {
            let initial_len = sources.len();
            sources.retain(|s| s.name != source_name);
            
            if sources.len() != initial_len {
                self.recalculate_index(asset);
                return true;
            }
        }
        false
    }
    
    pub fn clear_asset(&mut self, asset: &str) {
        self.sources.remove(asset);
        self.latest_index_prices.remove(asset);
    }
    
    pub fn clear(&mut self) {
        self.sources.clear();
        self.latest_index_prices.clear();
    }
    
    pub fn source_count(&self, asset: &str) -> usize {
        self.sources.get(asset).map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for IndexPriceAggregator {
    fn default() -> Self {
        Self::new()
    }
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
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
    fn test_single_source() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.update_price("BTC", "binance", 50000.0);
        
        let index = aggregator.get_index_price("BTC");
        assert!(index.is_some());
        assert!((index.unwrap().price - 50000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_multiple_sources_median() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("BTC".to_string(), "coinbase".to_string());
        aggregator.add_source("BTC".to_string(), "kraken".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "coinbase", 50200.0);
        aggregator.update_price("BTC", "kraken", 49800.0);
        
        let index = aggregator.get_index_price("BTC").unwrap();
        
        assert!((index.price - 50000.0).abs() < 1.0);
    }
    
    #[test]
    fn test_outlier_rejection() {
        let mut aggregator = IndexPriceAggregator::with_outlier_threshold(2.0);
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("BTC".to_string(), "coinbase".to_string());
        aggregator.add_source("BTC".to_string(), "malicious".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "coinbase", 50100.0);
        aggregator.update_price("BTC", "malicious", 100000.0);
        
        let index = aggregator.get_index_price("BTC").unwrap();
        
        assert!((index.price - 50050.0).abs() < 100.0);
    }
    
    #[test]
    fn test_confidence_calculation() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("BTC".to_string(), "coinbase".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "coinbase", 50100.0);
        
        let index = aggregator.get_index_price("BTC").unwrap();
        
        assert!(index.confidence > 0.5);
    }
    
    #[test]
    fn test_confidence_low_spread() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("BTC".to_string(), "coinbase".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "coinbase", 50010.0);
        
        let index = aggregator.get_index_price("BTC").unwrap();
        
        assert!(index.confidence > 0.9);
    }
    
    #[test]
    fn test_source_removal() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("BTC".to_string(), "coinbase".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "coinbase", 50200.0);
        
        aggregator.remove_source("BTC", "coinbase");
        
        let index = aggregator.get_index_price("BTC").unwrap();
        assert!((index.price - 50000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_update_existing_source() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("BTC", "binance", 51000.0);
        
        let index = aggregator.get_index_price("BTC").unwrap();
        assert!((index.price - 51000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_no_sources() {
        let aggregator = IndexPriceAggregator::new();
        
        let index = aggregator.get_index_price("BTC");
        assert!(index.is_none());
    }
    
    #[test]
    fn test_asset_separation() {
        let mut aggregator = IndexPriceAggregator::new();
        
        aggregator.add_source("BTC".to_string(), "binance".to_string());
        aggregator.add_source("ETH".to_string(), "binance".to_string());
        
        aggregator.update_price("BTC", "binance", 50000.0);
        aggregator.update_price("ETH", "binance", 3000.0);
        
        let btc_index = aggregator.get_index_price("BTC").unwrap();
        let eth_index = aggregator.get_index_price("ETH").unwrap();
        
        assert!((btc_index.price - 50000.0).abs() < 0.01);
        assert!((eth_index.price - 3000.0).abs() < 0.01);
    }
}
