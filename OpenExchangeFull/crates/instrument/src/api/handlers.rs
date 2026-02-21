//! HTTP request handlers for instrument API.

use crate::api::models::*;
use crate::db::postgres::PostgresInstrumentStore;
use crate::error::InstrumentError;
use crate::store::{InstrumentQuery, InstrumentStore};
use crate::types::{InstrumentId, InstrumentStatus, OptionType};
use crate::worker::service::InstrumentWorker;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::DateTime;
use std::collections::HashMap;
use std::sync::Arc;


/// Shared state for instrument API handlers.
pub struct InstrumentApiState {
    /// One store per environment
    pub stores: HashMap<String, Arc<PostgresInstrumentStore>>,
    /// Worker for force regeneration (optional)
    pub worker: Option<Arc<InstrumentWorker>>,
}

impl InstrumentApiState {
    /// Get the store for a given environment string.
    fn get_store(&self, env: &str) -> Result<&Arc<PostgresInstrumentStore>, (StatusCode, Json<ErrorResponse>)> {
        self.stores.get(env).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Invalid environment: {}. Use prod, virtual, or static.", env),
                }),
            )
        })
    }
}

/// GET /api/v1/{env}/instruments
pub async fn list_instruments(
    State(state): State<Arc<InstrumentApiState>>,
    Path(env): Path<String>,
    Query(params): Query<ListInstrumentsParams>,
) -> Result<Json<ListInstrumentsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.get_store(&env)?;

    // Build query from params
    let mut query = InstrumentQuery::new();

    if let Some(ref underlying) = params.underlying {
        query = query.with_underlying(underlying);
    }
    if let Some(ref option_type) = params.option_type {
        if let Some(ot) = OptionType::from_db_str(option_type) {
            query = query.with_option_type(ot);
        }
    }
    if let Some(ref status) = params.status {
        if let Some(s) = InstrumentStatus::from_db_str(status) {
            query = query.with_status(s);
        }
    }
    if let Some(strike_min) = params.strike_min {
        query.strike_min = Some(strike_min);
    }
    if let Some(strike_max) = params.strike_max {
        query.strike_max = Some(strike_max);
    }

    // Parse expiry dates
    if let Some(ref expiry_after) = params.expiry_after {
        if let Ok(dt) = DateTime::parse_from_rfc3339(expiry_after) {
            query.expiry_after = Some(dt.with_timezone(&chrono::Utc));
        }
    }
    if let Some(ref expiry_before) = params.expiry_before {
        if let Ok(dt) = DateTime::parse_from_rfc3339(expiry_before) {
            query.expiry_before = Some(dt.with_timezone(&chrono::Utc));
        }
    }

    // Clamp limit
    let limit = params.limit.min(1000);
    query = query.with_pagination(limit, params.offset);

    // Get total count (without pagination)
    let mut count_query = query.clone();
    count_query.limit = None;
    count_query.offset = None;
    let total_count = store.count(&count_query).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to count instruments: {}", e),
            }),
        )
    })?;

    // Get paginated results
    let instruments = store.list(&query).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to list instruments: {}", e),
            }),
        )
    })?;

    let returned_count = instruments.len();
    let responses: Vec<InstrumentResponse> = instruments.iter().map(|i| i.into()).collect();

    Ok(Json(ListInstrumentsResponse {
        success: true,
        environment: env,
        total_count,
        returned_count,
        offset: params.offset,
        instruments: responses,
    }))
}

/// GET /api/v1/{env}/instruments/active
pub async fn list_active_instruments(
    State(state): State<Arc<InstrumentApiState>>,
    Path(env): Path<String>,
    Query(mut params): Query<ListInstrumentsParams>,
) -> Result<Json<ListInstrumentsResponse>, (StatusCode, Json<ErrorResponse>)> {
    params.status = Some("active".to_string());
    list_instruments(State(state), Path(env), Query(params)).await
}

/// GET /api/v1/{env}/instruments/{id}
pub async fn get_instrument_by_id(
    State(state): State<Arc<InstrumentApiState>>,
    Path((env, id)): Path<(String, String)>,
) -> Result<Json<GetInstrumentResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.get_store(&env)?;

    let instrument_id = InstrumentId::new(&id);
    let instrument = store.get(&instrument_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to get instrument: {}", e),
            }),
        )
    })?;

    match instrument {
        Some(i) => Ok(Json(GetInstrumentResponse {
            success: true,
            instrument: (&i).into(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument not found: {}", id),
            }),
        )),
    }
}

/// GET /api/v1/{env}/instruments/symbol/{symbol}
pub async fn get_instrument_by_symbol(
    State(state): State<Arc<InstrumentApiState>>,
    Path((env, symbol)): Path<(String, String)>,
) -> Result<Json<GetInstrumentResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.get_store(&env)?;

    let instrument = store.get_by_symbol(&symbol).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to get instrument: {}", e),
            }),
        )
    })?;

    match instrument {
        Some(i) => Ok(Json(GetInstrumentResponse {
            success: true,
            instrument: (&i).into(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument not found: {}", symbol),
            }),
        )),
    }
}

/// GET /api/v1/{env}/instruments/stats
pub async fn get_stats(
    State(state): State<Arc<InstrumentApiState>>,
    Path(env): Path<String>,
) -> Result<Json<StatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.get_store(&env)?;

    let total = store
        .count(&InstrumentQuery::new())
        .await
        .unwrap_or(0);

    let active = store
        .count(&InstrumentQuery::new().with_status(InstrumentStatus::Active))
        .await
        .unwrap_or(0);
    let inactive = store
        .count(&InstrumentQuery::new().with_status(InstrumentStatus::Inactive))
        .await
        .unwrap_or(0);
    let expired = store
        .count(&InstrumentQuery::new().with_status(InstrumentStatus::Expired))
        .await
        .unwrap_or(0);
    let settled = store
        .count(&InstrumentQuery::new().with_status(InstrumentStatus::Settled))
        .await
        .unwrap_or(0);

    let mut by_status = HashMap::new();
    by_status.insert("active".to_string(), active);
    by_status.insert("inactive".to_string(), inactive);
    by_status.insert("expired".to_string(), expired);
    by_status.insert("settled".to_string(), settled);

    // Count by underlying - use common asset symbols
    let mut by_underlying = HashMap::new();
    for symbol in &["BTC", "ETH", "SOL"] {
        let count = store
            .count(&InstrumentQuery::new().with_underlying(*symbol))
            .await
            .unwrap_or(0);
        if count > 0 {
            by_underlying.insert(symbol.to_string(), count);
        }
    }

    Ok(Json(StatsResponse {
        success: true,
        environment: env,
        statistics: StatsData {
            total,
            by_status,
            by_underlying,
        },
    }))
}

/// PATCH /api/v1/{env}/instruments/{id}/status
pub async fn update_instrument_status(
    State(state): State<Arc<InstrumentApiState>>,
    Path((env, id)): Path<(String, String)>,
    Json(body): Json<UpdateStatusRequest>,
) -> Result<Json<UpdateStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.get_store(&env)?;

    let new_status = InstrumentStatus::from_db_str(&body.status).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: format!("Invalid status: {}", body.status),
            }),
        )
    })?;

    let instrument_id = InstrumentId::new(&id);

    store
        .update_status(&instrument_id, new_status)
        .await
        .map_err(|e| {
            let status_code = match &e {
                InstrumentError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status_code,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                }),
            )
        })?;

    // Fetch updated instrument
    let instrument = store
        .get(&instrument_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: e.to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Instrument not found: {}", id),
                }),
            )
        })?;

    Ok(Json(UpdateStatusResponse {
        success: true,
        instrument: (&instrument).into(),
    }))
}

/// POST /api/v1/{env}/instruments/regenerate
pub async fn force_regenerate(
    State(state): State<Arc<InstrumentApiState>>,
    Path(_env): Path<String>,
    Json(body): Json<ForceRegenerateRequest>,
) -> Result<Json<ForceRegenerateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let worker = state.worker.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                success: false,
                error: "Worker not available for regeneration".to_string(),
            }),
        )
    })?;

    let (created, updated) = worker
        .force_regenerate(&body.underlying, body.spot_price)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Regeneration failed: {}", e),
                }),
            )
        })?;

    Ok(Json(ForceRegenerateResponse {
        success: true,
        underlying: body.underlying,
        spot_price: body.spot_price,
        instruments_created: created,
        instruments_updated: updated,
    }))
}
