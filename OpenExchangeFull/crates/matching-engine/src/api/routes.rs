//! HTTP routes for the Matching Engine API

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::store::MatchingStore;
use super::handlers::*;

/// Create the matching engine router
/// 
/// Routes:
/// - POST   /api/v1/internal/orders              - Submit order
/// - DELETE /api/v1/internal/orders/:instrument_id/:order_id - Cancel order
/// - GET    /api/v1/internal/books/:instrument_id - Get order book snapshot
/// - GET    /api/v1/internal/trades/:instrument_id - Get recent trades
/// - GET    /api/v1/matching/health              - Health check (service-specific path)
pub fn create_router<S: MatchingStore + 'static + ?Sized>(state: MatchingApiState<S>) -> Router {
    Router::new()
        // Health check with service-specific path to avoid conflicts
        .route("/api/v1/matching/health", get(health))
        // Order submission
        .route(
            "/api/v1/internal/orders",
            post(submit_order),
        )
        // Order cancellation
        .route(
            "/api/v1/internal/orders/:instrument_id/:order_id",
            delete(cancel_order),
        )
        // Order book snapshot
        .route(
            "/api/v1/internal/books/:instrument_id",
            get(get_order_book),
        )
        // Recent trades
        .route(
            "/api/v1/internal/trades/:instrument_id",
            get(get_trades),
        )
        .with_state(state)
}

/// Create router with dynamic dispatch (trait object)
/// 
/// Use this when you have `Arc<dyn MatchingStore>` instead of a concrete type.
pub fn create_dyn_router(store: Arc<dyn MatchingStore + Send + Sync>) -> Router {
    let state = DynMatchingApiState { store };
    create_router(state)
}
