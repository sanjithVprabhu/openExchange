//! Core domain types for instruments.
//!
//! These types represent the fundamental building blocks of the options exchange:
//! - `OptionInstrument`: A tradeable option contract
//! - `OptionType`: Call or Put
//! - `ExerciseStyle`: European or American
//! - `Strike`: Strike price with precision
//! - `UnderlyingAsset`: The underlying asset (BTC, ETH, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for an instrument.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrumentId(String);

impl InstrumentId {
    /// Create a new instrument ID from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new unique instrument ID.
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the inner string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for InstrumentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for InstrumentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Type of option: Call or Put.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    /// Call option - right to buy at strike price.
    Call,
    /// Put option - right to sell at strike price.
    Put,
}

impl OptionType {
    /// Get the short code for the option type.
    pub fn code(&self) -> &'static str {
        match self {
            OptionType::Call => "C",
            OptionType::Put => "P",
        }
    }

    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            OptionType::Call => "call",
            OptionType::Put => "put",
        }
    }

    /// Parse from database string representation.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "call" => Some(OptionType::Call),
            "put" => Some(OptionType::Put),
            _ => None,
        }
    }
}

impl fmt::Display for OptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptionType::Call => write!(f, "Call"),
            OptionType::Put => write!(f, "Put"),
        }
    }
}

/// Exercise style of the option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExerciseStyle {
    /// European - can only be exercised at expiry.
    European,
    /// American - can be exercised any time before expiry.
    American,
}

impl fmt::Display for ExerciseStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExerciseStyle::European => write!(f, "European"),
            ExerciseStyle::American => write!(f, "American"),
        }
    }
}

/// Status of an instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentStatus {
    /// Instrument is active and tradeable.
    Active,
    /// Instrument is inactive (spot price moved out of range, no new orders).
    Inactive,
    /// Instrument is suspended (trading halted by admin).
    Suspended,
    /// Instrument has expired.
    Expired,
    /// Instrument has been settled.
    Settled,
    /// Instrument is pending activation.
    Pending,
}

impl fmt::Display for InstrumentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstrumentStatus::Active => write!(f, "Active"),
            InstrumentStatus::Inactive => write!(f, "Inactive"),
            InstrumentStatus::Suspended => write!(f, "Suspended"),
            InstrumentStatus::Expired => write!(f, "Expired"),
            InstrumentStatus::Settled => write!(f, "Settled"),
            InstrumentStatus::Pending => write!(f, "Pending"),
        }
    }
}

impl InstrumentStatus {
    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            InstrumentStatus::Active => "active",
            InstrumentStatus::Inactive => "inactive",
            InstrumentStatus::Suspended => "suspended",
            InstrumentStatus::Expired => "expired",
            InstrumentStatus::Settled => "settled",
            InstrumentStatus::Pending => "pending",
        }
    }

    /// Parse from database string representation.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(InstrumentStatus::Active),
            "inactive" => Some(InstrumentStatus::Inactive),
            "suspended" => Some(InstrumentStatus::Suspended),
            "expired" => Some(InstrumentStatus::Expired),
            "settled" => Some(InstrumentStatus::Settled),
            "pending" => Some(InstrumentStatus::Pending),
            _ => None,
        }
    }
}

/// Strike price with precision handling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Strike {
    /// The strike price value.
    value: f64,
    /// Number of decimal places for display.
    decimals: u32,
}

impl Strike {
    /// Create a new strike price.
    pub fn new(value: f64, decimals: u32) -> Self {
        Self { value, decimals }
    }

    /// Get the strike value.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Get the number of decimals.
    pub fn decimals(&self) -> u32 {
        self.decimals
    }

    /// Format the strike for display.
    pub fn to_string_with_precision(&self) -> String {
        format!("{:.prec$}", self.value, prec = self.decimals as usize)
    }

    /// Format the strike for symbol (no decimal point, scaled).
    pub fn to_symbol_string(&self) -> String {
        // For symbols, we use integer representation (e.g., 50000 not 50000.00)
        if self.value.fract() == 0.0 {
            format!("{}", self.value as i64)
        } else {
            // Keep minimal decimal places needed
            let s = format!("{}", self.value);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }
}

impl fmt::Display for Strike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_with_precision())
    }
}

impl Eq for Strike {}

impl std::hash::Hash for Strike {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the bits of the f64 for consistent hashing
        self.value.to_bits().hash(state);
        self.decimals.hash(state);
    }
}

/// Underlying asset information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnderlyingAsset {
    /// Asset symbol (e.g., "BTC", "ETH").
    pub symbol: String,
    /// Human-readable name.
    pub name: String,
    /// Number of decimals for the asset.
    pub decimals: u32,
    /// Contract size (how much of the asset per contract).
    pub contract_size: f64,
    /// Minimum tick size for prices.
    pub tick_size: f64,
    /// Number of decimals for price display.
    pub price_decimals: u32,
}

impl UnderlyingAsset {
    /// Create from config Asset.
    pub fn from_config(asset: &config::Asset) -> Self {
        Self {
            symbol: asset.symbol.clone(),
            name: asset.name.clone(),
            decimals: asset.decimals,
            contract_size: asset.contract_size,
            tick_size: asset.tick_size,
            price_decimals: asset.price_decimals,
        }
    }
}

/// A tradeable option instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionInstrument {
    /// Unique identifier.
    pub id: InstrumentId,
    /// Human-readable symbol (e.g., "BTC-20240315-50000-C").
    pub symbol: String,
    /// Underlying asset.
    pub underlying: UnderlyingAsset,
    /// Option type (Call/Put).
    pub option_type: OptionType,
    /// Strike price.
    pub strike: Strike,
    /// Expiry date and time (UTC).
    pub expiry: DateTime<Utc>,
    /// Exercise style.
    pub exercise_style: ExerciseStyle,
    /// Settlement currency symbol (e.g., "USDT").
    pub settlement_currency: String,
    /// Contract size (units of underlying per contract).
    pub contract_size: f64,
    /// Minimum tick size for price.
    pub tick_size: f64,
    /// Minimum order size in contracts.
    pub min_order_size: u64,
    /// Current status.
    pub status: InstrumentStatus,
    /// When the instrument was created.
    pub created_at: DateTime<Utc>,
    /// When the instrument was last updated.
    pub updated_at: DateTime<Utc>,
}

impl OptionInstrument {
    /// Generate the standard symbol format: ASSET-YYYYMMDD-STRIKE-TYPE
    /// Example: BTC-20240315-50000-C
    pub fn generate_symbol(
        underlying: &str,
        expiry: DateTime<Utc>,
        strike: &Strike,
        option_type: OptionType,
    ) -> String {
        format!(
            "{}-{}-{}-{}",
            underlying,
            expiry.format("%Y%m%d"),
            strike.to_symbol_string(),
            option_type.code()
        )
    }

    /// Parse a symbol string into components.
    /// Returns (underlying, expiry_str, strike_str, option_type_code)
    pub fn parse_symbol(symbol: &str) -> Option<(String, String, String, String)> {
        let parts: Vec<&str> = symbol.split('-').collect();
        if parts.len() != 4 {
            return None;
        }
        Some((
            parts[0].to_string(),
            parts[1].to_string(),
            parts[2].to_string(),
            parts[3].to_string(),
        ))
    }

    /// Check if the instrument has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expiry
    }

    /// Check if the instrument is tradeable.
    pub fn is_tradeable(&self) -> bool {
        self.status == InstrumentStatus::Active && !self.is_expired()
    }

    /// Get time to expiry in seconds.
    pub fn time_to_expiry_seconds(&self) -> i64 {
        (self.expiry - Utc::now()).num_seconds()
    }

    /// Get time to expiry in days (fractional).
    pub fn time_to_expiry_days(&self) -> f64 {
        self.time_to_expiry_seconds() as f64 / 86400.0
    }

    /// Get time to expiry in years (for Black-Scholes).
    pub fn time_to_expiry_years(&self) -> f64 {
        self.time_to_expiry_days() / 365.0
    }
}

impl fmt::Display for OptionInstrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_instrument_id() {
        let id = InstrumentId::new("test-123");
        assert_eq!(id.as_str(), "test-123");
        assert_eq!(id.to_string(), "test-123");

        let generated = InstrumentId::generate();
        assert!(!generated.as_str().is_empty());
    }

    #[test]
    fn test_option_type() {
        assert_eq!(OptionType::Call.code(), "C");
        assert_eq!(OptionType::Put.code(), "P");
        assert_eq!(OptionType::Call.to_string(), "Call");
        assert_eq!(OptionType::Put.to_string(), "Put");
    }

    #[test]
    fn test_strike() {
        let strike = Strike::new(50000.0, 2);
        assert_eq!(strike.value(), 50000.0);
        assert_eq!(strike.decimals(), 2);
        assert_eq!(strike.to_string_with_precision(), "50000.00");
        assert_eq!(strike.to_symbol_string(), "50000");

        let strike_decimal = Strike::new(50000.5, 2);
        assert_eq!(strike_decimal.to_symbol_string(), "50000.5");
    }

    #[test]
    fn test_generate_symbol() {
        let expiry = Utc.with_ymd_and_hms(2024, 3, 15, 8, 0, 0).unwrap();
        let strike = Strike::new(50000.0, 2);

        let call_symbol = OptionInstrument::generate_symbol("BTC", expiry, &strike, OptionType::Call);
        assert_eq!(call_symbol, "BTC-20240315-50000-C");

        let put_symbol = OptionInstrument::generate_symbol("BTC", expiry, &strike, OptionType::Put);
        assert_eq!(put_symbol, "BTC-20240315-50000-P");
    }

    #[test]
    fn test_parse_symbol() {
        let parsed = OptionInstrument::parse_symbol("BTC-20240315-50000-C");
        assert!(parsed.is_some());
        let (underlying, expiry, strike, opt_type) = parsed.unwrap();
        assert_eq!(underlying, "BTC");
        assert_eq!(expiry, "20240315");
        assert_eq!(strike, "50000");
        assert_eq!(opt_type, "C");

        // Invalid symbols
        assert!(OptionInstrument::parse_symbol("BTC-20240315-50000").is_none());
        assert!(OptionInstrument::parse_symbol("INVALID").is_none());
    }

    #[test]
    fn test_instrument_status() {
        assert_eq!(InstrumentStatus::Active.to_string(), "Active");
        assert_eq!(InstrumentStatus::Expired.to_string(), "Expired");
    }
}
