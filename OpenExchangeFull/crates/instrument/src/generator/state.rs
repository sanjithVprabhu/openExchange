//! Generation state management.
//!
//! Tracks displacement trigger reference points per asset and manages
//! the lifecycle of generation state initialization and updates.

#[cfg(feature = "postgres")]
use crate::db::models::{Environment, GenerationState};
use crate::generator::grid::GridStrikeGenerator;
use config::AssetGenerationConfig;
use tracing::info;

/// Manages generation state for displacement triggers.
///
/// This struct handles:
/// - Initializing generation state from spot price
/// - Checking if displacement triggers have been crossed
/// - Calculating new bounds after displacement
/// - Persisting state to database
pub struct GenerationStateManager;

/// Result of checking displacement triggers.
#[derive(Debug, Clone)]
pub enum DisplacementResult {
    /// No trigger crossed - no action needed.
    NoChange,
    /// Upper trigger crossed - need to extend strikes upward.
    UpperCrossed {
        new_reference: f64,
        new_max_strike: f64,
        old_max_strike: f64,
        new_upper_trigger: f64,
    },
    /// Lower trigger crossed - need to extend strikes downward.
    LowerCrossed {
        new_reference: f64,
        new_min_strike: f64,
        old_min_strike: f64,
        new_lower_trigger: f64,
    },
}

/// In-memory representation of generation state (usable without postgres).
#[derive(Debug, Clone)]
pub struct GenState {
    pub asset_symbol: String,
    pub upper_reference: f64,
    pub lower_reference: f64,
    pub upper_trigger: f64,
    pub lower_trigger: f64,
    pub max_strike: f64,
    pub min_strike: f64,
    pub last_spot_price: f64,
}

impl GenerationStateManager {
    /// Initialize generation state from a spot price and config.
    /// This is used on first startup when no state exists in the database.
    pub fn initialize(
        asset_symbol: &str,
        spot_price: f64,
        config: &AssetGenerationConfig,
    ) -> GenState {
        let (min_strike, max_strike, upper_trigger, lower_trigger) =
            GridStrikeGenerator::calculate_initial_bounds(spot_price, config);

        let snapped_ref = GridStrikeGenerator::snap_to_grid(spot_price, config.grid_size);

        info!(
            "Initialized generation state for {}: ref={}, range=[{}, {}], triggers=[{}, {}]",
            asset_symbol, snapped_ref, min_strike, max_strike, lower_trigger, upper_trigger
        );

        GenState {
            asset_symbol: asset_symbol.to_string(),
            upper_reference: snapped_ref,
            lower_reference: snapped_ref,
            upper_trigger,
            lower_trigger,
            max_strike,
            min_strike,
            last_spot_price: spot_price,
        }
    }

    /// Check if the current spot price has crossed any displacement triggers.
    pub fn check_displacement(
        spot_price: f64,
        state: &GenState,
        config: &AssetGenerationConfig,
    ) -> DisplacementResult {
        // Check upper displacement
        if spot_price >= state.upper_trigger {
            let (new_max, new_upper_trigger, new_ref) =
                GridStrikeGenerator::calculate_upper_displacement(state.upper_trigger, config);

            info!(
                "{}: Upper displacement triggered! spot={} >= trigger={}. new_ref={}, new_max={}, new_trigger={}",
                state.asset_symbol, spot_price, state.upper_trigger, new_ref, new_max, new_upper_trigger
            );

            return DisplacementResult::UpperCrossed {
                new_reference: new_ref,
                new_max_strike: new_max,
                old_max_strike: state.max_strike,
                new_upper_trigger,
            };
        }

        // Check lower displacement
        if spot_price <= state.lower_trigger {
            let (new_min, new_lower_trigger, new_ref) =
                GridStrikeGenerator::calculate_lower_displacement(state.lower_trigger, config);

            info!(
                "{}: Lower displacement triggered! spot={} <= trigger={}. new_ref={}, new_min={}, new_trigger={}",
                state.asset_symbol, spot_price, state.lower_trigger, new_ref, new_min, new_lower_trigger
            );

            return DisplacementResult::LowerCrossed {
                new_reference: new_ref,
                new_min_strike: new_min,
                old_min_strike: state.min_strike,
                new_lower_trigger,
            };
        }

        DisplacementResult::NoChange
    }

    /// Apply an upper displacement to the state, returning the updated state.
    pub fn apply_upper_displacement(
        state: &GenState,
        new_reference: f64,
        new_max_strike: f64,
        new_upper_trigger: f64,
        spot_price: f64,
    ) -> GenState {
        GenState {
            asset_symbol: state.asset_symbol.clone(),
            upper_reference: new_reference,
            lower_reference: state.lower_reference,
            upper_trigger: new_upper_trigger,
            lower_trigger: state.lower_trigger,
            max_strike: new_max_strike,
            min_strike: state.min_strike,
            last_spot_price: spot_price,
        }
    }

    /// Apply a lower displacement to the state, returning the updated state.
    pub fn apply_lower_displacement(
        state: &GenState,
        new_reference: f64,
        new_min_strike: f64,
        new_lower_trigger: f64,
        spot_price: f64,
    ) -> GenState {
        GenState {
            asset_symbol: state.asset_symbol.clone(),
            upper_reference: state.upper_reference,
            lower_reference: new_reference,
            upper_trigger: state.upper_trigger,
            lower_trigger: new_lower_trigger,
            max_strike: state.max_strike,
            min_strike: new_min_strike,
            last_spot_price: spot_price,
        }
    }

    /// Convert in-memory state to database-compatible format.
    #[cfg(feature = "postgres")]
    pub fn to_db_state(state: &GenState, environment: Environment) -> GenerationState {
        GenerationState {
            environment,
            asset_symbol: state.asset_symbol.clone(),
            upper_reference: state.upper_reference,
            lower_reference: state.lower_reference,
            upper_trigger: state.upper_trigger,
            lower_trigger: state.lower_trigger,
            max_strike: state.max_strike,
            min_strike: state.min_strike,
            last_spot_price: state.last_spot_price,
        }
    }

    /// Convert from database state to in-memory format.
    #[cfg(feature = "postgres")]
    pub fn from_db_state(db_state: &GenerationState) -> GenState {
        GenState {
            asset_symbol: db_state.asset_symbol.clone(),
            upper_reference: db_state.upper_reference,
            lower_reference: db_state.lower_reference,
            upper_trigger: db_state.upper_trigger,
            lower_trigger: db_state.lower_trigger,
            max_strike: db_state.max_strike,
            min_strike: db_state.min_strike,
            last_spot_price: db_state.last_spot_price,
        }
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

    #[test]
    fn test_initialize_state() {
        let config = btc_config();
        let state = GenerationStateManager::initialize("BTC", 50000.0, &config);

        assert_eq!(state.asset_symbol, "BTC");
        assert_eq!(state.upper_reference, 50000.0);
        assert_eq!(state.lower_reference, 50000.0);
        assert_eq!(state.upper_trigger, 65000.0);
        assert_eq!(state.lower_trigger, 35000.0);
        assert_eq!(state.max_strike, 70000.0);
        assert_eq!(state.min_strike, 30000.0);
        assert_eq!(state.last_spot_price, 50000.0);
    }

    #[test]
    fn test_no_displacement() {
        let config = btc_config();
        let state = GenerationStateManager::initialize("BTC", 50000.0, &config);

        // Spot moved to $55k, no trigger crossed
        let result = GenerationStateManager::check_displacement(55000.0, &state, &config);
        assert!(matches!(result, DisplacementResult::NoChange));
    }

    #[test]
    fn test_upper_displacement() {
        let config = btc_config();
        let state = GenerationStateManager::initialize("BTC", 50000.0, &config);

        // Spot crosses $65k upper trigger
        let result = GenerationStateManager::check_displacement(66000.0, &state, &config);

        match result {
            DisplacementResult::UpperCrossed {
                new_reference,
                new_max_strike,
                old_max_strike,
                new_upper_trigger,
            } => {
                assert_eq!(new_reference, 65000.0); // trigger value, not spot
                assert_eq!(new_max_strike, 85000.0); // 65k + 20k
                assert_eq!(old_max_strike, 70000.0);
                assert_eq!(new_upper_trigger, 80000.0); // 65k + 15k
            }
            _ => panic!("Expected UpperCrossed"),
        }
    }

    #[test]
    fn test_lower_displacement() {
        let config = btc_config();
        let state = GenerationStateManager::initialize("BTC", 50000.0, &config);

        // Spot drops below $35k lower trigger
        let result = GenerationStateManager::check_displacement(34000.0, &state, &config);

        match result {
            DisplacementResult::LowerCrossed {
                new_reference,
                new_min_strike,
                old_min_strike,
                new_lower_trigger,
            } => {
                assert_eq!(new_reference, 35000.0); // trigger value, not spot
                assert_eq!(new_min_strike, 15000.0); // 35k - 20k
                assert_eq!(old_min_strike, 30000.0);
                assert_eq!(new_lower_trigger, 20000.0); // 35k - 15k
            }
            _ => panic!("Expected LowerCrossed"),
        }
    }

    #[test]
    fn test_sequential_upper_displacements() {
        let config = btc_config();
        let state = GenerationStateManager::initialize("BTC", 50000.0, &config);

        // First upper displacement at $65k
        let result = GenerationStateManager::check_displacement(66000.0, &state, &config);
        let state = match result {
            DisplacementResult::UpperCrossed {
                new_reference,
                new_max_strike,
                new_upper_trigger,
                ..
            } => GenerationStateManager::apply_upper_displacement(
                &state,
                new_reference,
                new_max_strike,
                new_upper_trigger,
                66000.0,
            ),
            _ => panic!("Expected UpperCrossed"),
        };

        // State should be updated
        assert_eq!(state.upper_trigger, 80000.0);
        assert_eq!(state.max_strike, 85000.0);
        assert_eq!(state.upper_reference, 65000.0);

        // Second upper displacement at $80k
        let result = GenerationStateManager::check_displacement(82000.0, &state, &config);
        match result {
            DisplacementResult::UpperCrossed {
                new_reference,
                new_max_strike,
                new_upper_trigger,
                ..
            } => {
                assert_eq!(new_reference, 80000.0);
                assert_eq!(new_max_strike, 100000.0); // 80k + 20k
                assert_eq!(new_upper_trigger, 95000.0); // 80k + 15k
            }
            _ => panic!("Expected UpperCrossed"),
        }
    }
}
