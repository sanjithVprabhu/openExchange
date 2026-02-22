use crate::types::{MarginConfig, MarginRequirement, Position, PositionSide};

pub struct MarginCalculator {
    config: MarginConfig,
}

impl MarginCalculator {
    pub fn new(config: MarginConfig) -> Self {
        Self { config }
    }

    pub fn calculate_position_margin(
        &self,
        position: &Position,
        strike_price: f64,
        contract_size: f64,
        current_price: f64,
        is_call: bool,
    ) -> MarginRequirement {
        match position.side {
            PositionSide::Long => {
                MarginRequirement::zero()
            }
            PositionSide::Short => {
                self.calculate_short_margin(
                    position.quantity,
                    strike_price,
                    contract_size,
                    current_price,
                    is_call,
                )
            }
        }
    }

    fn calculate_short_margin(
        &self,
        quantity: u32,
        strike: f64,
        contract_size: f64,
        current_price: f64,
        is_call: bool,
    ) -> MarginRequirement {
        let quantity_f = quantity as f64;

        let initial = if is_call {
            let alpha = self.config.short_call_stress_multiplier;
            let stress_margin = alpha * current_price;
            let itm_margin = if current_price > strike {
                current_price - strike
            } else {
                0.0
            };
            quantity_f * contract_size * stress_margin.max(itm_margin)
        } else {
            quantity_f * contract_size * strike
        };

        let maintenance = initial * self.config.maintenance_ratio;

        MarginRequirement::new(initial, maintenance)
    }

    pub fn calculate_order_margin(
        &self,
        order_side: &str,
        position_side: PositionSide,
        quantity: u32,
        price: f64,
        strike_price: f64,
        contract_size: f64,
        current_price: f64,
        is_call: bool,
    ) -> MarginRequirement {
        match (order_side, position_side) {
            ("buy", PositionSide::Long) => {
                let premium = price * quantity as f64;
                MarginRequirement::new(premium, 0.0)
            }
            ("sell", PositionSide::Short) => {
                self.calculate_short_margin(
                    quantity,
                    strike_price,
                    contract_size,
                    current_price,
                    is_call,
                )
            }
            ("buy", PositionSide::Short) => {
                MarginRequirement::zero()
            }
            ("sell", PositionSide::Long) => {
                MarginRequirement::zero()
            }
            _ => MarginRequirement::zero(),
        }
    }

    pub fn calculate_portfolio_margin(
        &self,
        positions: &[(String, Position, f64, f64, bool)],
    ) -> MarginRequirement {
        let mut total_initial = 0.0;
        let mut total_maintenance = 0.0;

        for (_, position, strike, current_price, is_call) in positions {
            if position.quantity == 0 {
                continue;
            }

            let margin = self.calculate_position_margin(
                position,
                *strike,
                0.01,
                *current_price,
                *is_call,
            );

            total_initial += margin.initial_margin;
            total_maintenance += margin.maintenance_margin;
        }

        MarginRequirement::new(total_initial, total_maintenance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_long_call_margin() {
        let calc = MarginCalculator::new(MarginConfig::default());
        let position = Position::new(
            Uuid::new_v4(),
            "BTC-50000-C".to_string(),
            PositionSide::Long,
            10,
            100.0,
        );

        let margin = calc.calculate_position_margin(&position, 50000.0, 0.01, 50000.0, true);

        assert_eq!(margin.initial_margin, 0.0);
        assert_eq!(margin.maintenance_margin, 0.0);
    }

    #[test]
    fn test_short_call_margin() {
        let calc = MarginCalculator::new(MarginConfig::default());
        let position = Position::new(
            Uuid::new_v4(),
            "BTC-50000-C".to_string(),
            PositionSide::Short,
            10,
            100.0,
        );

        let margin = calc.calculate_position_margin(&position, 50000.0, 0.01, 50000.0, true);

        let expected = 10.0 * 0.01 * (0.15 * 50000.0);
        assert_eq!(margin.initial_margin, expected);
        assert_eq!(margin.maintenance_margin, expected * 0.75);
    }

    #[test]
    fn test_short_put_margin() {
        let calc = MarginCalculator::new(MarginConfig::default());
        let position = Position::new(
            Uuid::new_v4(),
            "BTC-40000-P".to_string(),
            PositionSide::Short,
            10,
            100.0,
        );

        let margin = calc.calculate_position_margin(&position, 40000.0, 0.01, 50000.0, false);

        let expected = 10.0 * 0.01 * 40000.0;
        assert_eq!(margin.initial_margin, expected);
        assert_eq!(margin.maintenance_margin, expected * 0.75);
    }
}
