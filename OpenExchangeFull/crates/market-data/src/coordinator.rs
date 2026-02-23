use crate::candles::{Candle, CandleBuilder, CandleInterval};
use crate::index_price::IndexPriceAggregator;
use crate::mark_price::MarkPriceEngine;
use crate::order_book::{OrderBookBuilder, OrderSide};
use crate::types::{Greeks, IndexPrice, MarkPrice, OptionType, OrderBookSnapshot, Trade};
use crate::vol_surface::VolSurface;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct MarketDataCoordinator {
    mark_price_engine: Arc<RwLock<MarkPriceEngine>>,
    order_book_builder: Arc<RwLock<OrderBookBuilder>>,
    index_price_aggregator: Arc<RwLock<IndexPriceAggregator>>,
    candle_builder: Arc<RwLock<CandleBuilder>>,
}

impl MarketDataCoordinator {
    pub fn new() -> Self {
        Self {
            mark_price_engine: Arc::new(RwLock::new(MarkPriceEngine::new())),
            order_book_builder: Arc::new(RwLock::new(OrderBookBuilder::new())),
            index_price_aggregator: Arc::new(RwLock::new(IndexPriceAggregator::new())),
            candle_builder: Arc::new(RwLock::new(CandleBuilder::new())),
        }
    }
    
    pub async fn update_index_price(&self, underlying: String, price: f64) {
        let mut engine = self.mark_price_engine.write().await;
        engine.update_index_price(underlying.clone(), price);
    }
    
    pub async fn update_index_price_with_source(
        &self,
        underlying: String,
        source: String,
        price: f64,
    ) {
        {
            let mut aggregator = self.index_price_aggregator.write().await;
            aggregator.add_source(underlying.clone(), source.clone());
            aggregator.update_price(&underlying, &source, price);
        }
        
        if let Some(index_price) = self.get_index_price(&underlying).await {
            let mut engine = self.mark_price_engine.write().await;
            engine.update_index_price(underlying, index_price.price);
        }
    }
    
    pub async fn calculate_mark_price(
        &self,
        instrument_id: &str,
        underlying_symbol: &str,
        strike_price: f64,
        expiry_timestamp: i64,
        option_type: OptionType,
    ) -> f64 {
        let mut engine = self.mark_price_engine.write().await;
        engine.calculate_mark_price(
            instrument_id,
            underlying_symbol,
            strike_price,
            expiry_timestamp,
            option_type,
        )
    }
    
    pub async fn get_mark_price(&self, instrument_id: &str) -> Option<f64> {
        let engine = self.mark_price_engine.read().await;
        engine.get_mark_price(instrument_id)
    }
    
    pub async fn get_mark_price_full(&self, instrument_id: &str) -> Option<MarkPrice> {
        let engine = self.mark_price_engine.read().await;
        
        let mark_price = engine.get_mark_price(instrument_id)?;
        let index_price = None;
        let implied_vol = None;
        
        Some(MarkPrice {
            instrument_id: instrument_id.to_string(),
            mark_price,
            index_price: index_price.unwrap_or(0.0),
            implied_vol: implied_vol.unwrap_or(0.0),
            timestamp: Utc::now(),
        })
    }
    
    pub async fn get_greeks(
        &self,
        underlying_symbol: &str,
        strike_price: f64,
        expiry_timestamp: i64,
        option_type: OptionType,
    ) -> Option<Greeks> {
        let mut engine = self.mark_price_engine.write().await;
        engine.calculate_greeks(underlying_symbol, strike_price, expiry_timestamp, option_type)
    }
    
    pub async fn get_order_book(&self, instrument_id: &str, sequence: u64) -> OrderBookSnapshot {
        let builder = self.order_book_builder.read().await;
        builder.build_snapshot(instrument_id, sequence)
    }
    
    pub async fn get_index_price(&self, asset: &str) -> Option<IndexPrice> {
        let aggregator = self.index_price_aggregator.read().await;
        aggregator.get_index_price(asset)
    }
    
    pub async fn get_vol_surface(&self, underlying: &str) -> Option<VolSurface> {
        let engine = self.mark_price_engine.read().await;
        engine.get_vol_surface(underlying).cloned()
    }
    
    pub async fn get_candles(
        &self,
        instrument_id: &str,
        interval: CandleInterval,
        limit: usize,
    ) -> Vec<Candle> {
        let builder = self.candle_builder.read().await;
        builder.get_candles(instrument_id, interval, limit)
    }
    
    pub async fn get_candles_str(
        &self,
        instrument_id: &str,
        interval_str: &str,
        limit: usize,
    ) -> Option<Vec<Candle>> {
        let builder = self.candle_builder.read().await;
        builder.get_candles_str(instrument_id, interval_str, limit)
    }
    
    pub async fn on_trade(&self, trade: Trade) {
        let mut builder = self.candle_builder.write().await;
        builder.add_trade(&trade);
    }
    
    pub async fn on_order_book_update(
        &self,
        instrument_id: String,
        bids: Vec<(f64, u32)>,
        asks: Vec<(f64, u32)>,
    ) {
        let mut builder = self.order_book_builder.write().await;
        
        for (price, quantity) in bids {
            builder.add_order(instrument_id.clone(), OrderSide::Buy, price, quantity);
        }
        
        for (price, quantity) in asks {
            builder.add_order(instrument_id.clone(), OrderSide::Sell, price, quantity);
        }
    }
    
    pub async fn on_order_added(
        &self,
        instrument_id: String,
        side: OrderSide,
        price: f64,
        quantity: u32,
    ) {
        let mut builder = self.order_book_builder.write().await;
        builder.add_order(instrument_id, side, price, quantity);
    }
    
    pub async fn on_order_removed(
        &self,
        instrument_id: &str,
        side: OrderSide,
        price: f64,
        quantity: u32,
    ) -> bool {
        let mut builder = self.order_book_builder.write().await;
        builder.remove_order(instrument_id, side, price, quantity)
    }
    
    pub async fn set_vol_surface(&self, underlying: String, surface: VolSurface) {
        let mut engine = self.mark_price_engine.write().await;
        engine.set_vol_surface(underlying, surface);
    }
    
    pub async fn set_vol_for_testing(
        &self,
        underlying: &str,
        days_to_expiry: u32,
        spot: f64,
        strike: f64,
        vol: f64,
    ) {
        let mut engine = self.mark_price_engine.write().await;
        engine.set_vol_for_testing(underlying, days_to_expiry, spot, strike, vol);
    }
    
    pub async fn get_best_bid(&self, instrument_id: &str) -> Option<f64> {
        let builder = self.order_book_builder.read().await;
        builder.get_best_bid(instrument_id)
    }
    
    pub async fn get_best_ask(&self, instrument_id: &str) -> Option<f64> {
        let builder = self.order_book_builder.read().await;
        builder.get_best_ask(instrument_id)
    }
    
    pub async fn get_mid_price(&self, instrument_id: &str) -> Option<f64> {
        let builder = self.order_book_builder.read().await;
        builder.get_mid_price(instrument_id)
    }
    
    pub async fn get_spread(&self, instrument_id: &str) -> Option<f64> {
        let builder = self.order_book_builder.read().await;
        builder.get_spread(instrument_id)
    }
    
    pub async fn clear_instrument(&self, instrument_id: &str) {
        let mut builder = self.order_book_builder.write().await;
        builder.clear_instrument(instrument_id);
        
        let mut candle_builder = self.candle_builder.write().await;
        candle_builder.close_instrument_candles(instrument_id);
    }
    
    pub async fn close_expired_candles(&self) {
        let mut builder = self.candle_builder.write().await;
        builder.close_expired_candles();
    }
}

impl Default for MarketDataCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mark_price_calculation() {
        let coordinator = MarketDataCoordinator::new();
        
        coordinator.update_index_price("BTC".to_string(), 50000.0).await;
        coordinator.set_vol_for_testing("BTC", 30, 50000.0, 50000.0, 0.5).await;
        
        let mark_price = coordinator.calculate_mark_price(
            "BTC-20240315-50000-C",
            "BTC",
            50000.0,
            (Utc::now() + chrono::Duration::days(30)).timestamp(),
            OptionType::Call,
        ).await;
        
        assert!(mark_price > 0.0);
    }
    
    #[tokio::test]
    async fn test_index_price_aggregation() {
        let coordinator = MarketDataCoordinator::new();
        
        coordinator.update_index_price_with_source(
            "BTC".to_string(),
            "binance".to_string(),
            50000.0,
        ).await;
        
        coordinator.update_index_price_with_source(
            "BTC".to_string(),
            "coinbase".to_string(),
            50200.0,
        ).await;
        
        let index = coordinator.get_index_price("BTC").await;
        assert!(index.is_some());
    }
    
    #[tokio::test]
    async fn test_order_book_update() {
        let coordinator = MarketDataCoordinator::new();
        
        coordinator.on_order_book_update(
            "BTC".to_string(),
            vec![(50000.0, 10)],
            vec![(51000.0, 5)],
        ).await;
        
        let best_bid = coordinator.get_best_bid("BTC").await;
        let best_ask = coordinator.get_best_ask("BTC").await;
        
        assert_eq!(best_bid, Some(50000.0));
        assert_eq!(best_ask, Some(51000.0));
    }
    
    #[tokio::test]
    async fn test_candle_generation() {
        let coordinator = MarketDataCoordinator::new();
        
        let trade = Trade {
            trade_id: "test".to_string(),
            instrument_id: "BTC".to_string(),
            price: 50000.0,
            quantity: 1,
            aggressor_side: Some("buy".to_string()),
            timestamp: Utc::now(),
        };
        
        coordinator.on_trade(trade).await;
        
        let candles = coordinator.get_candles("BTC", CandleInterval::OneMinute, 10).await;
        
        assert!(!candles.is_empty());
    }
    
    #[tokio::test]
    async fn test_vol_surface_setting() {
        let coordinator = MarketDataCoordinator::new();
        
        let mut surface = VolSurface::new("BTC".to_string());
        surface.set_vol(30, 50000.0, 50000.0, 0.5);
        
        coordinator.set_vol_surface("BTC".to_string(), surface).await;
        
        let retrieved = coordinator.get_vol_surface("BTC").await;
        assert!(retrieved.is_some());
    }
    
    #[tokio::test]
    async fn test_spread_calculation() {
        let coordinator = MarketDataCoordinator::new();
        
        coordinator.on_order_book_update(
            "BTC".to_string(),
            vec![(50000.0, 10)],
            vec![(51000.0, 5)],
        ).await;
        
        let spread = coordinator.get_spread("BTC").await;
        assert_eq!(spread, Some(1000.0));
    }
    
    #[tokio::test]
    async fn test_mid_price_calculation() {
        let coordinator = MarketDataCoordinator::new();
        
        coordinator.on_order_book_update(
            "BTC".to_string(),
            vec![(50000.0, 10)],
            vec![(51000.0, 5)],
        ).await;
        
        let mid = coordinator.get_mid_price("BTC").await;
        assert_eq!(mid, Some(50500.0));
    }
}
