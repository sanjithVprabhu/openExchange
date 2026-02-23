//! HTTP API handlers for the Matching Engine

use axum::{
    extract::{Path, State, Query},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::domain::{OrderBookSnapshot, Trade};
use crate::store::{MatchingStore, StoreError};
use crate::domain::BookOrder;
use crate::domain::OrderSide;
use crate::domain::TimeInForce;
use uuid::Uuid;

/// State for the matching API - uses Arc for Clone
pub struct MatchingApiState<S: ?Sized> {
    pub store: Arc<S>,
}

impl<S: MatchingStore + ?Sized> Clone for MatchingApiState<S> {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

/// Convenience type for dynamic dispatch
pub type DynMatchingApiState = MatchingApiState<dyn MatchingStore + Send + Sync>;

/// Request to submit an order
#[derive(Debug, Deserialize)]
pub struct SubmitOrderRequest {
    pub instrument_id: String,
    pub order_id: Option<Uuid>,
    pub user_id: Uuid,
    pub side: String,
    pub price: f64,
    pub quantity: u32,
    pub time_in_force: Option<String>,
}

/// Request to cancel an order
#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: Uuid,
}

/// Response for order submission
#[derive(Debug, serde::Serialize)]
pub struct SubmitOrderResponse {
    pub success: bool,
    pub trades: Vec<Trade>,
    pub remaining_quantity: u32,
    pub message: Option<String>,
}

/// Response for order book
#[derive(Debug, serde::Serialize)]
pub struct OrderBookResponse {
    pub success: bool,
    pub instrument_id: String,
    pub bids: Vec<PriceLevelResponse>,
    pub asks: Vec<PriceLevelResponse>,
    pub spread: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct PriceLevelResponse {
    pub price: f64,
    pub quantity: u32,
}

/// Response for trades
#[derive(Debug, serde::Serialize)]
pub struct TradesResponse {
    pub success: bool,
    pub instrument_id: String,
    pub trades: Vec<Trade>,
}

/// Submit an order to the matching engine
pub async fn submit_order<S: MatchingStore + 'static + ?Sized>(
    State(state): State<MatchingApiState<S>>,
    Json(req): Json<SubmitOrderRequest>,
) -> Json<SubmitOrderResponse> {
    let side = match req.side.to_lowercase().as_str() {
        "buy" => OrderSide::Buy,
        "sell" => OrderSide::Sell,
        _ => {
            return Json(SubmitOrderResponse {
                success: false,
                trades: vec![],
                remaining_quantity: 0,
                message: Some("Invalid side. Use 'buy' or 'sell'".to_string()),
            });
        }
    };

    let tif = match req.time_in_force.as_deref().unwrap_or("gtc").to_lowercase().as_str() {
        "ioc" => TimeInForce::Ioc,
        "fok" => TimeInForce::Fok,
        _ => TimeInForce::Gtc,
    };

    let order = BookOrder::new(
        req.order_id.unwrap_or_else(Uuid::new_v4),
        req.user_id,
        side,
        req.price,
        req.quantity,
        0, // Sequence will be assigned by store
        tif,
    ).with_instrument_id(req.instrument_id);

    match state.store.submit_order(order).await {
        Ok(result) => Json(SubmitOrderResponse {
            success: true,
            trades: result.trades,
            remaining_quantity: result.remaining_order
                .map(|o| o.quantity)
                .unwrap_or(0),
            message: None,
        }),
        Err(e) => Json(SubmitOrderResponse {
            success: false,
            trades: vec![],
            remaining_quantity: 0,
            message: Some(e.to_string()),
        }),
    }
}

/// Cancel an order
pub async fn cancel_order<S: MatchingStore + 'static + ?Sized>(
    State(state): State<MatchingApiState<S>>,
    Path((instrument_id, order_id)): Path<(String, Uuid)>,
) -> Json<serde_json::Value> {
    match state.store.cancel_order(&instrument_id, order_id).await {
        Ok(Some(_)) => Json(serde_json::json!({
            "success": true,
            "message": "Order cancelled"
        })),
        Ok(None) => Json(serde_json::json!({
            "success": false,
            "message": "Order not found"
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": e.to_string()
        })),
    }
}

/// Get order book for an instrument
pub async fn get_order_book<S: MatchingStore + 'static + ?Sized>(
    State(state): State<MatchingApiState<S>>,
    Path(instrument_id): Path<String>,
) -> Json<OrderBookResponse> {
    match state.store.get_book(&instrument_id).await {
        Ok(Some(book)) => {
            let bids: Vec<PriceLevelResponse> = book.bids
                .iter()
                .take(10)
                .map(|(price, orders)| PriceLevelResponse {
                    price: price.0 .0,
                    quantity: orders.iter().map(|o| o.quantity).sum(),
                })
                .collect();

            let asks: Vec<PriceLevelResponse> = book.asks
                .iter()
                .take(10)
                .map(|(price, orders)| PriceLevelResponse {
                    price: price.0,
                    quantity: orders.iter().map(|o| o.quantity).sum(),
                })
                .collect();

            Json(OrderBookResponse {
                success: true,
                instrument_id,
                bids,
                asks,
                spread: book.spread(),
            })
        }
        Ok(None) => Json(OrderBookResponse {
            success: false,
            instrument_id,
            bids: vec![],
            asks: vec![],
            spread: None,
        }),
        Err(e) => Json(OrderBookResponse {
            success: false,
            instrument_id,
            bids: vec![],
            asks: vec![],
            spread: None,
        }),
    }
}

/// Get recent trades
pub async fn get_trades<S: MatchingStore + 'static + ?Sized>(
    State(state): State<MatchingApiState<S>>,
    Path(instrument_id): Path<String>,
    Query(params): Query<serde_json::Value>,
) -> Json<TradesResponse> {
    let limit = params.get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as u32;

    match state.store.get_trades(&instrument_id, limit).await {
        Ok(trades) => Json(TradesResponse {
            success: true,
            instrument_id,
            trades,
        }),
        Err(e) => Json(TradesResponse {
            success: false,
            instrument_id,
            trades: vec![],
        }),
    }
}

/// Health check
pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "matching-engine"
    }))
}
