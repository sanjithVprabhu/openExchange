//! API models for OMS HTTP endpoints

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use common::types::{Side, OrderType, TimeInForce};
use crate::types::{OrderStatus, Order};

/// Request to create a new order
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub instrument_id: String,
    pub side: Side,
    pub order_type: OrderType,
    #[serde(default = "default_time_in_force")]
    pub time_in_force: TimeInForce,
    pub price: Option<f64>,
    pub quantity: u32,
    #[serde(default)]
    pub client_order_id: Option<String>,
}

fn default_time_in_force() -> TimeInForce {
    TimeInForce::Gtc
}

/// Response after creating an order
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub success: bool,
    pub order: Option<OrderResponse>,
    #[serde(default)]
    pub error: Option<ErrorDetail>,
}

impl CreateOrderResponse {
    pub fn success(order: OrderResponse) -> Self {
        Self {
            success: true,
            order: Some(order),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            order: None,
            error: Some(ErrorDetail {
                code: code.into(),
                message: message.into(),
                details: None,
            }),
        }
    }
}

/// Single order in API response
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: Uuid,
    pub user_id: Uuid,
    pub instrument_id: String,
    pub side: Side,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub price: Option<f64>,
    pub quantity: u32,
    pub filled_quantity: u32,
    pub remaining_quantity: u32,
    pub avg_fill_price: Option<f64>,
    pub status: OrderStatus,
    #[serde(default)]
    pub client_order_id: Option<String>,
    #[serde(default)]
    pub risk_approved_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub risk_rejection_reason: Option<String>,
    #[serde(default)]
    pub required_margin: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<Order> for OrderResponse {
    fn from(order: Order) -> Self {
        Self {
            order_id: order.order_id,
            user_id: order.user_id,
            instrument_id: order.instrument_id,
            side: order.side,
            order_type: order.order_type,
            time_in_force: order.time_in_force,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            remaining_quantity: order.quantity - order.filled_quantity,
            avg_fill_price: order.avg_fill_price,
            status: order.status,
            client_order_id: order.client_order_id,
            risk_approved_at: order.risk_approved_at,
            risk_rejection_reason: order.risk_rejection_reason,
            required_margin: order.required_margin,
            created_at: order.created_at,
            updated_at: order.updated_at,
        }
    }
}

/// List orders request parameters
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ListOrdersParams {
    #[serde(default)]
    pub instrument_id: Option<String>,
    #[serde(default)]
    pub side: Option<Side>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// List orders response
#[derive(Debug, Serialize, Deserialize)]
pub struct ListOrdersResponse {
    pub success: bool,
    pub total_count: u64,
    pub returned_count: u32,
    pub offset: u32,
    pub orders: Vec<OrderResponse>,
}

/// Fill record in response
#[derive(Debug, Serialize, Deserialize)]
pub struct FillResponse {
    pub fill_id: Uuid,
    pub trade_id: Uuid,
    pub quantity: u32,
    pub price: f64,
    pub fee: f64,
    pub fee_currency: String,
    pub is_maker: bool,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

impl From<crate::types::OrderFill> for FillResponse {
    fn from(fill: crate::types::OrderFill) -> Self {
        Self {
            fill_id: fill.fill_id,
            trade_id: fill.trade_id,
            quantity: fill.quantity,
            price: fill.price,
            fee: fill.fee,
            fee_currency: fill.fee_currency,
            is_maker: fill.is_maker,
            executed_at: fill.executed_at,
        }
    }
}

/// Get fills response
#[derive(Debug, Serialize, Deserialize)]
pub struct GetFillsResponse {
    pub success: bool,
    pub order_id: Uuid,
    pub total_fills: u32,
    pub fills: Vec<FillResponse>,
}

/// Cancel order response
#[derive(Debug, Serialize, Deserialize)]
pub struct CancelOrderResponse {
    pub success: bool,
    pub order: Option<OrderResponse>,
    #[serde(default)]
    pub error: Option<ErrorDetail>,
}

/// Error detail
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<serde_json::Value>,
}

/// Generic error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}
