use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::engine::{InstrumentInfo, RiskEngine};
use crate::types::RiskCheckResult;

pub struct DirectRiskClient {
    engine: Arc<RwLock<RiskEngine>>,
}

impl DirectRiskClient {
    pub fn new(engine: Arc<RwLock<RiskEngine>>) -> Self {
        Self { engine }
    }

    pub async fn check_order(
        &self,
        user_id: Uuid,
        order_side: &str,
        instrument_id: &str,
        quantity: u32,
        price: f64,
    ) -> RiskCheckResult {
        let engine = self.engine.read().await;
        engine.check_order(user_id, order_side, instrument_id, quantity, price)
    }

    pub async fn reserve_margin(&self, user_id: Uuid, amount: f64) {
        let mut engine = self.engine.write().await;
        engine.reserve_margin(user_id, amount);
    }

    pub async fn release_margin(&self, user_id: Uuid, amount: f64) {
        let mut engine = self.engine.write().await;
        engine.release_margin(user_id, amount);
    }

    pub async fn update_position(
        &self,
        user_id: Uuid,
        instrument_id: String,
        side: crate::types::PositionSide,
        quantity: u32,
        price: f64,
    ) {
        let mut engine = self.engine.write().await;
        engine.update_position(user_id, instrument_id, side, quantity, price);
    }

    pub async fn update_wallet_balance(&self, user_id: Uuid, balance: f64) {
        let mut engine = self.engine.write().await;
        engine.update_wallet_balance(user_id, balance);
    }

    pub async fn register_instrument(&self, instrument_id: String, info: InstrumentInfo) {
        let mut engine = self.engine.write().await;
        engine.register_instrument(instrument_id, info);
    }

    pub async fn update_price(&self, instrument_id: String, price: f64) {
        let mut engine = self.engine.write().await;
        engine.update_price(instrument_id, price);
    }

    pub async fn check_liquidation(&self, user_id: Uuid) -> bool {
        let engine = self.engine.read().await;
        engine.check_liquidation(user_id)
    }

    pub fn engine(&self) -> Arc<RwLock<RiskEngine>> {
        Arc::clone(&self.engine)
    }
}
