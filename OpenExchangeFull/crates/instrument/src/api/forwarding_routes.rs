//! Axum routes for gateway forwarding handlers.

use super::forwarding::*;
use axum::routing::{get, patch, post};
use axum::Router;
use std::sync::Arc;

/// Create forwarding routes for the gateway.
///
/// These routes forward HTTP requests to the instrument service via HTTP.
pub fn instrument_forwarding_routes(state: Arc<ForwardingState>) -> Router {
    Router::new()
        .route(
            "/api/v1/{env}/instruments",
            get(list_instruments),
        )
        .route(
            "/api/v1/{env}/instruments/active",
            get(list_active_instruments),
        )
        .route(
            "/api/v1/{env}/instruments/stats",
            get(get_stats),
        )
        .route(
            "/api/v1/{env}/instruments/symbol/{symbol}",
            get(get_instrument_by_symbol),
        )
        .route(
            "/api/v1/{env}/instruments/{id}",
            get(get_instrument_by_id),
        )
        .route(
            "/api/v1/{env}/instruments/{id}/status",
            patch(update_instrument_status),
        )
        .route(
            "/api/v1/{env}/instruments/regenerate",
            post(force_regenerate),
        )
        .with_state(state)
}
