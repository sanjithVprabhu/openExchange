use crate::types::{OrderBookSnapshot, PriceLevel};
use chrono::Utc;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
struct BookOrder {
    quantity: u32,
}

#[derive(Debug, Clone)]
pub struct OrderBookBuilder {
    bids: BTreeMap<String, BTreeMap<ordered_float::OrderedFloat<f64>, Vec<BookOrder>>>,
    asks: BTreeMap<String, BTreeMap<ordered_float::OrderedFloat<f64>, Vec<BookOrder>>>,
}

impl OrderBookBuilder {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
    
    pub fn add_order(
        &mut self,
        instrument_id: String,
        side: OrderSide,
        price: f64,
        quantity: u32,
    ) {
        let order = BookOrder { quantity };
        
        match side {
            OrderSide::Buy => {
                self.bids
                    .entry(instrument_id)
                    .or_insert_with(BTreeMap::new)
                    .entry(ordered_float::OrderedFloat(price))
                    .or_insert_with(Vec::new)
                    .push(order);
            }
            OrderSide::Sell => {
                self.asks
                    .entry(instrument_id)
                    .or_insert_with(BTreeMap::new)
                    .entry(ordered_float::OrderedFloat(price))
                    .or_insert_with(Vec::new)
                    .push(order);
            }
        }
    }
    
    pub fn remove_order(
        &mut self,
        instrument_id: &str,
        side: OrderSide,
        price: f64,
        quantity: u32,
    ) -> bool {
        match side {
            OrderSide::Buy => {
                if let Some(instrument_bids) = self.bids.get_mut(instrument_id) {
                    if let Some(price_orders) = instrument_bids.get_mut(&ordered_float::OrderedFloat(price)) {
                        let mut remaining = quantity;
                        while remaining > 0 && !price_orders.is_empty() {
                            let order = price_orders.remove(0);
                            if order.quantity <= remaining {
                                remaining -= order.quantity;
                            } else {
                                price_orders.insert(0, BookOrder { quantity: order.quantity - remaining });
                                remaining = 0;
                            }
                        }
                        if price_orders.is_empty() {
                            instrument_bids.remove(&ordered_float::OrderedFloat(price));
                        }
                        return remaining == 0;
                    }
                }
            }
            OrderSide::Sell => {
                if let Some(instrument_asks) = self.asks.get_mut(instrument_id) {
                    if let Some(price_orders) = instrument_asks.get_mut(&ordered_float::OrderedFloat(price)) {
                        let mut remaining = quantity;
                        while remaining > 0 && !price_orders.is_empty() {
                            let order = price_orders.remove(0);
                            if order.quantity <= remaining {
                                remaining -= order.quantity;
                            } else {
                                price_orders.insert(0, BookOrder { quantity: order.quantity - remaining });
                                remaining = 0;
                            }
                        }
                        if price_orders.is_empty() {
                            instrument_asks.remove(&ordered_float::OrderedFloat(price));
                        }
                        return remaining == 0;
                    }
                }
            }
        }
        false
    }
    
    pub fn build_snapshot(
        &self,
        instrument_id: &str,
        sequence: u64,
    ) -> OrderBookSnapshot {
        let bids = self.build_levels(
            &self.bids.get(instrument_id),
            true,
        );
        
        let asks = self.build_levels(
            &self.asks.get(instrument_id),
            false,
        );
        
        OrderBookSnapshot {
            instrument_id: instrument_id.to_string(),
            bids,
            asks,
            sequence,
            timestamp: Utc::now(),
        }
    }
    
    fn build_levels(
        &self,
        levels: &Option<&BTreeMap<ordered_float::OrderedFloat<f64>, Vec<BookOrder>>>,
        descending: bool,
    ) -> Vec<PriceLevel> {
        let Some(levels) = levels else {
            return vec![];
        };
        
        let mut result: Vec<PriceLevel> = levels
            .iter()
            .map(|(price, orders)| {
                let quantity: u32 = orders.iter().map(|o| o.quantity).sum();
                PriceLevel {
                    price: price.0,
                    quantity,
                    order_count: orders.len(),
                }
            })
            .collect();
        
        if descending {
            result.reverse();
        }
        
        result
    }
    
    pub fn get_best_bid(&self, instrument_id: &str) -> Option<f64> {
        self.bids
            .get(instrument_id)
            .and_then(|bids| bids.keys().next())
            .map(|p| p.0)
    }
    
    pub fn get_best_ask(&self, instrument_id: &str) -> Option<f64> {
        self.asks
            .get(instrument_id)
            .and_then(|asks| asks.keys().next())
            .map(|p| p.0)
    }
    
    pub fn get_mid_price(&self, instrument_id: &str) -> Option<f64> {
        let best_bid = self.get_best_bid(instrument_id)?;
        let best_ask = self.get_best_ask(instrument_id)?;
        Some((best_bid + best_ask) / 2.0)
    }
    
    pub fn get_spread(&self, instrument_id: &str) -> Option<f64> {
        let best_bid = self.get_best_bid(instrument_id)?;
        let best_ask = self.get_best_ask(instrument_id)?;
        Some(best_ask - best_bid)
    }
    
    pub fn clear_instrument(&mut self, instrument_id: &str) {
        self.bids.remove(instrument_id);
        self.asks.remove(instrument_id);
    }
    
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }
    
    pub fn total_bids(&self, instrument_id: &str) -> usize {
        self.bids
            .get(instrument_id)
            .map(|bids| {
                bids.values().map(|orders| orders.len()).sum()
            })
            .unwrap_or(0)
    }
    
    pub fn total_asks(&self, instrument_id: &str) -> usize {
        self.asks
            .get(instrument_id)
            .map(|asks| {
                asks.values().map(|orders| orders.len()).sum()
            })
            .unwrap_or(0)
    }
}

impl Default for OrderBookBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_order_buy() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order(
            "BTC-20240315-50000-C".to_string(),
            OrderSide::Buy,
            50000.0,
            10,
        );
        
        let snapshot = builder.build_snapshot("BTC-20240315-50000-C", 1);
        
        assert_eq!(snapshot.bids.len(), 1);
        assert_eq!(snapshot.bids[0].price, 50000.0);
        assert_eq!(snapshot.bids[0].quantity, 10);
    }
    
    #[test]
    fn test_add_order_sell() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order(
            "BTC-20240315-50000-C".to_string(),
            OrderSide::Sell,
            51000.0,
            5,
        );
        
        let snapshot = builder.build_snapshot("BTC-20240315-50000-C", 1);
        
        assert_eq!(snapshot.asks.len(), 1);
        assert_eq!(snapshot.asks[0].price, 51000.0);
    }
    
    #[test]
    fn test_order_book_bids_descending() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Buy, 51000.0, 5);
        builder.add_order("BTC".to_string(), OrderSide::Buy, 49000.0, 15);
        
        let snapshot = builder.build_snapshot("BTC", 1);
        
        assert!(snapshot.bids[0].price >= snapshot.bids[1].price);
        assert!(snapshot.bids[1].price >= snapshot.bids[2].price);
    }
    
    #[test]
    fn test_order_book_asks_ascending() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Sell, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Sell, 49000.0, 5);
        builder.add_order("BTC".to_string(), OrderSide::Sell, 51000.0, 15);
        
        let snapshot = builder.build_snapshot("BTC", 1);
        
        assert!(snapshot.asks[0].price <= snapshot.asks[1].price);
        assert!(snapshot.asks[1].price <= snapshot.asks[2].price);
    }
    
    #[test]
    fn test_aggregate_same_price() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 15);
        
        let snapshot = builder.build_snapshot("BTC", 1);
        
        assert_eq!(snapshot.bids.len(), 1);
        assert_eq!(snapshot.bids[0].quantity, 25);
    }
    
    #[test]
    fn test_best_bid_ask() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Sell, 51000.0, 5);
        
        assert_eq!(builder.get_best_bid("BTC"), Some(50000.0));
        assert_eq!(builder.get_best_ask("BTC"), Some(51000.0));
    }
    
    #[test]
    fn test_mid_price() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Sell, 51000.0, 5);
        
        assert_eq!(builder.get_mid_price("BTC"), Some(50500.0));
    }
    
    #[test]
    fn test_spread() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("BTC".to_string(), OrderSide::Sell, 51000.0, 5);
        
        assert_eq!(builder.get_spread("BTC"), Some(1000.0));
    }
    
    #[test]
    fn test_remove_order() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        
        let removed = builder.remove_order("BTC", OrderSide::Buy, 50000.0, 5);
        
        assert!(removed);
        
        let snapshot = builder.build_snapshot("BTC", 1);
        assert_eq!(snapshot.bids[0].quantity, 5);
    }
    
    #[test]
    fn test_empty_book() {
        let builder = OrderBookBuilder::new();
        
        let snapshot = builder.build_snapshot("BTC", 1);
        
        assert!(snapshot.bids.is_empty());
        assert!(snapshot.asks.is_empty());
    }
    
    #[test]
    fn test_clear_instrument() {
        let mut builder = OrderBookBuilder::new();
        
        builder.add_order("BTC".to_string(), OrderSide::Buy, 50000.0, 10);
        builder.add_order("ETH".to_string(), OrderSide::Buy, 3000.0, 5);
        
        builder.clear_instrument("BTC");
        
        assert!(builder.get_best_bid("BTC").is_none());
        assert!(builder.get_best_bid("ETH").is_some());
    }
}
