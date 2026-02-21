//! Grid-based strike generation.
//!
//! Generates strikes on a fixed grid (e.g., every $1000 for BTC) within
//! configurable bounds above and below a reference point. Supports
//! displacement triggers that extend the range when spot price moves.

use crate::types::Strike;
use config::AssetGenerationConfig;
use tracing::{debug, info};

/// Grid-based strike price generator.
///
/// Unlike percentage-based generation, this produces strikes at fixed dollar
/// increments (grid_size) within a bounded range around a reference point.
///
/// # Example
///
/// ```ignore
/// // BTC at $50,000 with $1000 grid, $20,000 bounds
/// let config = AssetGenerationConfig {
///     grid_size: 1000.0,
///     upper_bound: 20000.0,
///     lower_bound: 20000.0,
///     upper_disp: 15000.0,
///     lower_disp: 15000.0,
/// };
///
/// let strikes = GridStrikeGenerator::generate_strikes(50000.0, &config, 2);
/// // Produces: $30,000, $31,000, $32,000, ..., $70,000 (41 strikes)
/// ```
pub struct GridStrikeGenerator;

impl GridStrikeGenerator {
    /// Generate all strikes on the grid between min_strike and max_strike.
    ///
    /// # Arguments
    /// * `reference_price` - The reference point (spot price, snapped to grid)
    /// * `config` - Asset generation configuration
    /// * `price_decimals` - Number of decimal places for display
    ///
    /// # Returns
    /// A sorted vector of Strike values.
    pub fn generate_strikes(
        reference_price: f64,
        config: &AssetGenerationConfig,
        price_decimals: u32,
    ) -> Vec<Strike> {
        let grid_size = config.grid_size;

        // Snap reference to grid
        let snapped_ref = Self::snap_to_grid(reference_price, grid_size);

        // Calculate range
        let min_strike = snapped_ref - config.lower_bound;
        let max_strike = snapped_ref + config.upper_bound;

        Self::generate_strikes_in_range(min_strike, max_strike, grid_size, price_decimals)
    }

    /// Generate strikes between min and max at grid_size increments.
    pub fn generate_strikes_in_range(
        min_strike: f64,
        max_strike: f64,
        grid_size: f64,
        price_decimals: u32,
    ) -> Vec<Strike> {
        let mut strikes = Vec::new();

        if grid_size <= 0.0 || min_strike >= max_strike {
            return strikes;
        }

        // Start from min_strike, snapped to grid
        let start = Self::snap_to_grid_ceil(min_strike, grid_size);
        let mut current = start;

        while current <= max_strike {
            if current > 0.0 {
                strikes.push(Strike::new(current, price_decimals));
            }
            current += grid_size;
            // Prevent infinite loop from floating point issues
            if strikes.len() > 10000 {
                break;
            }
        }

        debug!(
            "Generated {} grid strikes from {} to {} (grid_size={})",
            strikes.len(),
            min_strike,
            max_strike,
            grid_size
        );

        strikes
    }

    /// Generate only the NEW strikes needed when a displacement trigger is crossed.
    ///
    /// When the upper trigger is crossed:
    ///   - old_max_strike is the previous maximum
    ///   - new_max_strike is the new maximum (new_reference + upper_bound)
    ///   - Returns strikes from (old_max + grid_size) to new_max
    ///
    /// When the lower trigger is crossed:
    ///   - old_min_strike is the previous minimum
    ///   - new_min_strike is the new minimum (new_reference - lower_bound)
    ///   - Returns strikes from new_min to (old_min - grid_size)
    pub fn generate_extension_strikes(
        old_boundary: f64,
        new_boundary: f64,
        grid_size: f64,
        price_decimals: u32,
        extending_upward: bool,
    ) -> Vec<Strike> {
        if extending_upward {
            // Generate from old_max + grid_size to new_max
            let start = old_boundary + grid_size;
            Self::generate_strikes_in_range(start, new_boundary, grid_size, price_decimals)
        } else {
            // Generate from new_min to old_min - grid_size
            let end = old_boundary - grid_size;
            Self::generate_strikes_in_range(new_boundary, end, grid_size, price_decimals)
        }
    }

    /// Calculate the initial generation state for an asset.
    ///
    /// Returns (min_strike, max_strike, upper_trigger, lower_trigger)
    pub fn calculate_initial_bounds(
        spot_price: f64,
        config: &AssetGenerationConfig,
    ) -> (f64, f64, f64, f64) {
        let snapped = Self::snap_to_grid(spot_price, config.grid_size);
        let min_strike = snapped - config.lower_bound;
        let max_strike = snapped + config.upper_bound;
        let upper_trigger = snapped + config.upper_disp;
        let lower_trigger = snapped - config.lower_disp;

        info!(
            "Initial bounds: snapped_ref={}, range=[{}, {}], triggers=[{}, {}]",
            snapped, min_strike, max_strike, lower_trigger, upper_trigger
        );

        (min_strike, max_strike, upper_trigger, lower_trigger)
    }

    /// Calculate new bounds after an upper displacement trigger is crossed.
    ///
    /// The new reference point becomes the trigger value that was crossed.
    ///
    /// Returns (new_max_strike, new_upper_trigger, new_reference)
    pub fn calculate_upper_displacement(
        crossed_trigger: f64,
        config: &AssetGenerationConfig,
    ) -> (f64, f64, f64) {
        let new_reference = crossed_trigger;
        let new_max_strike = new_reference + config.upper_bound;
        let new_upper_trigger = new_reference + config.upper_disp;

        info!(
            "Upper displacement: new_ref={}, new_max={}, new_trigger={}",
            new_reference, new_max_strike, new_upper_trigger
        );

        (new_max_strike, new_upper_trigger, new_reference)
    }

    /// Calculate new bounds after a lower displacement trigger is crossed.
    ///
    /// Returns (new_min_strike, new_lower_trigger, new_reference)
    pub fn calculate_lower_displacement(
        crossed_trigger: f64,
        config: &AssetGenerationConfig,
    ) -> (f64, f64, f64) {
        let new_reference = crossed_trigger;
        let new_min_strike = new_reference - config.lower_bound;
        let new_lower_trigger = new_reference - config.lower_disp;

        info!(
            "Lower displacement: new_ref={}, new_min={}, new_trigger={}",
            new_reference, new_min_strike, new_lower_trigger
        );

        (new_min_strike, new_lower_trigger, new_reference)
    }

    /// Calculate the active strike range based on current spot price.
    /// Instruments within this range are active; outside are inactive.
    pub fn calculate_active_range(
        spot_price: f64,
        config: &AssetGenerationConfig,
    ) -> (f64, f64) {
        let snapped = Self::snap_to_grid(spot_price, config.grid_size);
        let active_min = snapped - config.lower_bound;
        let active_max = snapped + config.upper_bound;
        (active_min, active_max)
    }

    /// Snap a price to the nearest grid point (rounds to nearest).
    pub fn snap_to_grid(price: f64, grid_size: f64) -> f64 {
        if grid_size <= 0.0 {
            return price;
        }
        (price / grid_size).round() * grid_size
    }

    /// Snap a price to the nearest grid point (rounds up / ceiling).
    fn snap_to_grid_ceil(price: f64, grid_size: f64) -> f64 {
        if grid_size <= 0.0 {
            return price;
        }
        (price / grid_size).ceil() * grid_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn btc_config() -> AssetGenerationConfig {
        AssetGenerationConfig {
            grid_size: 1000.0,
            upper_bound: 20000.0,
            lower_bound: 20000.0,
            upper_disp: 15000.0,
            lower_disp: 15000.0,
        }
    }

    fn eth_config() -> AssetGenerationConfig {
        AssetGenerationConfig {
            grid_size: 100.0,
            upper_bound: 1000.0,
            lower_bound: 1000.0,
            upper_disp: 800.0,
            lower_disp: 800.0,
        }
    }

    #[test]
    fn test_snap_to_grid() {
        assert_eq!(GridStrikeGenerator::snap_to_grid(50123.0, 1000.0), 50000.0);
        assert_eq!(GridStrikeGenerator::snap_to_grid(50500.0, 1000.0), 51000.0);
        assert_eq!(GridStrikeGenerator::snap_to_grid(50499.0, 1000.0), 50000.0);
        assert_eq!(GridStrikeGenerator::snap_to_grid(3050.0, 100.0), 3100.0);
        assert_eq!(GridStrikeGenerator::snap_to_grid(3049.0, 100.0), 3000.0);
    }

    #[test]
    fn test_generate_strikes_btc() {
        let config = btc_config();
        let strikes = GridStrikeGenerator::generate_strikes(50000.0, &config, 2);

        // BTC at $50k: range $30k-$70k, grid $1k = 41 strikes
        assert_eq!(strikes.len(), 41);

        // First strike should be $30,000
        assert_eq!(strikes[0].value(), 30000.0);

        // Last strike should be $70,000
        assert_eq!(strikes[40].value(), 70000.0);

        // ATM should be at index 20 = $50,000
        assert_eq!(strikes[20].value(), 50000.0);

        // All should be on grid
        for strike in &strikes {
            assert_eq!(strike.value() % 1000.0, 0.0, "Strike {} not on grid", strike.value());
        }
    }

    #[test]
    fn test_generate_strikes_eth() {
        let config = eth_config();
        let strikes = GridStrikeGenerator::generate_strikes(3000.0, &config, 2);

        // ETH at $3k: range $2k-$4k, grid $100 = 21 strikes
        assert_eq!(strikes.len(), 21);
        assert_eq!(strikes[0].value(), 2000.0);
        assert_eq!(strikes[20].value(), 4000.0);
    }

    #[test]
    fn test_initial_bounds() {
        let config = btc_config();
        let (min, max, upper_trig, lower_trig) =
            GridStrikeGenerator::calculate_initial_bounds(50000.0, &config);

        assert_eq!(min, 30000.0);
        assert_eq!(max, 70000.0);
        assert_eq!(upper_trig, 65000.0);
        assert_eq!(lower_trig, 35000.0);
    }

    #[test]
    fn test_upper_displacement() {
        let config = btc_config();

        // Spot crosses $65,000 trigger
        let (new_max, new_upper_trigger, new_ref) =
            GridStrikeGenerator::calculate_upper_displacement(65000.0, &config);

        assert_eq!(new_ref, 65000.0);
        assert_eq!(new_max, 85000.0); // 65k + 20k
        assert_eq!(new_upper_trigger, 80000.0); // 65k + 15k
    }

    #[test]
    fn test_extension_strikes_upward() {
        let config = btc_config();

        // After upper displacement: old max was 70k, new max is 85k
        let new_strikes = GridStrikeGenerator::generate_extension_strikes(
            70000.0,  // old max
            85000.0,  // new max
            config.grid_size,
            2,
            true, // extending upward
        );

        // Should generate $71k, $72k, ..., $85k = 15 new strikes
        assert_eq!(new_strikes.len(), 15);
        assert_eq!(new_strikes[0].value(), 71000.0);
        assert_eq!(new_strikes[14].value(), 85000.0);
    }

    #[test]
    fn test_extension_strikes_downward() {
        let config = btc_config();

        // After lower displacement: old min was 30k, new min is 15k
        let new_strikes = GridStrikeGenerator::generate_extension_strikes(
            30000.0,  // old min
            15000.0,  // new min
            config.grid_size,
            2,
            false, // extending downward
        );

        // Should generate $15k, $16k, ..., $29k = 15 new strikes
        assert_eq!(new_strikes.len(), 15);
        assert_eq!(new_strikes[0].value(), 15000.0);
        assert_eq!(new_strikes[14].value(), 29000.0);
    }

    #[test]
    fn test_spot_not_on_grid() {
        let config = btc_config();
        // Spot at $52,300 - should snap to $52,000
        let strikes = GridStrikeGenerator::generate_strikes(52300.0, &config, 2);

        // Snapped ref: $52,000, range: $32,000 - $72,000 = 41 strikes
        assert_eq!(strikes.len(), 41);
        assert_eq!(strikes[0].value(), 32000.0);
        assert_eq!(strikes[40].value(), 72000.0);
    }

    #[test]
    fn test_active_range() {
        let config = btc_config();
        let (active_min, active_max) =
            GridStrikeGenerator::calculate_active_range(52000.0, &config);

        assert_eq!(active_min, 32000.0);
        assert_eq!(active_max, 72000.0);
    }
}
