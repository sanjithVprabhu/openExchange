//! HTTP forwarding handlers for gateway mode.
//!
//! These handlers forward HTTP requests to the instrument service via HTTP.
//! This is simpler than gRPC forwarding and works in all environments.

use crate::api::models::{
    ErrorResponse, ForceRegenerateRequest, ForceRegenerateResponse, GetInstrumentResponse,
    ListInstrumentsParams, ListInstrumentsResponse, StatsResponse,
    UpdateStatusRequest, UpdateStatusResponse,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use tracing::{debug, error, info};

/// HTTP client for forwarding requests to the instrument service.
#[derive(Clone)]
pub struct InstrumentForwarder {
    client: reqwest::Client,
    base_url: String,
}

impl InstrumentForwarder {
    /// Create a new forwarder pointing to the instrument service.
    pub fn new(instrument_service_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: instrument_service_url.trim_end_matches('/').to_string(),
        }
    }

    /// Get the base URL of the instrument service.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// State for forwarding handlers.
#[derive(Clone)]
pub struct ForwardingState {
    pub instrument: InstrumentForwarder,
    // Future: add other service forwarders
    // pub oms: OmsForwarder,
    // pub matching: MatchingForwarder,
}

// =============================================================================
// Forwarding Handlers
// =============================================================================

/// GET /api/v1/{env}/instruments - List instruments
pub async fn list_instruments(
    State(state): State<Arc<ForwardingState>>,
    Path(env): Path<String>,
    Query(params): Query<ListInstrumentsParams>,
) -> Result<Json<ListInstrumentsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Forwarding list_instruments request to {} for env {}", state.instrument.base_url, env);

    let mut query = Vec::new();
    if let Some(ref underlying) = params.underlying {
        query.push(("underlying", underlying.clone()));
    }
    if let Some(ref option_type) = params.option_type {
        query.push(("option_type", option_type.clone()));
    }
    if let Some(ref status) = params.status {
        query.push(("status", status.clone()));
    }
    if let Some(ref expiry_after) = params.expiry_after {
        query.push(("expiry_after", expiry_after.clone()));
    }
    if let Some(ref expiry_before) = params.expiry_before {
        query.push(("expiry_before", expiry_before.clone()));
    }
    if let Some(strike_min) = params.strike_min {
        query.push(("strike_min", strike_min.to_string()));
    }
    if let Some(strike_max) = params.strike_max {
        query.push(("strike_max", strike_max.to_string()));
    }
    query.push(("limit", params.limit.to_string()));
    query.push(("offset", params.offset.to_string()));

    let url = format!("{}/api/v1/{}/instruments", state.instrument.base_url, env);

    let response = state.instrument.client
        .get(&url)
        .query(&query)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        error!("Instrument service returned error: {} - {}", status, body);
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument service error: {}", body),
            }),
        ));
    }

    let result: ListInstrumentsResponse = response.json().await.map_err(|e| {
        error!("Failed to parse response: {}", e);
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}

/// GET /api/v1/{env}/instruments/active - List active instruments
pub async fn list_active_instruments(
    state: State<Arc<ForwardingState>>,
    Path(env): Path<String>,
    Query(mut params): Query<ListInstrumentsParams>,
) -> Result<Json<ListInstrumentsResponse>, (StatusCode, Json<ErrorResponse>)> {
    params.status = Some("active".to_string());
    list_instruments(state, Path(env), Query(params)).await
}

/// GET /api/v1/{env}/instruments/{id} - Get instrument by ID
pub async fn get_instrument_by_id(
    State(state): State<Arc<ForwardingState>>,
    Path((env, id)): Path<(String, String)>,
) -> Result<Json<GetInstrumentResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Forwarding get_instrument request for id {} in env {}", id, env);

    let url = format!("{}/api/v1/{}/instruments/{}", state.instrument.base_url, env, id);

    let response = state.instrument.client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to forward request: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument not found: {}", id),
            }),
        ));
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument service error: {}", body),
            }),
        ));
    }

    let result: GetInstrumentResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}

/// GET /api/v1/{env}/instruments/symbol/{symbol} - Get instrument by symbol
pub async fn get_instrument_by_symbol(
    State(state): State<Arc<ForwardingState>>,
    Path((env, symbol)): Path<(String, String)>,
) -> Result<Json<GetInstrumentResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Forwarding get_instrument_by_symbol request for {} in env {}", symbol, env);

    let url = format!("{}/api/v1/{}/instruments/symbol/{}", state.instrument.base_url, env, symbol);

    let response = state.instrument.client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument not found: {}", symbol),
            }),
        ));
    }

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument service error: {}", body),
            }),
        ));
    }

    let result: GetInstrumentResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}

/// GET /api/v1/{env}/instruments/stats - Get statistics
pub async fn get_stats(
    State(state): State<Arc<ForwardingState>>,
    Path(env): Path<String>,
) -> Result<Json<StatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("Forwarding get_stats request for env {}", env);

    let url = format!("{}/api/v1/{}/instruments/stats", state.instrument.base_url, env);

    let response = state.instrument.client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Instrument service error: {}", body),
            }),
        ));
    }

    let result: StatsResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}

/// PATCH /api/v1/{env}/instruments/{id}/status - Update instrument status
pub async fn update_instrument_status(
    State(state): State<Arc<ForwardingState>>,
    Path((env, id)): Path<(String, String)>,
    Json(body): Json<UpdateStatusRequest>,
) -> Result<Json<UpdateStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Forwarding update_status request for {} in env {} to {}", id, env, body.status);

    let url = format!("{}/api/v1/{}/instruments/{}/status", state.instrument.base_url, env, id);

    let response = state.instrument.client
        .patch(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            status,
            Json(ErrorResponse {
                success: false,
                error: body,
            }),
        ));
    }

    let result: UpdateStatusResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}

/// POST /api/v1/{env}/instruments/regenerate - Force regenerate instruments
pub async fn force_regenerate(
    State(state): State<Arc<ForwardingState>>,
    Path(env): Path<String>,
    Json(body): Json<ForceRegenerateRequest>,
) -> Result<Json<ForceRegenerateResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Forwarding force_regenerate request for {} at {}", body.underlying, body.spot_price);

    let url = format!("{}/api/v1/{}/instruments/regenerate", state.instrument.base_url, env);

    let response = state.instrument.client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to connect to instrument service: {}", e),
                }),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            status,
            Json(ErrorResponse {
                success: false,
                error: body,
            }),
        ));
    }

    let result: ForceRegenerateResponse = response.json().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to parse response: {}", e),
            }),
        )
    })?;

    Ok(Json(result))
}
