use crate::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, info, instrument};

#[instrument(skip(path))]
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<MasterConfig> {
    let path = path.as_ref();
    info!("Loading configuration from: {:?}", path);

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    debug!("Config file content length: {} bytes", content.len());

    // Perform environment variable substitution
    let substituted = substitution::substitute_env_vars(&content)?;
    debug!("Environment variable substitution completed");

    // Parse YAML
    let config: MasterConfig = serde_yaml::from_str(&substituted)
        .with_context(|| "Failed to parse YAML configuration")?;

    info!("Configuration loaded successfully");
    Ok(config)
}

#[instrument]
pub fn generate_default_config() -> MasterConfig {
    use defaults::*;

    MasterConfig {
        exchange: ExchangeConfig {
            name: "My Exchange".to_string(),
            description: "A white-label crypto options exchange".to_string(),
            version: "1.0.0".to_string(),
            mode: ExchangeMode::Virtual,
            trading_hours: default_trading_hours(),
        },
        instrument: InstrumentConfig {
            supported_assets: vec![
                Asset {
                    symbol: "BTC".to_string(),
                    name: "Bitcoin".to_string(),
                    decimals: 8,
                    contract_size: 0.01,
                    min_order_size: 1,
                    tick_size: 0.5,
                    price_decimals: 2,
                    enabled: true,
                },
                Asset {
                    symbol: "ETH".to_string(),
                    name: "Ethereum".to_string(),
                    decimals: 18,
                    contract_size: 0.1,
                    min_order_size: 1,
                    tick_size: 0.1,
                    price_decimals: 2,
                    enabled: true,
                },
            ],
            settlement_currencies: vec![SettlementCurrency {
                symbol: "USDT".to_string(),
                name: "Tether".to_string(),
                decimals: 6,
                enabled: true,
                primary: true,
                chains: vec![],
            }],
            market_data: None,
            expiry_schedule: None,
            storage: None,
            cache: None,
            generation: None,
            worker: None,
            static_prices: None,
        },
        deployment: DeploymentConfig::default(),
        // All optional module configs default to None
        oms: None,
        matching_engine: None,
        risk_engine: None,
        settlement: None,
        wallet: None,
        market_data: None,
        services: None,
        api: None,
        fees: None,
        virtual_trading: None,
        compliance: None,
        monitoring: None,
        security: None,
        features: None,
    }
}

#[instrument]
pub fn save_config<P: AsRef<Path> + std::fmt::Debug>(config: &MasterConfig, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Saving configuration to: {:?}", path);

    let yaml = serde_yaml::to_string(config)
        .with_context(|| "Failed to serialize configuration to YAML")?;

    fs::write(path, yaml)
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    info!("Configuration saved successfully");
    Ok(())
}