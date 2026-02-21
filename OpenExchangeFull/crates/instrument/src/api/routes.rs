//! Axum route definitions for the instrument API.

use crate::api::handlers::{self, InstrumentApiState};
use axum::routing::{get, patch, post};
use axum::Router;
use std::sync::Arc;

/// Create all instrument routes.
///
/// # Routes
///
/// - `GET /api/v1/{env}/instruments` - List instruments with filters
/// - `GET /api/v1/{env}/instruments/active` - List active instruments
/// - `GET /api/v1/{env}/instruments/{id}` - Get by ID
/// - `GET /api/v1/{env}/instruments/symbol/{symbol}` - Get by symbol
/// - `GET /api/v1/{env}/instruments/stats` - Get statistics
/// - `PATCH /api/v1/{env}/instruments/{id}/status` - Update status (admin)
/// - `POST /api/v1/{env}/instruments/regenerate` - Force regeneration (admin)
pub fn instrument_routes(state: Arc<InstrumentApiState>) -> Router {
    Router::new()
        .route(
            "/api/v1/{env}/instruments",
            get(handlers::list_instruments),
        )
        .route(
            "/api/v1/{env}/instruments/active",
            get(handlers::list_active_instruments),
        )
        .route(
            "/api/v1/{env}/instruments/stats",
            get(handlers::get_stats),
        )
        .route(
            "/api/v1/{env}/instruments/symbol/{symbol}",
            get(handlers::get_instrument_by_symbol),
        )
        .route(
            "/api/v1/{env}/instruments/{id}",
            get(handlers::get_instrument_by_id),
        )
        .route(
            "/api/v1/{env}/instruments/{id}/status",
            patch(handlers::update_instrument_status),
        )
        .route(
            "/api/v1/{env}/instruments/regenerate",
            post(handlers::force_regenerate),
        )
        .with_state(state)
}
