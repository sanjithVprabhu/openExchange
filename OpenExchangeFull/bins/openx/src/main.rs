//! OpenExchange CLI and Server Binary
//!
//! This is the main entry point for the OpenExchange application.
//! It provides commands for initializing, validating, and starting
//! the exchange.

use anyhow::{Context, Result};
use cli::{Cli, Commands, DeploymentMode};
use config::{generate_default_config, load_config, save_config, validate_config};
use observability::{init_logging, LogFormat};
use server::{ports, CombinedServer, ServerConfig, ServerExt};
use std::path::Path;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging("openx", LogFormat::Pretty)?;

    info!("OpenExchange starting...");
    debug!("Debug logging enabled");

    let cli = Cli::parse_args();
    debug!(?cli, "CLI arguments parsed");

    match cli.command {
        Commands::Start {
            mode,
            config,
            http,
            grpc,
            ws,
        } => {
            info!("Executing 'start' command");
            start_exchange(mode, config, http, grpc, ws).await
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

/// Get default ports for each deployment mode
fn get_default_ports(mode: &DeploymentMode) -> (u16, u16, u16) {
    match mode {
        DeploymentMode::Monolith | DeploymentMode::Gateway => {
            (ports::GATEWAY_HTTP, ports::GATEWAY_GRPC, ports::GATEWAY_WS)
        }
        DeploymentMode::Instrument => (
            ports::INSTRUMENT_HTTP,
            ports::INSTRUMENT_GRPC,
            ports::INSTRUMENT_WS,
        ),
        DeploymentMode::Oms => (ports::OMS_HTTP, ports::OMS_GRPC, ports::OMS_WS),
        DeploymentMode::Matching => (
            ports::MATCHING_HTTP,
            ports::MATCHING_GRPC,
            ports::MATCHING_WS,
        ),
        DeploymentMode::Wallet => (ports::WALLET_HTTP, ports::WALLET_GRPC, ports::WALLET_WS),
        DeploymentMode::Settlement => (
            ports::SETTLEMENT_HTTP,
            ports::SETTLEMENT_GRPC,
            ports::SETTLEMENT_WS,
        ),
        DeploymentMode::Risk => (ports::RISK_HTTP, ports::RISK_GRPC, ports::RISK_WS),
        DeploymentMode::Market => (ports::MARKET_HTTP, ports::MARKET_GRPC, ports::MARKET_WS),
    }
}

async fn start_exchange<P: AsRef<Path>>(
    mode: DeploymentMode,
    config_path: P,
    http_override: Option<u16>,
    grpc_override: Option<u16>,
    ws_override: Option<u16>,
) -> Result<()> {
    let config_path = config_path.as_ref();

    // Check if using default config (no port overrides)
    let using_default_config =
        http_override.is_none() && grpc_override.is_none() && ws_override.is_none();

    if using_default_config {
        println!("Starting in {} mode with default ports", mode.as_str());
    }

    // Load and validate config
    let config = load_config(config_path)?;
    let report = validate_config(&config);

    // Log warnings
    if !report.warnings.is_empty() {
        warn!("Configuration warnings:");
        for warning in &report.warnings {
            warn!(field = %warning.field, message = %warning.message);
        }
    }

    // Check validation errors
    if !report.is_valid() {
        error!(
            error_count = report.errors.len(),
            "Configuration validation failed"
        );
        for err in &report.errors {
            error!("{}", err);
        }
        anyhow::bail!("Cannot start exchange due to configuration errors");
    }

    // Get default ports for this mode
    let (default_http, default_grpc, default_ws) = get_default_ports(&mode);

    // Apply CLI overrides or use defaults
    let http_port = http_override.unwrap_or(default_http);
    let grpc_port = grpc_override.unwrap_or(default_grpc);
    let ws_port = ws_override.unwrap_or(default_ws);

    let mode_name = mode.as_str();

    // Log port information
    if using_default_config {
        println!(
            "Using default ports: HTTP={}, gRPC={}, WebSocket={}",
            http_port, grpc_port, ws_port
        );
    } else {
        if http_override.is_none() {
            debug!(port = default_http, "Using default HTTP port");
        }
        if grpc_override.is_none() {
            debug!(port = default_grpc, "Using default gRPC port");
        }
        if ws_override.is_none() {
            debug!(port = default_ws, "Using default WebSocket port");
        }
    }

    info!(
        mode = mode_name,
        http_port, grpc_port, ws_port, "Starting exchange"
    );

    // Start service
    start_service_with_ports(mode_name, http_port, grpc_port, ws_port).await
}

async fn start_service_with_ports(
    service_name: &str,
    http_port: u16,
    grpc_port: u16,
    ws_port: u16,
) -> Result<()> {
    info!(
        service = service_name,
        http_port, grpc_port, ws_port, "Starting service"
    );

    // Create server config with the specified ports
    let server_config = ServerConfig {
        host: "0.0.0.0".to_string(),
        http_port: Some(http_port),
        grpc_port: Some(grpc_port),
        websocket_port: Some(ws_port),
    };

    // Create server with custom config
    let server = CombinedServer::ping_server_with_config(service_name, server_config);

    // Validate ports
    server.validate_ports().await?;

    // Start server with graceful shutdown (Ctrl+C handling)
    server.run_with_ctrl_c().await?;

    Ok(())
}

async fn validate_command<P: AsRef<Path>>(config_path: P) -> Result<()> {
    info!(path = ?config_path.as_ref(), "Validating configuration");

    let config = match load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            error!(%e, "Failed to load configuration");
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
            println!("  [info] {} = {}", default.field, default.value);
        }
        println!();
    }

    // Warnings
    if !report.warnings.is_empty() {
        println!("Warnings ({}):", report.warnings.len());
        for warning in &report.warnings {
            println!("  [warn] [{}] {}", warning.field, warning.message);
        }
        println!();
    }

    // Errors
    if !report.errors.is_empty() {
        println!("Errors ({}):", report.errors.len());
        for err in &report.errors {
            println!("  [error] {}", err);
        }
        println!();
        anyhow::bail!("Configuration validation failed");
    }

    println!("[ok] Configuration is valid!");
    println!();
    println!("Exchange: {}", config.exchange.name);
    println!("Version: {}", config.exchange.version);
    println!("Mode: {:?}", config.exchange.mode);
    println!(
        "Supported Assets: {}",
        config.instrument.supported_assets.len()
    );
    println!(
        "Settlement Currencies: {}",
        config.instrument.settlement_currencies.len()
    );

    Ok(())
}

async fn init_command<P: AsRef<Path>>(output_path: P) -> Result<()> {
    let output_path = output_path.as_ref();
    info!(?output_path, "Initializing new configuration file");

    // Generate default config
    let config = generate_default_config();

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {:?}", parent))?;
    }

    // Save config
    save_config(&config, output_path)?;

    println!("[ok] Configuration file created successfully!");
    println!();
    println!("Location: {:?}", output_path);
    println!();
    println!("This configuration includes:");
    println!("  - Exchange metadata (name, description, version)");
    println!("  - 2 supported assets (BTC, ETH)");
    println!("  - 1 settlement currency (USDT)");
    println!();
    println!("Next steps:");
    println!("  1. Edit the configuration file to customize settings");
    println!("  2. Set required environment variables (database connections, API keys)");
    println!(
        "  3. Run 'openx validate --config {:?}' to check configuration",
        output_path
    );
    println!(
        "  4. Run 'openx start --config {:?}' to start the exchange",
        output_path
    );

    Ok(())
}
