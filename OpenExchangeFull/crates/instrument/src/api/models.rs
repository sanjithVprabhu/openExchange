//! API request/response models.

use crate::types::OptionInstrument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query parameters for listing instruments.
#[derive(Debug, Deserialize)]
pub struct ListInstrumentsParams {
    pub underlying: Option<String>,
    pub option_type: Option<String>,
    pub status: Option<String>,
    pub expiry_after: Option<String>,
    pub expiry_before: Option<String>,
    pub strike_min: Option<f64>,
    pub strike_max: Option<f64>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

/// Response for listing instruments.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListInstrumentsResponse {
    pub success: bool,
    pub environment: String,
    pub total_count: usize,
    pub returned_count: usize,
    pub offset: usize,
    pub instruments: Vec<InstrumentResponse>,
}

/// Single instrument in API response.
#[derive(Debug, Serialize, Deserialize)]
pub struct InstrumentResponse {
    pub id: String,
    pub symbol: String,
    pub underlying: UnderlyingResponse,
    pub option_type: String,
    pub exercise_style: String,
    pub strike: StrikeResponse,
    pub expiry: String,
    pub settlement_currency: String,
    pub contract_size: f64,
    pub tick_size: f64,
    pub min_order_size: u64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnderlyingResponse {
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    pub contract_size: f64,
    pub tick_size: f64,
    pub price_decimals: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrikeResponse {
    pub value: f64,
    pub decimals: u32,
}

impl From<&OptionInstrument> for InstrumentResponse {
    fn from(i: &OptionInstrument) -> Self {
        Self {
            id: i.id.to_string(),
            symbol: i.symbol.clone(),
            underlying: UnderlyingResponse {
                symbol: i.underlying.symbol.clone(),
                name: i.underlying.name.clone(),
                decimals: i.underlying.decimals,
                contract_size: i.underlying.contract_size,
                tick_size: i.underlying.tick_size,
                price_decimals: i.underlying.price_decimals,
            },
            option_type: i.option_type.as_db_str().to_string(),
            exercise_style: match i.exercise_style {
                crate::types::ExerciseStyle::European => "european".to_string(),
                crate::types::ExerciseStyle::American => "american".to_string(),
            },
            strike: StrikeResponse {
                value: i.strike.value(),
                decimals: i.strike.decimals(),
            },
            expiry: i.expiry.to_rfc3339(),
            settlement_currency: i.settlement_currency.clone(),
            contract_size: i.contract_size,
            tick_size: i.tick_size,
            min_order_size: i.min_order_size,
            status: i.status.as_db_str().to_string(),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

/// Response for a single instrument.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetInstrumentResponse {
    pub success: bool,
    pub instrument: InstrumentResponse,
}

/// Statistics response.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub success: bool,
    pub environment: String,
    pub statistics: StatsData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsData {
    pub total: usize,
    pub by_status: HashMap<String, usize>,
    pub by_underlying: HashMap<String, usize>,
}

/// Request to update instrument status.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

/// Response after updating status.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStatusResponse {
    pub success: bool,
    pub instrument: InstrumentResponse,
}

/// Request to force regeneration.
#[derive(Debug, Serialize, Deserialize)]
pub struct ForceRegenerateRequest {
    pub underlying: String,
    pub spot_price: f64,
}

/// Response after force regeneration.
#[derive(Debug, Serialize, Deserialize)]
pub struct ForceRegenerateResponse {
    pub success: bool,
    pub underlying: String,
    pub spot_price: f64,
    pub instruments_created: usize,
    pub instruments_updated: u64,
}

/// Error response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}
