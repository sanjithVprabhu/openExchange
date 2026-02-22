use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use super::handlers::*;

pub fn create_router(state: Arc<RiskApiState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route(
            "/api/v1/internal/risk/check",
            post(check_risk),
        )
        .route(
            "/api/v1/internal/risk/release",
            post(release_margin),
        )
        .route(
            "/api/v1/{env}/risk/margin/:user_id",
            get(get_margin_info),
        )
        .route(
            "/api/v1/{env}/risk/positions/:user_id",
            get(get_positions),
        )
        .route(
            "/api/v1/internal/risk/balance",
            post(update_balance),
        )
        .route(
            "/api/v1/internal/risk/instrument",
            post(register_instrument),
        )
        .with_state(state)
}
