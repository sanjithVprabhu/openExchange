//! Order Management System domain types
//!
//! This module defines the core domain types for the OMS.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::types::{Side, OrderType as CommonOrderType, TimeInForce as CommonTimeInForce};

/// Order status in the OMS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    /// Order created, waiting for risk check
    PendingRisk,
    /// Risk approved, order is in the book
    Open,
    /// Order partially filled
    PartiallyFilled,
    /// Order fully filled
    Filled,
    /// Order cancelled by user
    Cancelled,
    /// Order rejected by risk engine
    Rejected,
    /// Order expired (DAY/IOC/FOK)
    Expired,
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::PendingRisk
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderStatus::PendingRisk => write!(f, "pending_risk"),
            OrderStatus::Open => write!(f, "open"),
            OrderStatus::PartiallyFilled => write!(f, "partially_filled"),
            OrderStatus::Filled => write!(f, "filled"),
            OrderStatus::Cancelled => write!(f, "cancelled"),
            OrderStatus::Rejected => write!(f, "rejected"),
            OrderStatus::Expired => write!(f, "expired"),
        }
    }
}

/// Order in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Instrument being traded
    pub instrument_id: String,
    /// Buy or sell
    pub side: Side,
    /// Order type (limit/market)
    pub order_type: CommonOrderType,
    /// Time in force
    pub time_in_force: CommonTimeInForce,
    /// Limit price (NULL for market orders)
    pub price: Option<f64>,
    /// Total quantity of contracts
    pub quantity: u32,
    /// Filled quantity
    pub filled_quantity: u32,
    /// Average fill price
    pub avg_fill_price: Option<f64>,
    /// Current order status
    pub status: OrderStatus,
    /// Client-specified order ID
    pub client_order_id: Option<String>,
    /// When risk was approved
    pub risk_approved_at: Option<DateTime<Utc>>,
    /// Reason for rejection (if rejected)
    pub risk_rejection_reason: Option<String>,
    /// Required margin for this order
    pub required_margin: Option<f64>,
    /// Order creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Order {
    /// Create a new order
    pub fn new(
        user_id: Uuid,
        instrument_id: String,
        side: Side,
        order_type: CommonOrderType,
        time_in_force: CommonTimeInForce,
        price: Option<f64>,
        quantity: u32,
    ) -> Self {
        let now = Utc::now();
        Self {
            order_id: Uuid::new_v4(),
            user_id,
            instrument_id,
            side,
            order_type,
            time_in_force,
            price,
            quantity,
            filled_quantity: 0,
            avg_fill_price: None,
            status: OrderStatus::PendingRisk,
            client_order_id: None,
            risk_approved_at: None,
            risk_rejection_reason: None,
            required_margin: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Apply a fill to this order
    pub fn apply_fill(&mut self, fill_quantity: u32, fill_price: f64) {
        let new_filled = self.filled_quantity + fill_quantity;
        
        // Update average fill price
        let total_value = (self.avg_fill_price.unwrap_or(0.0) * self.filled_quantity as f64)
            + (fill_price * fill_quantity as f64);
        self.avg_fill_price = Some(total_value / new_filled as f64);
        
        self.filled_quantity = new_filled;
        self.updated_at = Utc::now();

        // Update status
        if self.filled_quantity >= self.quantity {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > 0 {
            self.status = OrderStatus::PartiallyFilled;
        }
    }

    /// Check if order can be cancelled
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::PendingRisk | OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }

    /// Check if order is active (can receive fills)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Open | OrderStatus::PartiallyFilled
        )
    }

    /// Get remaining quantity to fill
    pub fn remaining_quantity(&self) -> u32 {
        self.quantity.saturating_sub(self.filled_quantity)
    }
}

/// Fill record for an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    /// Unique fill identifier
    pub fill_id: Uuid,
    /// Order this fill belongs to
    pub order_id: Uuid,
    /// Trade identifier from matching engine
    pub trade_id: Uuid,
    /// Quantity filled
    pub quantity: u32,
    /// Fill price
    pub price: f64,
    /// Counterparty order ID
    pub counterparty_order_id: Option<Uuid>,
    /// Fee charged
    pub fee: f64,
    /// Fee currency
    pub fee_currency: String,
    /// Whether this order was the maker
    pub is_maker: bool,
    /// Execution timestamp
    pub executed_at: DateTime<Utc>,
    /// Fill record creation timestamp
    pub created_at: DateTime<Utc>,
}

impl OrderFill {
    /// Create a new fill record
    pub fn new(
        order_id: Uuid,
        trade_id: Uuid,
        quantity: u32,
        price: f64,
        is_maker: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            fill_id: Uuid::new_v4(),
            order_id,
            trade_id,
            quantity,
            price,
            counterparty_order_id: None,
            fee: 0.0,
            fee_currency: "USDT".to_string(),
            is_maker,
            executed_at: now,
            created_at: now,
        }
    }
}

/// Environment for order isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Prod,
    Virtual,
    Static,
}

impl Environment {
    /// Get table suffix for this environment
    pub fn table_suffix(&self) -> &'static str {
        match self {
            Environment::Prod => "prod",
            Environment::Virtual => "virtual",
            Environment::Static => "static",
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Prod => write!(f, "prod"),
            Environment::Virtual => write!(f, "virtual"),
            Environment::Static => write!(f, "static"),
        }
    }
}

impl From<&str> for Environment {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "prod" | "production" => Environment::Prod,
            "virtual" | "paper" => Environment::Virtual,
            "static" | "test" => Environment::Static,
            _ => Environment::Static,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_new() {
        let order = Order::new(
            Uuid::new_v4(),
            "BTC-20260315-50000-C".to_string(),
            Side::Buy,
            CommonOrderType::Limit,
            CommonTimeInForce::Gtc,
            Some(150.0),
            10,
        );

        assert_eq!(order.status, OrderStatus::PendingRisk);
        assert_eq!(order.filled_quantity, 0);
    }

    #[test]
    fn test_order_apply_fill() {
        let mut order = Order::new(
            Uuid::new_v4(),
            "BTC-20260315-50000-C".to_string(),
            Side::Buy,
            CommonOrderType::Limit,
            CommonTimeInForce::Gtc,
            Some(150.0),
            10,
        );

        order.apply_fill(4, 150.0);
        
        assert_eq!(order.filled_quantity, 4);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.avg_fill_price, Some(150.0));

        order.apply_fill(6, 151.0);
        
        assert_eq!(order.filled_quantity, 10);
        assert_eq!(order.status, OrderStatus::Filled);
    }

    #[test]
    fn test_order_can_cancel() {
        let mut order = Order::new(
            Uuid::new_v4(),
            "BTC-20260315-50000-C".to_string(),
            Side::Buy,
            CommonOrderType::Limit,
            CommonTimeInForce::Gtc,
            Some(150.0),
            10,
        );

        assert!(order.can_cancel()); // PendingRisk

        order.status = OrderStatus::Open;
        assert!(order.can_cancel());

        order.status = OrderStatus::PartiallyFilled;
        assert!(order.can_cancel());

        order.status = OrderStatus::Filled;
        assert!(!order.can_cancel());

        order.status = OrderStatus::Cancelled;
        assert!(!order.can_cancel());
    }

    #[test]
    fn test_environment_from_str() {
        assert_eq!(Environment::from("prod"), Environment::Prod);
        assert_eq!(Environment::from("PROD"), Environment::Prod);
        assert_eq!(Environment::from("virtual"), Environment::Virtual);
        assert_eq!(Environment::from("static"), Environment::Static);
    }
}
