use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub user_id: Uuid,
    pub instrument_id: String,
    pub side: PositionSide,
    pub quantity: u32,
    pub avg_price: f64,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Position {
    pub fn new(
        user_id: Uuid,
        instrument_id: String,
        side: PositionSide,
        quantity: u32,
        price: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            user_id,
            instrument_id,
            side,
            quantity,
            avg_price: price,
            opened_at: now,
            updated_at: now,
        }
    }

    pub fn update_fill(&mut self, fill_quantity: u32, fill_price: f64) {
        let total_value = self.avg_price * self.quantity as f64
            + fill_price * fill_quantity as f64;
        self.quantity += fill_quantity;
        self.avg_price = total_value / self.quantity as f64;
        self.updated_at = Utc::now();
    }

    pub fn reduce(&mut self, quantity: u32) {
        self.quantity = self.quantity.saturating_sub(quantity);
        self.updated_at = Utc::now();
    }

    pub fn is_closed(&self) -> bool {
        self.quantity == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarginRequirement {
    pub initial_margin: f64,
    pub maintenance_margin: f64,
}

impl MarginRequirement {
    pub fn new(initial_margin: f64, maintenance_margin: f64) -> Self {
        Self {
            initial_margin,
            maintenance_margin,
        }
    }

    pub fn zero() -> Self {
        Self {
            initial_margin: 0.0,
            maintenance_margin: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRiskState {
    pub user_id: Uuid,
    pub wallet_balance: f64,
    pub positions: HashMap<String, Position>,
    pub reserved_margin: f64,
    pub total_initial_margin: f64,
    pub total_maintenance_margin: f64,
    pub unrealized_pnl: f64,
    pub updated_at: DateTime<Utc>,
}

impl UserRiskState {
    pub fn new(user_id: Uuid, wallet_balance: f64) -> Self {
        Self {
            user_id,
            wallet_balance,
            positions: HashMap::new(),
            reserved_margin: 0.0,
            total_initial_margin: 0.0,
            total_maintenance_margin: 0.0,
            unrealized_pnl: 0.0,
            updated_at: Utc::now(),
        }
    }

    pub fn equity(&self) -> f64 {
        self.wallet_balance + self.unrealized_pnl
    }

    pub fn free_margin(&self) -> f64 {
        self.equity() - self.total_initial_margin - self.reserved_margin
    }

    pub fn is_liquidatable(&self) -> bool {
        self.equity() < self.total_maintenance_margin
    }

    pub fn margin_usage(&self) -> f64 {
        if self.total_initial_margin == 0.0 {
            0.0
        } else {
            (self.total_initial_margin + self.reserved_margin) / self.equity()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginConfig {
    pub short_call_stress_multiplier: f64,
    pub maintenance_ratio: f64,
    pub max_position_size: u32,
    pub max_total_notional: f64,
    pub max_open_positions: usize,
}

impl Default for MarginConfig {
    fn default() -> Self {
        Self {
            short_call_stress_multiplier: 0.15,
            maintenance_ratio: 0.75,
            max_position_size: 10000,
            max_total_notional: 1_000_000.0,
            max_open_positions: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheckResult {
    pub approved: bool,
    pub reason: Option<String>,
    pub required_margin: f64,
    pub free_margin: f64,
    pub projected_free_margin: f64,
    pub margin_lock_id: Option<String>,
}

impl RiskCheckResult {
    pub fn approved(required_margin: f64, free_margin: f64, projected_free: f64) -> Self {
        Self {
            approved: true,
            reason: None,
            required_margin,
            free_margin,
            projected_free_margin: projected_free,
            margin_lock_id: Some(Uuid::new_v4().to_string()),
        }
    }

    pub fn rejected(reason: String, required: f64, free: f64) -> Self {
        Self {
            approved: false,
            reason: Some(reason),
            required_margin: required,
            free_margin: free,
            projected_free_margin: free,
            margin_lock_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LiquidationState {
    Healthy,
    Liquidatable,
    Liquidating,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginLock {
    pub lock_id: String,
    pub user_id: Uuid,
    pub order_id: Uuid,
    pub amount: f64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
}
