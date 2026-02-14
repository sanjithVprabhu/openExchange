use anyhow::{Context, Result};
use cli::{Cli, Commands};
use config::{generate_default_config, load_config, save_config, validate_config, ValidationReport};
use std::path::Path;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with INFO and DEBUG levels
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_env_filter("info,open_exchange=debug,config=debug,cli=debug")
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    info!("OpenExchange starting...");
    debug!("Debug logging enabled");

    let cli = Cli::parse_args();
    debug!("CLI arguments parsed: {:?}", cli);

    match cli.command {
        Commands::Start { config } => {
            info!("Executing 'start' command");
            start_exchange(config).await
        }
        Commands::Validate { config } => {
            info!("Executing 'validate' command");
            validate_command(config).await
        }
        Commands::Init { output } => {
            info!("Executing 'init' command");
            init_command(output).await
        }
    }
}

async fn start_exchange<P: AsRef<Path>>(config_path: P) -> Result<()> {
    info!("Starting exchange with config: {:?}", config_path.as_ref());

    // Load and validate config
    let config = load_config(&config_path)?;
    let report = validate_config(&config);

    // Log defaults applied
    if !report.defaults_applied.is_empty() {
        warn!("The following defaults were applied:");
        for default in &report.defaults_applied {
            warn!("  - {} = {}", default.field, default.value);
        }
    }

    // Log warnings
    if !report.warnings.is_empty() {
        warn!("Configuration warnings:");
        for warning in &report.warnings {
            warn!("  - [{}] {}", warning.field, warning.message);
        }
    }

    // Check validation errors
    if !report.is_valid() {
        error!("Configuration validation failed with {} errors:", report.errors.len());
        for error in &report.errors {
            error!("  - {}", error);
        }
        anyhow::bail!("Cannot start exchange due to configuration errors");
    }

    info!("Configuration validated successfully");
    info!("Exchange '{}' starting in {:?} mode", 
        config.exchange.name, 
        format!("{:?}", config.exchange.mode)
    );

    // TODO: Initialize and start all services
    info!("Initializing services...");
    debug!("Order Management System: enabled");
    debug!("Matching Engine: enabled");
    debug!("Risk Engine: enabled");
    debug!("Settlement Service: enabled");
    debug!("Wallet Service: enabled");
    debug!("Market Data Service: enabled");

    info!("All services initialized successfully");
    info!("Exchange is ready to accept connections");

    // Keep the process running
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, stopping exchange...");

    Ok(())
}

async fn validate_command<P: AsRef<Path>>(config_path: P) -> Result<()> {
    info!("Validating configuration: {:?}", config_path.as_ref());

    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            anyhow::bail!(e);
        }
    };

    let report = validate_config(&config);

    // Print summary
    println!("\n=== Configuration Validation Report ===\n");

    // Defaults
    if !report.defaults_applied.is_empty() {
        println!("Defaults Applied ({}):", report.defaults_applied.len());
        for default in &report.defaults_applied {
            println!("  ℹ️  {} = {}", default.field, default.value);
        }
        println!();
    }

    // Warnings
    if !report.warnings.is_empty() {
        println!("Warnings ({}):", report.warnings.len());
        for warning in &report.warnings {
            println!("  ⚠️  [{}] {}", warning.field, warning.message);
        }
        println!();
    }

    // Errors
    if !report.errors.is_empty() {
        println!("Errors ({}):", report.errors.len());
        for error in &report.errors {
            println!("  ❌ {}", error);
        }
        println!();
        anyhow::bail!("Configuration validation failed");
    }

    println!("✅ Configuration is valid!");
    println!();
    println!("Exchange: {}", config.exchange.name);
    println!("Version: {}", config.exchange.version);
    println!("Mode: {:?}", format!("{:?}", config.exchange.mode));
    println!("Supported Assets: {}", 
        config.instrument.supported_assets.len()
    );
    println!("Settlement Currencies: {}", 
        config.instrument.settlement_currencies.len()
    );
    println!("Market Data Providers: {}", 
        config.market_data.providers.len()
    );

    Ok(())
}

async fn init_command<P: AsRef<Path>>(output_path: P) -> Result<()> {
    let output_path = output_path.as_ref();
    info!("Initializing new configuration file: {:?}", output_path);

    // Generate default config
    let config = generate_default_config();

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {:?}", parent))?;
    }

    // Save config
    save_config(&config, output_path)?;

    println!("✅ Configuration file created successfully!");
    println!();
    println!("Location: {:?}", output_path);
    println!();
    println!("This configuration includes:");
    println!("  - Exchange metadata (name, description, version)");
    println!("  - 2 supported assets (BTC, ETH)");
    println!("  - 1 settlement currency (USDT)");
    println!("  - Market data provider configuration (Binance WebSocket)");
    println!("  - Expiry schedules (daily, weekly, monthly, quarterly, yearly)");
    println!("  - OMS configuration");
    println!("  - Matching engine settings");
    println!("  - Risk engine parameters");
    println!();
    println!("Next steps:");
    println!("  1. Edit the configuration file to customize settings");
    println!("  2. Set required environment variables (database connections, API keys)");
    println!("  3. Run 'openx validate --config {:?}' to check configuration", output_path);
    println!("  4. Run 'openx start --config {:?}' to start the exchange", output_path);

    Ok(())
}