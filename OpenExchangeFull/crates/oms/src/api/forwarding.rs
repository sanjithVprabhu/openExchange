//! Gateway forwarding handlers for OMS
//!
//! These handlers forward requests from the gateway to the OMS service

use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;
use reqwest::Client;

use common::addressbook::AddressBook;
use crate::api::models::*;

pub struct OmsForwardingState {
    pub client: Client,
    pub address_book: Arc<AddressBook>,
}

#[derive(Clone)]
pub struct OmsForwarder {
    pub client: Client,
    pub base_url: String,
}

impl OmsForwarder {
    pub fn new(oms_service_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: oms_service_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Forward create order request
pub async fn forward_create_order(
    State(state): State<Arc<OmsForwardingState>>,
    Path(env): Path<String>,
    Json(req): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders", oms_url, env);

    let response = state.client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: CreateOrderResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

/// Forward list orders request
pub async fn forward_list_orders(
    State(state): State<Arc<OmsForwardingState>>,
    Path(env): Path<String>,
    Query(params): Query<ListOrdersParams>,
) -> Result<Json<ListOrdersResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders", oms_url, env);

    let response = state.client
        .get(&url)
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: ListOrdersResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

/// Forward get order request
pub async fn forward_get_order(
    State(state): State<Arc<OmsForwardingState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<CreateOrderResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders/{}", oms_url, env, order_id);

    let response = state.client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: CreateOrderResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

/// Forward cancel order request
pub async fn forward_cancel_order(
    State(state): State<Arc<OmsForwardingState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<CancelOrderResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders/{}", oms_url, env, order_id);

    let response = state.client
        .delete(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: CancelOrderResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

/// Forward get fills request
pub async fn forward_get_fills(
    State(state): State<Arc<OmsForwardingState>>,
    Path((env, order_id)): Path<(String, String)>,
) -> Result<Json<GetFillsResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders/{}/fills", oms_url, env, order_id);

    let response = state.client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: GetFillsResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}

/// Forward active orders request
pub async fn forward_get_active_orders(
    State(state): State<Arc<OmsForwardingState>>,
    Path((env, user_id)): Path<(String, String)>,
) -> Result<Json<ListOrdersResponse>, String> {
    let oms_url = state.address_book.get_oms_url()
        .ok_or("OMS service not registered")?;

    let url = format!("{}/api/v1/{}/orders/active/{}", oms_url, env, user_id);

    let response = state.client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let result: ListOrdersResponse = response
        .json()
        .await
        .map_err(|e| e.to_string())?;

    Ok(Json(result))
}
