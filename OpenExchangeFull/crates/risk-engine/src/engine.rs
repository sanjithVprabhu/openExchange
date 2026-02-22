use crate::calculator::MarginCalculator;
use crate::types::{MarginConfig, MarginRequirement, Position, PositionSide, RiskCheckResult, UserRiskState};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct RiskEngine {
    user_states: HashMap<Uuid, UserRiskState>,
    margin_calc: MarginCalculator,
    config: MarginConfig,
    current_prices: HashMap<String, f64>,
    instrument_info: HashMap<String, InstrumentInfo>,
}

#[derive(Clone)]
pub struct InstrumentInfo {
    pub strike_price: f64,
    pub contract_size: f64,
    pub is_call: bool,
}

impl RiskEngine {
    pub fn new(config: MarginConfig) -> Self {
        let margin_calc = MarginCalculator::new(config.clone());

        Self {
            user_states: HashMap::new(),
            margin_calc,
            config,
            current_prices: HashMap::new(),
            instrument_info: HashMap::new(),
        }
    }

    fn get_or_create_user_state(&mut self, user_id: Uuid) -> &mut UserRiskState {
        self.user_states
            .entry(user_id)
            .or_insert_with(|| UserRiskState::new(user_id, 0.0))
    }

    pub fn register_instrument(&mut self, instrument_id: String, info: InstrumentInfo) {
        self.instrument_info.insert(instrument_id, info);
    }

    pub fn update_price(&mut self, instrument_id: String, price: f64) {
        self.current_prices.insert(instrument_id, price);
    }

    pub fn update_wallet_balance(&mut self, user_id: Uuid, balance: f64) {
        let state = self.get_or_create_user_state(user_id);
        state.wallet_balance = balance;
        state.updated_at = chrono::Utc::now();
    }

    pub fn check_order(
        &self,
        user_id: Uuid,
        order_side: &str,
        instrument_id: &str,
        quantity: u32,
        price: f64,
    ) -> RiskCheckResult {
        info!(
            user_id = %user_id,
            instrument = instrument_id,
            "Checking order risk"
        );

        let user_state = match self.user_states.get(&user_id) {
            Some(state) => state,
            None => {
                return RiskCheckResult::rejected(
                    "User not found".to_string(),
                    0.0,
                    0.0,
                );
            }
        };

        let instrument_info = match self.instrument_info.get(instrument_id) {
            Some(info) => info,
            None => {
                return RiskCheckResult::rejected(
                    format!("Instrument not found: {}", instrument_id),
                    0.0,
                    user_state.free_margin(),
                );
            }
        };

        let current_price = self.current_prices
            .get(instrument_id)
            .copied()
            .unwrap_or(price);

        let position_side = if order_side == "buy" {
            PositionSide::Long
        } else {
            PositionSide::Short
        };

        let required_margin = self.margin_calc.calculate_order_margin(
            order_side,
            position_side,
            quantity,
            price,
            instrument_info.strike_price,
            instrument_info.contract_size,
            current_price,
            instrument_info.is_call,
        );

        let free_margin = user_state.free_margin();

        if required_margin.initial_margin > free_margin {
            warn!(
                user_id = %user_id,
                required = required_margin.initial_margin,
                free = free_margin,
                "Insufficient margin"
            );

            return RiskCheckResult::rejected(
                format!(
                    "Insufficient margin: required {}, available {}",
                    required_margin.initial_margin, free_margin
                ),
                required_margin.initial_margin,
                free_margin,
            );
        }

        let current_position = user_state
            .positions
            .get(instrument_id)
            .map(|p| p.quantity)
            .unwrap_or(0);

        let new_position_size = current_position + quantity;

        if new_position_size > self.config.max_position_size {
            return RiskCheckResult::rejected(
                format!(
                    "Position size limit exceeded: {} > {}",
                    new_position_size, self.config.max_position_size
                ),
                required_margin.initial_margin,
                free_margin,
            );
        }

        if user_state.positions.len() >= self.config.max_open_positions {
            return RiskCheckResult::rejected(
                format!(
                    "Max open positions exceeded: {}",
                    self.config.max_open_positions
                ),
                required_margin.initial_margin,
                free_margin,
            );
        }

        let projected_free = free_margin - required_margin.initial_margin;

        info!(
            user_id = %user_id,
            required = required_margin.initial_margin,
            free = free_margin,
            projected_free = projected_free,
            "Order approved"
        );

        RiskCheckResult::approved(
            required_margin.initial_margin,
            free_margin,
            projected_free,
        )
    }

    pub fn reserve_margin(&mut self, user_id: Uuid, amount: f64) {
        let state = self.get_or_create_user_state(user_id);
        state.reserved_margin += amount;
        state.updated_at = chrono::Utc::now();
    }

    pub fn release_margin(&mut self, user_id: Uuid, amount: f64) {
        if let Some(state) = self.user_states.get_mut(&user_id) {
            state.reserved_margin = (state.reserved_margin - amount).max(0.0);
            state.updated_at = chrono::Utc::now();
        }
    }

    pub fn update_position(
        &mut self,
        user_id: Uuid,
        instrument_id: String,
        side: PositionSide,
        quantity: u32,
        price: f64,
    ) {
        let state = self.get_or_create_user_state(user_id);

        match state.positions.get_mut(&instrument_id) {
            Some(position) => {
                position.update_fill(quantity, price);
            }
            None => {
                let position = Position::new(
                    user_id,
                    instrument_id.clone(),
                    side,
                    quantity,
                    price,
                );
                state.positions.insert(instrument_id, position);
            }
        }

        state.updated_at = chrono::Utc::now();
    }

    pub fn close_position(&mut self, user_id: Uuid, instrument_id: &str, quantity: u32) {
        if let Some(state) = self.user_states.get_mut(&user_id) {
            if let Some(position) = state.positions.get_mut(instrument_id) {
                position.reduce(quantity);
                if position.is_closed() {
                    state.positions.remove(instrument_id);
                }
            }
            state.updated_at = chrono::Utc::now();
        }
    }

    pub fn recalculate_portfolio(&mut self, user_id: Uuid) {
        let state = match self.user_states.get_mut(&user_id) {
            Some(s) => s,
            None => return,
        };

        let mut total_initial = 0.0;
        let mut total_maintenance = 0.0;

        for (instrument_id, position) in &state.positions {
            if position.quantity == 0 {
                continue;
            }

            let instrument_info = match self.instrument_info.get(instrument_id) {
                Some(info) => info,
                None => continue,
            };

            let current_price = self.current_prices
                .get(instrument_id)
                .copied()
                .unwrap_or(position.avg_price);

            let margin = self.margin_calc.calculate_position_margin(
                position,
                instrument_info.strike_price,
                instrument_info.contract_size,
                current_price,
                instrument_info.is_call,
            );

            total_initial += margin.initial_margin;
            total_maintenance += margin.maintenance_margin;
        }

        state.total_initial_margin = total_initial;
        state.total_maintenance_margin = total_maintenance;
        state.updated_at = chrono::Utc::now();
    }

    pub fn check_liquidation(&self, user_id: Uuid) -> bool {
        self.user_states
            .get(&user_id)
            .map(|state| state.is_liquidatable())
            .unwrap_or(false)
    }

    pub fn get_user_state(&self, user_id: Uuid) -> Option<&UserRiskState> {
        self.user_states.get(&user_id)
    }

    pub fn get_user_positions(&self, user_id: Uuid) -> Vec<&Position> {
        self.user_states
            .get(&user_id)
            .map(|state| state.positions.values().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> RiskEngine {
        RiskEngine::new(MarginConfig::default())
    }

    #[test]
    fn test_order_approval_sufficient_margin() {
        let mut engine = create_test_engine();
        let user_id = Uuid::new_v4();

        engine.register_instrument(
            "BTC-50000-C".to_string(),
            InstrumentInfo {
                strike_price: 50000.0,
                contract_size: 0.01,
                is_call: true,
            },
        );

        engine.update_wallet_balance(user_id, 10000.0);

        let result = engine.check_order(user_id, "buy", "BTC-50000-C", 10, 100.0);

        assert!(result.approved);
    }

    #[test]
    fn test_order_rejection_insufficient_margin() {
        let mut engine = create_test_engine();
        let user_id = Uuid::new_v4();

        engine.register_instrument(
            "BTC-50000-C".to_string(),
            InstrumentInfo {
                strike_price: 50000.0,
                contract_size: 0.01,
                is_call: true,
            },
        );

        engine.update_wallet_balance(user_id, 100.0);

        let result = engine.check_order(user_id, "buy", "BTC-50000-C", 10, 100.0);

        assert!(!result.approved);
        assert!(result.reason.is_some());
    }

    #[test]
    fn test_margin_reservation() {
        let mut engine = create_test_engine();
        let user_id = Uuid::new_v4();

        engine.update_wallet_balance(user_id, 10000.0);

        engine.reserve_margin(user_id, 1000.0);

        let state = engine.get_user_state(user_id).unwrap();
        assert_eq!(state.reserved_margin, 1000.0);
        assert_eq!(state.free_margin(), 9000.0);

        engine.release_margin(user_id, 1000.0);

        let state = engine.get_user_state(user_id).unwrap();
        assert_eq!(state.reserved_margin, 0.0);
    }
}
