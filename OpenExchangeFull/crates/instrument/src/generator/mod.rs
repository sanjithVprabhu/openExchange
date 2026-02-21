//! Instrument generation from configuration.
//!
//! This module generates option instruments based on:
//! - Supported assets from config
//! - Expiry schedules (daily, weekly, monthly, quarterly, yearly)
//! - Grid-based strike generation with displacement triggers
//! - Strike generation around spot price

pub mod grid;
pub mod state;

pub use grid::GridStrikeGenerator;
pub use state::GenerationStateManager;

use crate::error::{InstrumentError, InstrumentResult};
use crate::types::{
    ExerciseStyle, InstrumentId, InstrumentStatus, OptionInstrument, OptionType, Strike,
    UnderlyingAsset,
};
use chrono::{DateTime, Datelike, Duration, NaiveTime, Utc, Weekday};
use config::{Asset, ExpiryConfig, ExpirySchedule, InstrumentConfig};
use tracing::{debug, info, instrument};

/// Generates expiry dates from expiry schedule configuration.
pub struct ExpiryGenerator;

impl ExpiryGenerator {
    /// Generate all expiry dates from the config.
    pub fn generate_expiries(config: &ExpiryConfig) -> Vec<DateTime<Utc>> {
        let mut expiries = Vec::new();
        let now = Utc::now();

        // Daily expiries
        if config.daily.enabled {
            expiries.extend(Self::generate_daily_expiries(&config.daily, now));
        }

        // Weekly expiries
        if config.weekly.enabled {
            expiries.extend(Self::generate_weekly_expiries(&config.weekly, now));
        }

        // Monthly expiries
        if config.monthly.enabled {
            expiries.extend(Self::generate_monthly_expiries(&config.monthly, now));
        }

        // Quarterly expiries
        if config.quarterly.enabled {
            expiries.extend(Self::generate_quarterly_expiries(&config.quarterly, now));
        }

        // Yearly expiries
        if config.yearly.enabled {
            expiries.extend(Self::generate_yearly_expiries(&config.yearly, now));
        }

        // Sort and deduplicate
        expiries.sort();
        expiries.dedup();

        // Filter out any expiries in the past
        expiries.retain(|e| *e > now);

        expiries
    }

    /// Generate daily expiry dates.
    fn generate_daily_expiries(schedule: &ExpirySchedule, now: DateTime<Utc>) -> Vec<DateTime<Utc>> {
        let count = schedule.count.unwrap_or(7) as i64;
        let expiry_time = Self::parse_expiry_time(&schedule.expiry_time_utc);

        (1..=count)
            .filter_map(|day| {
                let date = (now + Duration::days(day)).date_naive();
                date.and_time(expiry_time)
                    .and_local_timezone(Utc)
                    .single()
            })
            .collect()
    }

    /// Generate weekly expiry dates (default: Fridays).
    fn generate_weekly_expiries(
        schedule: &ExpirySchedule,
        now: DateTime<Utc>,
    ) -> Vec<DateTime<Utc>> {
        let count = schedule.count.unwrap_or(4);
        let expiry_time = Self::parse_expiry_time(&schedule.expiry_time_utc);
        let target_day = Self::parse_weekday(schedule.day_of_week.as_deref().unwrap_or("Friday"));

        let mut expiries = Vec::new();
        let mut current = now;

        while expiries.len() < count as usize {
            // Find next target day
            current += Duration::days(1);
            if current.weekday() == target_day {
                if let Some(expiry) = current
                    .date_naive()
                    .and_time(expiry_time)
                    .and_local_timezone(Utc)
                    .single()
                {
                    if expiry > now {
                        expiries.push(expiry);
                    }
                }
            }
        }

        expiries
    }

    /// Generate monthly expiry dates (default: last Friday of month).
    fn generate_monthly_expiries(
        schedule: &ExpirySchedule,
        now: DateTime<Utc>,
    ) -> Vec<DateTime<Utc>> {
        let count = schedule.count.unwrap_or(6);
        let expiry_time = Self::parse_expiry_time(&schedule.expiry_time_utc);
        let day_type = schedule.day_type.as_deref().unwrap_or("last_friday");

        let mut expiries = Vec::new();
        let mut year = now.year();
        let mut month = now.month();

        while expiries.len() < count as usize {
            if let Some(date) = Self::find_day_in_month(year, month, day_type) {
                if let Some(expiry) = date.and_time(expiry_time).and_local_timezone(Utc).single() {
                    if expiry > now {
                        expiries.push(expiry);
                    }
                }
            }

            // Move to next month
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }

        expiries
    }

    /// Generate quarterly expiry dates.
    fn generate_quarterly_expiries(
        schedule: &ExpirySchedule,
        now: DateTime<Utc>,
    ) -> Vec<DateTime<Utc>> {
        let expiry_time = Self::parse_expiry_time(&schedule.expiry_time_utc);
        let day_type = schedule.day_type.as_deref().unwrap_or("last_friday");
        let months = schedule
            .months
            .clone()
            .unwrap_or_else(|| vec![3, 6, 9, 12]);

        let mut expiries = Vec::new();
        let year = now.year();

        // Generate for current year and next year
        for y in [year, year + 1] {
            for &month in &months {
                if let Some(date) = Self::find_day_in_month(y, month, day_type) {
                    if let Some(expiry) = date.and_time(expiry_time).and_local_timezone(Utc).single()
                    {
                        if expiry > now {
                            expiries.push(expiry);
                        }
                    }
                }
            }
        }

        expiries
    }

    /// Generate yearly expiry dates.
    fn generate_yearly_expiries(
        schedule: &ExpirySchedule,
        now: DateTime<Utc>,
    ) -> Vec<DateTime<Utc>> {
        let expiry_time = Self::parse_expiry_time(&schedule.expiry_time_utc);
        let day_type = schedule.day_type.as_deref().unwrap_or("last_friday");
        let month = schedule.month.unwrap_or(12);

        let mut expiries = Vec::new();
        let year = now.year();

        // Generate for current and next few years
        for y in [year, year + 1, year + 2] {
            if let Some(date) = Self::find_day_in_month(y, month, day_type) {
                if let Some(expiry) = date.and_time(expiry_time).and_local_timezone(Utc).single() {
                    if expiry > now {
                        expiries.push(expiry);
                    }
                }
            }
        }

        expiries
    }

    /// Parse expiry time string (e.g., "08:00") to NaiveTime.
    fn parse_expiry_time(time_str: &str) -> NaiveTime {
        NaiveTime::parse_from_str(time_str, "%H:%M").unwrap_or(NaiveTime::from_hms_opt(8, 0, 0).unwrap())
    }

    /// Parse weekday string to Weekday enum.
    fn parse_weekday(day: &str) -> Weekday {
        match day.to_lowercase().as_str() {
            "monday" | "mon" => Weekday::Mon,
            "tuesday" | "tue" => Weekday::Tue,
            "wednesday" | "wed" => Weekday::Wed,
            "thursday" | "thu" => Weekday::Thu,
            "friday" | "fri" => Weekday::Fri,
            "saturday" | "sat" => Weekday::Sat,
            "sunday" | "sun" => Weekday::Sun,
            _ => Weekday::Fri, // Default to Friday
        }
    }

    /// Find a specific day in a month (e.g., "last_friday", "third_friday").
    fn find_day_in_month(year: i32, month: u32, day_type: &str) -> Option<chrono::NaiveDate> {
        use chrono::NaiveDate;

        let first_of_month = NaiveDate::from_ymd_opt(year, month, 1)?;
        let days_in_month = Self::days_in_month(year, month);

        match day_type {
            "last_friday" => {
                // Find last Friday of the month
                let last_day = NaiveDate::from_ymd_opt(year, month, days_in_month)?;
                let days_from_friday = (last_day.weekday().num_days_from_monday() + 7 - 4) % 7;
                Some(last_day - Duration::days(days_from_friday as i64))
            }
            "third_friday" => {
                // Find third Friday of the month
                let days_to_friday = (Weekday::Fri.num_days_from_monday() as i32
                    - first_of_month.weekday().num_days_from_monday() as i32
                    + 7)
                    % 7;
                let first_friday = first_of_month + Duration::days(days_to_friday as i64);
                Some(first_friday + Duration::weeks(2)) // Third Friday
            }
            "last_day" => NaiveDate::from_ymd_opt(year, month, days_in_month),
            "first_day" => Some(first_of_month),
            _ => {
                // Try to parse as "last_friday" by default
                Self::find_day_in_month(year, month, "last_friday")
            }
        }
    }

    /// Get number of days in a month.
    fn days_in_month(year: i32, month: u32) -> u32 {
        use chrono::NaiveDate;
        if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        }
        .map(|d| d.pred_opt().unwrap().day())
        .unwrap_or(30)
    }
}

/// Generates strike prices around a spot price.
pub struct StrikeGenerator;

impl StrikeGenerator {
    /// Generate strike prices around a spot price.
    ///
    /// # Arguments
    /// * `spot_price` - Current spot price of the underlying
    /// * `tick_size` - Minimum price increment
    /// * `price_decimals` - Number of decimal places for price
    /// * `num_strikes` - Number of strikes above and below spot (total = 2*num + 1)
    /// * `strike_interval_pct` - Interval between strikes as percentage of spot (e.g., 0.05 for 5%)
    pub fn generate_strikes(
        spot_price: f64,
        tick_size: f64,
        price_decimals: u32,
        num_strikes: usize,
        strike_interval_pct: f64,
    ) -> Vec<Strike> {
        let mut strikes = Vec::new();

        // Round spot price to nearest tick
        let atm_strike = Self::round_to_tick(spot_price, tick_size);
        let interval = spot_price * strike_interval_pct;
        let interval = Self::round_to_tick(interval, tick_size).max(tick_size);

        // Generate strikes below ATM
        for i in (1..=num_strikes).rev() {
            let strike_value = atm_strike - (i as f64 * interval);
            if strike_value > 0.0 {
                strikes.push(Strike::new(
                    Self::round_to_tick(strike_value, tick_size),
                    price_decimals,
                ));
            }
        }

        // ATM strike
        strikes.push(Strike::new(atm_strike, price_decimals));

        // Generate strikes above ATM
        for i in 1..=num_strikes {
            let strike_value = atm_strike + (i as f64 * interval);
            strikes.push(Strike::new(
                Self::round_to_tick(strike_value, tick_size),
                price_decimals,
            ));
        }

        strikes
    }

    /// Generate standard strike prices for an asset based on typical market conventions.
    pub fn generate_standard_strikes(
        spot_price: f64,
        asset_symbol: &str,
        price_decimals: u32,
    ) -> Vec<Strike> {
        // Different assets have different strike conventions
        let (interval_pct, num_strikes) = match asset_symbol {
            "BTC" => (0.025, 10), // 2.5% intervals, 10 strikes each direction
            "ETH" => (0.03, 10),  // 3% intervals
            "SOL" => (0.05, 8),   // 5% intervals
            _ => (0.05, 8),       // Default: 5% intervals, 8 strikes
        };

        // Determine tick size based on price magnitude
        let tick_size = if spot_price > 10000.0 {
            100.0
        } else if spot_price > 1000.0 {
            10.0
        } else if spot_price > 100.0 {
            1.0
        } else {
            0.1
        };

        Self::generate_strikes(spot_price, tick_size, price_decimals, num_strikes, interval_pct)
    }

    /// Round a value to the nearest tick.
    fn round_to_tick(value: f64, tick_size: f64) -> f64 {
        (value / tick_size).round() * tick_size
    }
}

/// Generates complete option instruments from configuration.
pub struct InstrumentGenerator;

impl InstrumentGenerator {
    /// Generate all instruments for enabled assets based on config.
    ///
    /// # Arguments
    /// * `config` - Instrument configuration
    /// * `spot_prices` - Current spot prices for each asset (symbol -> price)
    /// * `settlement_currency` - Settlement currency symbol (e.g., "USDT")
    #[instrument(skip(config, spot_prices))]
    pub fn generate_instruments(
        config: &InstrumentConfig,
        spot_prices: &std::collections::HashMap<String, f64>,
        settlement_currency: &str,
    ) -> InstrumentResult<Vec<OptionInstrument>> {
        let mut instruments = Vec::new();

        // Get expiry schedule
        let expiry_config = config.expiry_schedule.as_ref().ok_or_else(|| {
            InstrumentError::ConfigError("No expiry_schedule in config".to_string())
        })?;

        // Generate expiry dates
        let expiries = ExpiryGenerator::generate_expiries(expiry_config);
        info!("Generated {} expiry dates", expiries.len());

        // For each enabled asset
        for asset in &config.supported_assets {
            if !asset.enabled {
                debug!("Skipping disabled asset: {}", asset.symbol);
                continue;
            }

            // Get spot price
            let spot_price = spot_prices.get(&asset.symbol).ok_or_else(|| {
                InstrumentError::ConfigError(format!("No spot price for asset: {}", asset.symbol))
            })?;

            info!(
                "Generating instruments for {} at spot price {}",
                asset.symbol, spot_price
            );

            // Generate strikes
            let strikes = StrikeGenerator::generate_standard_strikes(
                *spot_price,
                &asset.symbol,
                asset.price_decimals,
            );

            // Generate instruments for each expiry, strike, and option type
            for expiry in &expiries {
                for strike in &strikes {
                    for option_type in [OptionType::Call, OptionType::Put] {
                        let instrument =
                            Self::create_instrument(asset, *expiry, strike, option_type, settlement_currency);
                        instruments.push(instrument);
                    }
                }
            }
        }

        info!("Generated {} total instruments", instruments.len());
        Ok(instruments)
    }

    /// Create a single option instrument.
    fn create_instrument(
        asset: &Asset,
        expiry: DateTime<Utc>,
        strike: &Strike,
        option_type: OptionType,
        settlement_currency: &str,
    ) -> OptionInstrument {
        let symbol =
            OptionInstrument::generate_symbol(&asset.symbol, expiry, strike, option_type);

        let now = Utc::now();

        OptionInstrument {
            id: InstrumentId::generate(),
            symbol,
            underlying: UnderlyingAsset::from_config(asset),
            option_type,
            strike: *strike,
            expiry,
            exercise_style: ExerciseStyle::European, // All options are European style
            settlement_currency: settlement_currency.to_string(),
            contract_size: asset.contract_size,
            tick_size: asset.tick_size,
            min_order_size: asset.min_order_size,
            status: InstrumentStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// Generate instruments with default spot prices (for testing).
    pub fn generate_with_default_spots(
        config: &InstrumentConfig,
        settlement_currency: &str,
    ) -> InstrumentResult<Vec<OptionInstrument>> {
        // Use placeholder spot prices
        let mut spot_prices = std::collections::HashMap::new();
        spot_prices.insert("BTC".to_string(), 50000.0);
        spot_prices.insert("ETH".to_string(), 3000.0);
        spot_prices.insert("SOL".to_string(), 100.0);

        Self::generate_instruments(config, &spot_prices, settlement_currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{ExpiryConfig, ExpirySchedule};

    fn create_test_expiry_config() -> ExpiryConfig {
        ExpiryConfig {
            daily: ExpirySchedule {
                enabled: true,
                count: Some(3),
                expiry_time_utc: "08:00".to_string(),
                day_of_week: None,
                day_type: None,
                months: None,
                month: None,
            },
            weekly: ExpirySchedule {
                enabled: true,
                count: Some(2),
                expiry_time_utc: "08:00".to_string(),
                day_of_week: Some("Friday".to_string()),
                day_type: None,
                months: None,
                month: None,
            },
            monthly: ExpirySchedule {
                enabled: false,
                count: Some(0),
                expiry_time_utc: "08:00".to_string(),
                day_of_week: None,
                day_type: None,
                months: None,
                month: None,
            },
            quarterly: ExpirySchedule {
                enabled: false,
                count: None,
                expiry_time_utc: "08:00".to_string(),
                day_of_week: None,
                day_type: None,
                months: None,
                month: None,
            },
            yearly: ExpirySchedule {
                enabled: false,
                count: None,
                expiry_time_utc: "08:00".to_string(),
                day_of_week: None,
                day_type: None,
                months: None,
                month: None,
            },
        }
    }

    #[test]
    fn test_generate_daily_expiries() {
        let config = create_test_expiry_config();
        let expiries = ExpiryGenerator::generate_expiries(&config);

        // Should have daily + weekly expiries
        assert!(!expiries.is_empty());

        // All expiries should be in the future
        let now = Utc::now();
        for expiry in &expiries {
            assert!(*expiry > now);
        }
    }

    #[test]
    fn test_generate_strikes() {
        let strikes = StrikeGenerator::generate_strikes(50000.0, 100.0, 2, 5, 0.02);

        // Should have 11 strikes (5 below + ATM + 5 above)
        assert_eq!(strikes.len(), 11);

        // ATM strike should be close to spot
        let atm = &strikes[5];
        assert!((atm.value() - 50000.0).abs() < 100.0);

        // Strikes should be in ascending order
        for i in 1..strikes.len() {
            assert!(strikes[i].value() > strikes[i - 1].value());
        }
    }

    #[test]
    fn test_standard_strikes_btc() {
        let strikes = StrikeGenerator::generate_standard_strikes(50000.0, "BTC", 2);

        // Should have 21 strikes (10 below + ATM + 10 above)
        assert_eq!(strikes.len(), 21);

        // Should be rounded to tick size (100 for BTC at 50k)
        for strike in &strikes {
            assert_eq!(strike.value() % 100.0, 0.0);
        }
    }

    #[test]
    fn test_round_to_tick() {
        assert_eq!(StrikeGenerator::round_to_tick(50123.0, 100.0), 50100.0);
        assert_eq!(StrikeGenerator::round_to_tick(50150.0, 100.0), 50200.0);
        assert_eq!(StrikeGenerator::round_to_tick(3456.7, 10.0), 3460.0);
    }

    #[test]
    fn test_find_last_friday() {
        // December 2024 - last Friday is the 27th
        let last_friday = ExpiryGenerator::find_day_in_month(2024, 12, "last_friday");
        assert!(last_friday.is_some());
        let date = last_friday.unwrap();
        assert_eq!(date.day(), 27);
        assert_eq!(date.weekday(), Weekday::Fri);
    }
}
