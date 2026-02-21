//! API handlers for OMS HTTP endpoints

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::types::{Order, OrderStatus, Environment};
use crate::manager::OrderManager;
use crate::api::models::*;
use crate::error::OmsError;

pub struct OmsApiState {
    pub manager: Arc<OrderManager>,
}

/// Health check handler
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "oms".to_string(),
    })
}

/// Create order handler
pub async fn create_order(
    State(state): State<Arc<OmsApiState>>,
    Path(env): Path<String>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());

    // For now, use a default user ID (in production, get from auth)
    let user_id = Uuid::nil();

    let order = Order::new(
        user_id,
        req.instrument_id,
        req.side,
        req.order_type,
        req.time_in_force,
        req.price,
        req.quantity,
    );

    match state.manager.submit_order(order, env).await {
        Ok(order) => Ok(Json(CreateOrderResponse::success(OrderResponse::from(order)))),
        Err(e) => {
            let (code, message) = match e {
                OmsError::ValidationError(msg) => ("VALIDATION_ERROR", msg),
                OmsError::RiskRejected(msg) => ("RISK_REJECTED", msg),
                _ => ("INTERNAL_ERROR", e.to_string()),
            };
            Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: code.to_string(),
                        message: message.to_string(),
                        details: None,
                    },
                }),
            ))
        }
    }
}

/// Get order handler
pub async fn get_order(
    State(state): State<Arc<OmsApiState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<CreateOrderResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());
    let order_id = Uuid::parse_str(&order_id)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: "INVALID_ORDER_ID".to_string(),
                        message: "Invalid order ID format".to_string(),
                        details: None,
                    },
                }),
            )
        })?;

    match state.manager.get_order(order_id, env).await {
        Ok(Some(order)) => Ok(Json(CreateOrderResponse::success(OrderResponse::from(order)))),
        Ok(None) => Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: ErrorDetail {
                    code: "ORDER_NOT_FOUND".to_string(),
                    message: format!("Order {} not found", order_id),
                    details: None,
                },
            }),
        )),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: ErrorDetail {
                    code: "INTERNAL_ERROR".to_string(),
                    message: e.to_string(),
                    details: None,
                },
            }),
        )),
    }
}

/// List orders handler
pub async fn list_orders(
    State(state): State<Arc<OmsApiState>>,
    Path(env): Path<String>,
    Query(params): Query<ListOrdersParams>,
) -> Result<Json<ListOrdersResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());

    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let statuses = params.status.as_ref().and_then(|s| {
        Some(s.split(',').filter_map(|ss| {
            match ss.trim().to_lowercase().as_str() {
                "pending_risk" => Some(OrderStatus::PendingRisk),
                "open" => Some(OrderStatus::Open),
                "partially_filled" => Some(OrderStatus::PartiallyFilled),
                "filled" => Some(OrderStatus::Filled),
                "cancelled" => Some(OrderStatus::Cancelled),
                "rejected" => Some(OrderStatus::Rejected),
                "expired" => Some(OrderStatus::Expired),
                _ => None,
            }
        }).collect())
    });

    match state.manager.list_orders(None, params.instrument_id.as_deref(), statuses, env, limit, offset).await {
        Ok(orders) => {
            let total_count = orders.len() as u64;
            let orders: Vec<OrderResponse> = orders.into_iter().map(OrderResponse::from).collect();
            Ok(Json(ListOrdersResponse {
                success: true,
                total_count,
                returned_count: orders.len() as u32,
                offset,
                orders,
            }))
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: ErrorDetail {
                    code: "INTERNAL_ERROR".to_string(),
                    message: e.to_string(),
                    details: None,
                },
            }),
        )),
    }
}

/// Cancel order handler
pub async fn cancel_order(
    State(state): State<Arc<OmsApiState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<CancelOrderResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());
    let order_id = Uuid::parse_str(&order_id)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: "INVALID_ORDER_ID".to_string(),
                        message: "Invalid order ID format".to_string(),
                        details: None,
                    },
                }),
            )
        })?;

    match state.manager.cancel_order(order_id, env).await {
        Ok(order) => Ok(Json(CancelOrderResponse {
            success: true,
            order: Some(OrderResponse::from(order)),
            error: None,
        })),
        Err(e) => {
            let code = match e {
                OmsError::NotFound(_) => "ORDER_NOT_FOUND",
                OmsError::OrderNotCancellable(_) => "INVALID_STATE",
                _ => "INTERNAL_ERROR",
            };
            Err((
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: code.to_string(),
                        message: e.to_string(),
                        details: None,
                    },
                }),
            ))
        }
    }
}

/// Get order fills handler
pub async fn get_fills(
    State(state): State<Arc<OmsApiState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<GetFillsResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());
    let order_id = Uuid::parse_str(&order_id)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: "INVALID_ORDER_ID".to_string(),
                        message: "Invalid order ID format".to_string(),
                        details: None,
                    },
                }),
            )
        })?;

    match state.manager.get_fills(order_id, env).await {
        Ok(fills) => {
            let fills: Vec<FillResponse> = fills.into_iter().map(FillResponse::from).collect();
            Ok(Json(GetFillsResponse {
                success: true,
                order_id,
                total_fills: fills.len() as u32,
                fills,
            }))
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: ErrorDetail {
                    code: "INTERNAL_ERROR".to_string(),
                    message: e.to_string(),
                    details: None,
                },
            }),
        )),
    }
}

/// Active orders handler
pub async fn get_active_orders(
    State(state): State<Arc<OmsApiState>>,
    Path((env, user_id)): Path<(String, String)>,
) -> Result<Json<ListOrdersResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let env = Environment::from(env.as_str());
    let user_id = Uuid::parse_str(&user_id)
        .map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: ErrorDetail {
                        code: "INVALID_USER_ID".to_string(),
                        message: "Invalid user ID format".to_string(),
                        details: None,
                    },
                }),
            )
        })?;

    match state.manager.get_active_orders(user_id, env).await {
        Ok(orders) => {
            let orders: Vec<OrderResponse> = orders.into_iter().map(OrderResponse::from).collect();
            Ok(Json(ListOrdersResponse {
                success: true,
                total_count: orders.len() as u64,
                returned_count: orders.len() as u32,
                offset: 0,
                orders,
            }))
        }
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: ErrorDetail {
                    code: "INTERNAL_ERROR".to_string(),
                    message: e.to_string(),
                    details: None,
                },
            }),
        )),
    }
}
