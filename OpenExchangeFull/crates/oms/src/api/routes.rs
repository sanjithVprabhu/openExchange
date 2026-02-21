//! API routes for OMS

use axum::{
    routing::{get, post, delete, patch},
    Router,
};
use crate::api::handlers::*;
use crate::api::OmsApiState;

/// Create the OMS router
pub fn create_router(state: OmsApiState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route(
            "/api/v1/:env/orders",
            post(create_order).get(list_orders),
        )
        .route(
            "/api/v1/:env/orders/active/:user_id",
            get(get_active_orders),
        )
        .route(
            "/api/v1/:env/orders/:order_id",
            get(get_order).delete(cancel_order),
        )
        .route(
            "/api/v1/:env/orders/:order_id/fills",
            get(get_fills),
        )
        .with_state(state)
}

/// Get the API state for the router
pub fn create_api_state(manager: crate::manager::OrderManager) -> OmsApiState {
    OmsApiState {
        manager: Arc::new(manager),
    }
}
