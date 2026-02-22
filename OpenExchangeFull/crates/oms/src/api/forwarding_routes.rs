//! Axum routes for gateway forwarding handlers.

use super::forwarding::*;
use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

pub fn oms_forwarding_routes(state: Arc<OmsForwardingState>) -> Router {
    Router::new()
        .route(
            "/api/v1/{env}/orders",
            post(forward_create_order).get(forward_list_orders),
        )
        .route(
            "/api/v1/{env}/orders/active/:user_id",
            get(forward_get_active_orders),
        )
        .route(
            "/api/v1/{env}/orders/:order_id",
            get(forward_get_order).delete(forward_cancel_order),
        )
        .route(
            "/api/v1/{env}/orders/:order_id/fills",
            get(forward_get_fills),
        )
        .with_state(state)
}
