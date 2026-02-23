use crate::types::Trade;
use chrono::{DateTime, Duration, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandleInterval {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    FourHours,
    OneDay,
}

impl CandleInterval {
    pub fn as_seconds(&self) -> i64 {
        match self {
            CandleInterval::OneMinute => 60,
            CandleInterval::FiveMinutes => 300,
            CandleInterval::FifteenMinutes => 900,
            CandleInterval::OneHour => 3600,
            CandleInterval::FourHours => 14400,
            CandleInterval::OneDay => 86400,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            CandleInterval::OneMinute => "1m",
            CandleInterval::FiveMinutes => "5m",
            CandleInterval::FifteenMinutes => "15m",
            CandleInterval::OneHour => "1h",
            CandleInterval::FourHours => "4h",
            CandleInterval::OneDay => "1d",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "1m" => Some(CandleInterval::OneMinute),
            "5m" => Some(CandleInterval::FiveMinutes),
            "15m" => Some(CandleInterval::FifteenMinutes),
            "1h" => Some(CandleInterval::OneHour),
            "4h" => Some(CandleInterval::FourHours),
            "1d" => Some(CandleInterval::OneDay),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub trade_count: u64,
    pub open_time: DateTime<Utc>,
    pub interval: CandleInterval,
}

impl Candle {
    pub fn new(open_time: DateTime<Utc>, interval: CandleInterval) -> Self {
        Self {
            open: 0.0,
            high: f64::MIN,
            low: f64::MAX,
            close: 0.0,
            volume: 0.0,
            trade_count: 0,
            open_time,
            interval,
        }
    }
    
    pub fn with_price(open_time: DateTime<Utc>, interval: CandleInterval, price: f64, quantity: f64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
            trade_count: 1,
            open_time,
            interval,
        }
    }
    
    pub fn update(&mut self, price: f64, quantity: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += quantity;
        self.trade_count += 1;
    }
    
    pub fn is_closed(&self, current_time: DateTime<Utc>) -> bool {
        let interval_seconds = self.interval.as_seconds();
        let elapsed = (current_time - self.open_time).num_seconds();
        elapsed >= interval_seconds
    }
}

#[derive(Debug, Clone)]
pub struct CandleBuilder {
    candles: HashMap<(String, CandleInterval), Vec<Candle>>,
    current_candles: HashMap<(String, CandleInterval), Candle>,
    default_intervals: Vec<CandleInterval>,
}

impl CandleBuilder {
    pub fn new() -> Self {
        Self {
            candles: HashMap::new(),
            current_candles: HashMap::new(),
            default_intervals: vec![
                CandleInterval::OneMinute,
                CandleInterval::FiveMinutes,
                CandleInterval::FifteenMinutes,
                CandleInterval::OneHour,
                CandleInterval::FourHours,
                CandleInterval::OneDay,
            ],
        }
    }
    
    pub fn with_intervals(intervals: Vec<CandleInterval>) -> Self {
        Self {
            candles: HashMap::new(),
            current_candles: HashMap::new(),
            default_intervals: intervals,
        }
    }
    
    fn get_open_time(timestamp: DateTime<Utc>, interval: CandleInterval) -> DateTime<Utc> {
        let seconds = timestamp.timestamp();
        let interval_seconds = interval.as_seconds();
        let open_seconds = (seconds / interval_seconds) * interval_seconds;
        Utc.timestamp_opt(open_seconds, 0).unwrap()
    }
    
    pub fn add_trade(&mut self, trade: &Trade) {
        let intervals = self.default_intervals.clone();
        for interval in intervals {
            self.add_trade_to_interval(trade, interval);
        }
    }
    
    pub fn add_trade_to_interval(&mut self, trade: &Trade, interval: CandleInterval) {
        let key = (trade.instrument_id.clone(), interval);
        let open_time = Self::get_open_time(trade.timestamp, interval);
        
        if let Some(current) = self.current_candles.get_mut(&key) {
            if current.open_time == open_time {
                current.update(trade.price, trade.quantity as f64);
            } else {
                let closed_candle = self.candles
                    .entry(key.clone())
                    .or_insert_with(Vec::new);
                closed_candle.push(*current);
                
                let new_candle = Candle::with_price(open_time, interval, trade.price, trade.quantity as f64);
                self.current_candles.insert(key, new_candle);
            }
        } else {
            let new_candle = Candle::with_price(open_time, interval, trade.price, trade.quantity as f64);
            self.current_candles.insert(key, new_candle);
        }
    }
    
    pub fn get_candles(&self, instrument_id: &str, interval: CandleInterval, limit: usize) -> Vec<Candle> {
        let key = (instrument_id.to_string(), interval);
        
        let mut result = self.candles.get(&key)
            .cloned()
            .unwrap_or_default();
        
        if let Some(current) = self.current_candles.get(&key) {
            result.push(*current);
        }
        
        if limit > 0 && result.len() > limit {
            result.split_off(result.len() - limit);
        }
        
        result
    }
    
    pub fn get_candles_str(&self, instrument_id: &str, interval_str: &str, limit: usize) -> Option<Vec<Candle>> {
        let interval = CandleInterval::from_str(interval_str)?;
        Some(self.get_candles(instrument_id, interval, limit))
    }
    
    pub fn close_expired_candles(&mut self) {
        let now = Utc::now();
        
        let keys: Vec<_> = self.current_candles.keys().cloned().collect();
        
        for key in keys {
            if let Some(current) = self.current_candles.get(&key) {
                if current.is_closed(now) {
                    let closed = self.candles
                        .entry(key.clone())
                        .or_insert_with(Vec::new);
                    closed.push(*current);
                    self.current_candles.remove(&key);
                }
            }
        }
    }
    
    pub fn close_instrument_candles(&mut self, instrument_id: &str) {
        let keys: Vec<_> = self.current_candles.keys()
            .filter(|(id, _)| id == instrument_id)
            .cloned()
            .collect();
        
        for key in keys {
            if let Some(current) = self.current_candles.remove(&key) {
                let closed = self.candles
                    .entry(key)
                    .or_insert_with(Vec::new);
                closed.push(current);
            }
        }
    }
    
    pub fn clear(&mut self) {
        self.candles.clear();
        self.current_candles.clear();
    }
    
    pub fn candle_count(&self, instrument_id: &str, interval: CandleInterval) -> usize {
        let key = (instrument_id.to_string(), interval);
        self.candles.get(&key).map(|c| c.len()).unwrap_or(0)
    }
    
    pub fn latest_candle(&self, instrument_id: &str, interval: CandleInterval) -> Option<Candle> {
        let key = (instrument_id.to_string(), interval);
        self.current_candles.get(&key).copied()
    }
}

impl Default for CandleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_trade(instrument_id: &str, price: f64, quantity: u32, seconds_offset: i64) -> Trade {
        Trade {
            trade_id: "test".to_string(),
            instrument_id: instrument_id.to_string(),
            price,
            quantity,
            aggressor_side: Some("buy".to_string()),
            timestamp: Utc::now() + Duration::seconds(seconds_offset),
        }
    }
    
    #[test]
    fn test_single_trade_candle() {
        let mut builder = CandleBuilder::new();
        
        let trade = create_test_trade("BTC", 50000.0, 1, 0);
        builder.add_trade(&trade);
        
        let candles = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        
        assert_eq!(candles.len(), 1);
        assert!((candles[0].close - 50000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_multiple_trades_same_interval() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 0));
        builder.add_trade(&create_test_trade("BTC", 51000.0, 1, 10));
        builder.add_trade(&create_test_trade("BTC", 50500.0, 1, 20));
        
        let candles = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        
        assert_eq!(candles.len(), 1);
        assert!((candles[0].high - 51000.0).abs() < 0.01);
        assert!((candles[0].low - 50000.0).abs() < 0.01);
        assert!((candles[0].close - 50500.0).abs() < 0.01);
    }
    
    #[test]
    fn test_volume_accumulation() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 10, 0));
        builder.add_trade(&create_test_trade("BTC", 50000.0, 20, 10));
        
        let candles = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        
        assert!((candles[0].volume - 30.0).abs() < 0.01);
    }
    
    #[test]
    fn test_trade_count() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 0));
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 10));
        
        let candles = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        
        assert_eq!(candles[0].trade_count, 2);
    }
    
    #[test]
    fn test_interval_separation() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 0));
        
        let m1 = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        let m5 = builder.get_candles("BTC", CandleInterval::FiveMinutes, 10);
        
        assert!(!m1.is_empty());
        assert!(!m5.is_empty());
    }
    
    #[test]
    fn test_instrument_separation() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 0));
        builder.add_trade(&create_test_trade("ETH", 3000.0, 1, 0));
        
        let btc = builder.get_candles("BTC", CandleInterval::OneMinute, 10);
        let eth = builder.get_candles("ETH", CandleInterval::OneMinute, 10);
        
        assert!(!btc.is_empty());
        assert!(!eth.is_empty());
        assert!((btc[0].close - 50000.0).abs() < 0.01);
        assert!((eth[0].close - 3000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_limit() {
        let mut builder = CandleBuilder::new();
        
        for i in 0..10 {
            builder.add_trade(&create_test_trade("BTC", 50000.0 + i as f64, 1, i * 100));
        }
        
        let candles = builder.get_candles("BTC", CandleInterval::OneMinute, 5);
        
        assert!(candles.len() <= 5);
    }
    
    #[test]
    fn test_latest_candle() {
        let mut builder = CandleBuilder::new();
        
        builder.add_trade(&create_test_trade("BTC", 50000.0, 1, 0));
        
        let latest = builder.latest_candle("BTC", CandleInterval::OneMinute);
        
        assert!(latest.is_some());
    }
    
    #[test]
    fn test_candle_interval_str() {
        assert_eq!(CandleInterval::OneMinute.as_str(), "1m");
        assert_eq!(CandleInterval::OneHour.as_str(), "1h");
        assert_eq!(CandleInterval::from_str("1h"), Some(CandleInterval::OneHour));
    }
}
