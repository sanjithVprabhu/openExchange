//! API routes for OMS

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::api::handlers::{OmsApiState, health_handler, create_order, list_orders, get_active_orders, get_order, cancel_order, get_fills};

/// Create the OMS router
pub fn create_router(state: Arc<OmsApiState>) -> Router {
    Router::new()
        .route("/api/v1/oms/health", get(health_handler))
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
pub fn create_api_state(manager: crate::manager::OrderManager) -> Arc<OmsApiState> {
    Arc::new(OmsApiState {
        manager: Arc::new(manager),
    })
}
