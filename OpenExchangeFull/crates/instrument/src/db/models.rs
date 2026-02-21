//! Database row models for instruments and generation state.
//!
//! These structs map directly to PostgreSQL table rows and handle
//! conversion to/from the domain types in `crate::types`.

use crate::types::{
    ExerciseStyle, InstrumentId, InstrumentStatus, OptionInstrument, OptionType, Strike,
    UnderlyingAsset,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Operating environment for instrument tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Prod,
    Virtual,
    Static,
}

impl Environment {
    /// Get the table name for this environment.
    pub fn table_name(&self) -> &'static str {
        match self {
            Environment::Prod => "instruments_prod",
            Environment::Virtual => "instruments_virtual",
            Environment::Static => "instruments_static",
        }
    }

    /// Get the environment string for generation_state table.
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Prod => "prod",
            Environment::Virtual => "virtual",
            Environment::Static => "static",
        }
    }

    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "prod" | "production" => Some(Environment::Prod),
            "virtual" => Some(Environment::Virtual),
            "static" => Some(Environment::Static),
            _ => None,
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Database row for an instrument.
/// Maps directly to instruments_{prod,virtual,static} tables.
#[derive(Debug, Clone, FromRow)]
pub struct InstrumentRow {
    pub id: Uuid,
    pub symbol: String,
    pub underlying_symbol: String,
    pub underlying_name: String,
    pub underlying_decimals: i32,
    pub option_type: String,
    pub exercise_style: String,
    pub strike_value: sqlx::types::BigDecimal,
    pub strike_decimals: i32,
    pub expiry: DateTime<Utc>,
    pub settlement_currency: String,
    pub contract_size: sqlx::types::BigDecimal,
    pub tick_size: sqlx::types::BigDecimal,
    pub min_order_size: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl InstrumentRow {
    /// Convert from domain type to database row.
    pub fn from_domain(instrument: &OptionInstrument) -> Self {
        use std::str::FromStr;
        Self {
            id: Uuid::parse_str(instrument.id.as_str()).unwrap_or_else(|_| Uuid::new_v4()),
            symbol: instrument.symbol.clone(),
            underlying_symbol: instrument.underlying.symbol.clone(),
            underlying_name: instrument.underlying.name.clone(),
            underlying_decimals: instrument.underlying.decimals as i32,
            option_type: instrument.option_type.as_db_str().to_string(),
            exercise_style: match instrument.exercise_style {
                ExerciseStyle::European => "european".to_string(),
                ExerciseStyle::American => "american".to_string(),
            },
            strike_value: sqlx::types::BigDecimal::from_str(&format!(
                "{:.8}",
                instrument.strike.value()
            ))
            .unwrap_or_default(),
            strike_decimals: instrument.strike.decimals() as i32,
            expiry: instrument.expiry,
            settlement_currency: instrument.settlement_currency.clone(),
            contract_size: sqlx::types::BigDecimal::from_str(&format!(
                "{:.16}",
                instrument.contract_size
            ))
            .unwrap_or_default(),
            tick_size: sqlx::types::BigDecimal::from_str(&format!(
                "{:.16}",
                instrument.tick_size
            ))
            .unwrap_or_default(),
            min_order_size: instrument.min_order_size as i64,
            status: instrument.status.as_db_str().to_string(),
            created_at: instrument.created_at,
            updated_at: instrument.updated_at,
        }
    }

    /// Convert from database row to domain type.
    pub fn to_domain(&self) -> OptionInstrument {
        let option_type = OptionType::from_db_str(&self.option_type).unwrap_or(OptionType::Call);
        let exercise_style = match self.exercise_style.as_str() {
            "american" => ExerciseStyle::American,
            _ => ExerciseStyle::European,
        };
        let status =
            InstrumentStatus::from_db_str(&self.status).unwrap_or(InstrumentStatus::Active);

        // Convert BigDecimal to f64
        let strike_value = bigdecimal_to_f64(&self.strike_value);
        let contract_size = bigdecimal_to_f64(&self.contract_size);
        let tick_size = bigdecimal_to_f64(&self.tick_size);

        OptionInstrument {
            id: InstrumentId::new(self.id.to_string()),
            symbol: self.symbol.clone(),
            underlying: UnderlyingAsset {
                symbol: self.underlying_symbol.clone(),
                name: self.underlying_name.clone(),
                decimals: self.underlying_decimals as u32,
                contract_size,
                tick_size,
                price_decimals: self.strike_decimals as u32,
            },
            option_type,
            strike: Strike::new(strike_value, self.strike_decimals as u32),
            expiry: self.expiry,
            exercise_style,
            settlement_currency: self.settlement_currency.clone(),
            contract_size,
            tick_size,
            min_order_size: self.min_order_size as u64,
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Database row for generation state.
/// Maps to the generation_state table.
#[derive(Debug, Clone, FromRow)]
pub struct GenerationStateRow {
    pub id: i32,
    pub environment: String,
    pub asset_symbol: String,
    pub upper_reference: sqlx::types::BigDecimal,
    pub lower_reference: sqlx::types::BigDecimal,
    pub upper_trigger: sqlx::types::BigDecimal,
    pub lower_trigger: sqlx::types::BigDecimal,
    pub max_strike: sqlx::types::BigDecimal,
    pub min_strike: sqlx::types::BigDecimal,
    pub last_spot_price: sqlx::types::BigDecimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Domain representation of generation state.
#[derive(Debug, Clone)]
pub struct GenerationState {
    pub environment: Environment,
    pub asset_symbol: String,
    pub upper_reference: f64,
    pub lower_reference: f64,
    pub upper_trigger: f64,
    pub lower_trigger: f64,
    pub max_strike: f64,
    pub min_strike: f64,
    pub last_spot_price: f64,
}

impl GenerationStateRow {
    /// Convert to domain type.
    pub fn to_domain(&self) -> GenerationState {
        GenerationState {
            environment: Environment::from_str(&self.environment).unwrap_or(Environment::Static),
            asset_symbol: self.asset_symbol.clone(),
            upper_reference: bigdecimal_to_f64(&self.upper_reference),
            lower_reference: bigdecimal_to_f64(&self.lower_reference),
            upper_trigger: bigdecimal_to_f64(&self.upper_trigger),
            lower_trigger: bigdecimal_to_f64(&self.lower_trigger),
            max_strike: bigdecimal_to_f64(&self.max_strike),
            min_strike: bigdecimal_to_f64(&self.min_strike),
            last_spot_price: bigdecimal_to_f64(&self.last_spot_price),
        }
    }
}

/// Helper to convert BigDecimal to f64.
fn bigdecimal_to_f64(bd: &sqlx::types::BigDecimal) -> f64 {
    bd.to_string()
        .parse::<f64>()
        .unwrap_or(0.0)
}
