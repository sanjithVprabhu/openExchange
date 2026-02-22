use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::engine::{InstrumentInfo, RiskEngine};
use crate::types::RiskCheckResult;

#[derive(Clone)]
pub struct RiskApiState {
    pub engine: Arc<tokio::sync::RwLock<RiskEngine>>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

#[derive(Debug, Deserialize)]
pub struct RiskCheckRequest {
    pub user_id: String,
    pub order_side: String,
    pub instrument_id: String,
    pub quantity: u32,
    pub price: f64,
}

#[derive(Debug, Serialize)]
pub struct RiskCheckResponse {
    pub approved: bool,
    pub reason: Option<String>,
    pub required_margin: f64,
    pub free_margin: f64,
    pub projected_free_margin: f64,
    pub margin_lock_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseMarginRequest {
    pub user_id: String,
    pub margin_lock_id: String,
    pub amount: f64,
}

#[derive(Debug, Serialize)]
pub struct MarginInfoResponse {
    pub user_id: String,
    pub wallet_balance: f64,
    pub reserved_margin: f64,
    pub total_initial_margin: f64,
    pub total_maintenance_margin: f64,
    pub free_margin: f64,
    pub equity: f64,
    pub is_liquidatable: bool,
}

#[derive(Debug, Serialize)]
pub struct PositionResponse {
    pub instrument_id: String,
    pub side: String,
    pub quantity: u32,
    pub avg_price: f64,
}

pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "risk".to_string(),
    })
}

pub async fn check_risk(
    State(state): State<Arc<RiskApiState>>,
    Json(req): Json<RiskCheckRequest>,
) -> Result<Json<RiskCheckResponse>, String> {
    let user_id = Uuid::parse_str(&req.user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let engine = state.engine.read().await;
    let result = engine.check_order(
        user_id,
        &req.order_side,
        &req.instrument_id,
        req.quantity,
        req.price,
    );

    Ok(Json(RiskCheckResponse {
        approved: result.approved,
        reason: result.reason,
        required_margin: result.required_margin,
        free_margin: result.free_margin,
        projected_free_margin: result.projected_free_margin,
        margin_lock_id: result.margin_lock_id,
    }))
}

pub async fn release_margin(
    State(state): State<Arc<RiskApiState>>,
    Json(req): Json<ReleaseMarginRequest>,
) -> Result<Json<serde_json::Value>, String> {
    let user_id = Uuid::parse_str(&req.user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let mut engine = state.engine.write().await;
    engine.release_margin(user_id, req.amount);

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Margin released"
    })))
}

pub async fn get_margin_info(
    State(state): State<Arc<RiskApiState>>,
    Path(user_id): Path<String>,
) -> Result<Json<MarginInfoResponse>, String> {
    let user_id = Uuid::parse_str(&user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let engine = state.engine.read().await;
    let user_state = engine.get_user_state(user_id)
        .ok_or("User not found")?;

    Ok(Json(MarginInfoResponse {
        user_id: user_id.to_string(),
        wallet_balance: user_state.wallet_balance,
        reserved_margin: user_state.reserved_margin,
        total_initial_margin: user_state.total_initial_margin,
        total_maintenance_margin: user_state.total_maintenance_margin,
        free_margin: user_state.free_margin(),
        equity: user_state.equity(),
        is_liquidatable: user_state.is_liquidatable(),
    }))
}

pub async fn get_positions(
    State(state): State<Arc<RiskApiState>>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<PositionResponse>>, String> {
    let user_id = Uuid::parse_str(&user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let engine = state.engine.read().await;
    let positions = engine.get_user_positions(user_id);

    let response: Vec<PositionResponse> = positions
        .into_iter()
        .map(|p| PositionResponse {
            instrument_id: p.instrument_id.clone(),
            side: format!("{:?}", p.side),
            quantity: p.quantity,
            avg_price: p.avg_price,
        })
        .collect();

    Ok(Json(response))
}

pub async fn update_balance(
    State(state): State<Arc<RiskApiState>>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, String> {
    let user_id = req["user_id"]
        .as_str()
        .ok_or("user_id required")?;
    let balance = req["balance"]
        .as_f64()
        .ok_or("balance required")?;

    let user_id = Uuid::parse_str(user_id)
        .map_err(|e| format!("Invalid user_id: {}", e))?;

    let mut engine = state.engine.write().await;
    engine.update_wallet_balance(user_id, balance);

    Ok(Json(serde_json::json!({
        "success": true
    })))
}

pub async fn register_instrument(
    State(state): State<Arc<RiskApiState>>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, String> {
    let instrument_id = req["instrument_id"]
        .as_str()
        .ok_or("instrument_id required")?;
    let strike_price = req["strike_price"]
        .as_f64()
        .ok_or("strike_price required")?;
    let contract_size = req["contract_size"]
        .as_f64()
        .unwrap_or(0.01);
    let is_call = req["is_call"]
        .as_bool()
        .unwrap_or(true);

    let mut engine = state.engine.write().await;
    engine.register_instrument(
        instrument_id.to_string(),
        InstrumentInfo {
            strike_price,
            contract_size,
            is_call,
        },
    );

    Ok(Json(serde_json::json!({
        "success": true
    })))
}
