//! OpenExchange CLI and Server Binary
//!
//! This is the main entry point for the OpenExchange application.
//! It provides commands for initializing, validating, and starting
//! the exchange.

use anyhow::{Context, Result};
use cli::{Cli, Commands, DeploymentMode};
use config::{generate_default_config, load_config, save_config, validate_config, MasterConfig};
use instrument::api::handlers::InstrumentApiState;
use instrument::db::models::Environment;
use instrument::db::postgres::PostgresInstrumentStore;
use instrument::worker::service::{InstrumentWorker, StaticSpotPriceProvider};
use observability::{init_logging, LogFormat};
use server::{ports, CombinedServer, ServerConfig, ServerExt};
use common::addressbook::AddressBook;
use oms::{
    OrderManager, PostgresOrderStore, MockMatchingClient,
    api::{handlers::OmsApiState, routes::create_router as create_oms_router, forwarding::OmsForwardingState, forwarding::OmsForwarder},
};
use risk_engine::{
    MarginConfig, RiskEngine,
    api::{RiskApiState, create_router as create_risk_router},
};
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
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
    let using_default_config =
        http_override.is_none() && grpc_override.is_none() && ws_override.is_none();

    if using_default_config {
        println!("Starting in {} mode with default ports", mode.as_str());
    }

    let config = load_config(config_path)?;
    let report = validate_config(&config);

    if !report.warnings.is_empty() {
        warn!("Configuration warnings:");
        for warning in &report.warnings {
            warn!(field = %warning.field, message = %warning.message);
        }
    }

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

    let (default_http, default_grpc, default_ws) = get_default_ports(&mode);
    let http_port = http_override.unwrap_or(default_http);
    let grpc_port = grpc_override.unwrap_or(default_grpc);
    let ws_port = ws_override.unwrap_or(default_ws);

    let mode_name = mode.as_str();

    info!(
        mode = mode_name,
        http_port, grpc_port, ws_port, "Starting exchange"
    );

    start_service_with_ports(&mode, &config, http_port, grpc_port, ws_port).await
}

async fn start_service_with_ports(
    mode: &DeploymentMode,
    config: &MasterConfig,
    http_port: u16,
    grpc_port: u16,
    ws_port: u16,
) -> Result<()> {
    match mode {
        DeploymentMode::Monolith | DeploymentMode::Gateway => {
            start_gateway_or_monolith(mode, config, http_port, grpc_port, ws_port).await
        }
        DeploymentMode::Instrument => {
            start_instrument_service(config, http_port, grpc_port).await
        }
        DeploymentMode::Oms => {
            start_oms_service(config, http_port).await
        }
        DeploymentMode::Risk => {
            start_risk_service(config, http_port).await
        }
        DeploymentMode::Matching | DeploymentMode::Wallet 
        | DeploymentMode::Settlement | DeploymentMode::Market => {
            // Future: start_other_service(config, http_port, grpc_port, ws_port, mode).await
            todo!("Service '{}' not yet implemented", mode.as_str())
        }
    }
}

/// Start in monolith or gateway mode.
///
/// Monolith: HTTP + Direct DB access + Worker (current behavior)
/// Gateway: HTTP + Forwarding to backends (no DB, no worker)
async fn start_gateway_or_monolith(
    mode: &DeploymentMode,
    config: &MasterConfig,
    http_port: u16,
    grpc_port: u16,
    ws_port: u16,
) -> Result<()> {
    let service_name = mode.as_str();

    info!(
        service = service_name,
        http_port, grpc_port, ws_port, "Starting gateway/monolith"
    );

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // For monolith mode, initialize instrument service with DB
    // For gateway mode, we'll use forwarding (no DB)
    let is_gateway = *mode == DeploymentMode::Gateway;

    let instrument_state = if is_gateway {
        // Gateway mode: Don't connect to DB, use forwarding
        None
    } else {
        // Monolith mode: Connect to DB directly
        initialize_instrument_service(config, shutdown_rx.clone()).await?
    };

    // Build HTTP router
    let mut http_router: axum::Router = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(server::health::simple_health_handler),
        )
        .route(
            "/",
            axum::routing::get(move || {
                let name = service_name.to_string();
                async move { format!("{} Service", name) }
            }),
        );

    if is_gateway {
        // Gateway mode: Add forwarding routes
        // Note: We forward via HTTP, so use instrument's HTTP port (8081), not gRPC port (9081)
        let instrument_url = get_service_url("instrument", 8081);
        info!("Gateway mode: Forwarding to instrument service at {}", instrument_url);

        let forwarder = instrument::api::InstrumentForwarder::new(&instrument_url);
        let forwarding_state = Arc::new(instrument::api::ForwardingState {
            instrument: forwarder,
        });

        http_router = http_router.merge(
            instrument::api::instrument_forwarding_routes(forwarding_state)
        );
    } else if let Some(ref state) = instrument_state {
        // Monolith mode: Add direct routes
        info!("Monolith mode: Mounting direct instrument API routes");
        http_router = http_router.merge(
            instrument::api::instrument_routes(Arc::clone(state))
        );
    }

    // Initialize Risk service first (needed by OMS)
    let risk_state = if is_gateway {
        None
    } else {
        initialize_risk_service(config).await?
    };

    // Add Risk routes
    if is_gateway {
        // Gateway mode: Add forwarding routes (future)
    } else if let Some(ref state) = risk_state {
        info!("Monolith mode: Mounting direct Risk API routes");
        http_router = http_router.merge(
            create_risk_router(Arc::clone(state))
        );
    }

    // Initialize OMS service - use direct client in monolith, HTTP in gateway
    let oms_state = if is_gateway {
        // Gateway mode: Use forwarding
        None
    } else if let Some(ref _risk) = risk_state {
        // Monolith mode: Use HTTP client to connect to Risk Engine
        initialize_oms_service_with_risk(config).await?
    } else {
        None
    };

    // Add OMS routes
    if is_gateway {
        // Gateway mode: Add forwarding routes
        let oms_url = get_service_url("oms", 8082);
        info!("Gateway mode: Forwarding to OMS service at {}", oms_url);

        let forwarder = OmsForwarder::new(&oms_url);
        let forwarding_state = Arc::new(OmsForwardingState {
            client: forwarder.client,
            address_book: AddressBook::new(),
        });

        http_router = http_router.merge(
            oms::api::forwarding_routes::oms_forwarding_routes(forwarding_state)
        );
    } else if let Some(ref state) = oms_state {
        // Monolith mode: Add direct routes
        info!("Monolith mode: Mounting direct OMS API routes");
        http_router = http_router.merge(
            create_oms_router(Arc::clone(state))
        );
    }

    // Create server config
    let server_config = ServerConfig {
        host: "0.0.0.0".to_string(),
        http_port: Some(http_port),
        grpc_port: if is_gateway { None } else { Some(grpc_port) }, // Gateway doesn't need gRPC
        websocket_port: Some(ws_port),
    };

    // Create combined server with the router
    let server = CombinedServer::with_http_router(server_config, http_router);

    // Validate ports
    server.validate_ports().await?;

    // Run server with Ctrl+C handling
    server.run_with_ctrl_c().await?;

    // Signal shutdown to workers
    let _ = shutdown_tx.send(true);

    Ok(())
}

/// Start instrument service (gRPC server + worker).
///
/// This is the backend service that handles instrument CRUD operations.
/// It connects to PostgreSQL and runs the background worker.
async fn start_instrument_service(
    config: &MasterConfig,
    http_port: u16,
    grpc_port: u16,
) -> Result<()> {
    info!(
        "Starting Instrument Service (gRPC on port {}, HTTP on port {})",
        grpc_port, http_port
    );

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Initialize instrument service (DB + Worker)
    let instrument_state = initialize_instrument_service(config, shutdown_rx.clone()).await?
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize instrument service"))?;

    // Build HTTP router (optional, for direct access/debugging)
    let http_router: axum::Router = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(server::health::simple_health_handler),
        )
        .route(
            "/",
            axum::routing::get(|| async { "OpenExchange Instrument Service" }),
        )
        .merge(instrument::api::instrument_routes(instrument_state.clone()));

    // Create server config with both HTTP and gRPC
    let server_config = ServerConfig {
        host: "0.0.0.0".to_string(),
        http_port: Some(http_port),
        grpc_port: Some(grpc_port),
        websocket_port: None,
    };

    // Create combined server
    let server = CombinedServer::with_http_router(server_config, http_router);

    // Validate ports
    server.validate_ports().await?;

    // Note: In a full implementation, we'd start the gRPC server here too
    // For now, we just start HTTP and let it be the main interface

    // Run server with Ctrl+C handling
    server.run_with_ctrl_c().await?;

    // Signal shutdown to workers
    let _ = shutdown_tx.send(true);

    Ok(())
}

/// Start OMS service (standalone node).
///
/// This runs as a standalone service on its own node.
async fn start_oms_service(
    config: &MasterConfig,
    http_port: u16,
) -> Result<()> {
    info!(
        "Starting OMS Service (HTTP on port {})",
        http_port
    );

    let oms_state = initialize_oms_service(config)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize OMS service"))?;

    let http_router: axum::Router = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(server::health::simple_health_handler),
        )
        .route(
            "/",
            axum::routing::get(|| async { "OpenExchange OMS Service" }),
        )
        .merge(create_oms_router(oms_state.clone()));

    let server_config = ServerConfig {
        host: "0.0.0.0".to_string(),
        http_port: Some(http_port),
        grpc_port: None,
        websocket_port: None,
    };

    let server = CombinedServer::with_http_router(server_config, http_router);
    server.validate_ports().await?;
    server.run_with_ctrl_c().await?;

    Ok(())
}

async fn initialize_risk_service(
    config: &MasterConfig,
) -> Result<Option<Arc<RiskApiState>>> {
    let margin_config = config.risk_engine.as_ref()
        .map(|r| MarginConfig {
            short_call_stress_multiplier: r.initial_margin.first()
                .map(|m| m.percentage)
                .unwrap_or(0.15),
            maintenance_ratio: 0.75,
            max_position_size: r.position_limits.max_contracts_per_instrument as u32,
            max_total_notional: r.position_limits.max_notional_per_user_usdt,
            max_open_positions: 100,
        })
        .unwrap_or_default();

    let engine = RiskEngine::new(margin_config);
    let state = RiskApiState {
        engine: Arc::new(tokio::sync::RwLock::new(engine)),
    };

    Ok(Some(Arc::new(state)))
}

async fn start_risk_service(
    config: &MasterConfig,
    http_port: u16,
) -> Result<()> {
    info!(
        "Starting Risk Service (HTTP on port {})",
        http_port
    );

    let risk_state = initialize_risk_service(config)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to initialize Risk service"))?;

    let http_router: axum::Router = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(server::health::simple_health_handler),
        )
        .route(
            "/",
            axum::routing::get(|| async { "OpenExchange Risk Service" }),
        )
        .merge(create_risk_router(risk_state.clone()));

    let server_config = ServerConfig {
        host: "0.0.0.0".to_string(),
        http_port: Some(http_port),
        grpc_port: None,
        websocket_port: None,
    };

    let server = CombinedServer::with_http_router(server_config, http_router);
    server.validate_ports().await?;
    server.run_with_ctrl_c().await?;

    Ok(())
}

/// Get service HTTP URL from environment variable.
///
/// Used by gateway to forward requests to backend services.
/// Priority:
/// 1. Environment variable (e.g., INSTRUMENT_SERVICE_URL)
/// 2. Localhost fallback for development
///
/// # Arguments
/// * `service` - Service name (e.g., "instrument", "oms")
/// * `default_port` - Default HTTP port if env var not set
fn get_service_url(service: &str, default_port: u16) -> String {
    // Try SERVICE_URL first, then GRPC_URL for backwards compatibility
    let env_var = format!("{}_SERVICE_URL", service.to_uppercase());
    
    std::env::var(&env_var)
        .or_else(|_| {
            // Fallback to GRPC_URL naming
            let grpc_var = format!("{}_GRPC_URL", service.to_uppercase());
            std::env::var(&grpc_var)
        })
        .unwrap_or_else(|_| {
            info!(
                "Service URL for {} not set, using localhost:{}",
                service, default_port
            );
            format!("http://localhost:{}", default_port)
        })
}

/// Initialize the instrument service for the given mode.
async fn initialize_instrument_service(
    config: &MasterConfig,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<Option<Arc<InstrumentApiState>>> {
    // Get database URL from environment or config
    let database_url = std::env::var("INSTRUMENT_DB_URL")
        .unwrap_or_else(|_| {
            // Try to build from config
            config.instrument.storage.as_ref()
                .and_then(|s| s.postgres.as_ref())
                .map(|pg| {
                    format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        pg.user,
                        pg.password,
                        pg.host,
                        pg.port,
                        pg.database
                    )
                })
                .unwrap_or_else(|| {
                    // Default for development
                    "postgresql://postgres:password@localhost:5432/openexchange".to_string()
                })
        });

    // Determine environment based on config mode
    let environment = match config.exchange.mode {
        config::ExchangeMode::Production => Environment::Prod,
        config::ExchangeMode::Virtual => Environment::Virtual,
        config::ExchangeMode::Both => Environment::Prod, // Default to prod for "both"
    };

    info!(
        "Connecting to instrument database at {} for environment {:?}",
        database_url.split('@').last().unwrap_or(&database_url),
        environment
    );

    // Create stores for each environment
    let mut stores = HashMap::new();

    // Try to connect to database (will fail if not available)
    match PostgresInstrumentStore::new(&database_url, Environment::Static).await {
        Ok(store) => {
            info!("Connected to instrument database for static environment");

            // Run migrations
            if let Err(e) = store.run_migrations().await {
                warn!("Failed to run migrations: {}", e);
            }

            stores.insert("static".to_string(), Arc::new(store));
        }
        Err(e) => {
            warn!("Could not connect to instrument database: {}", e);
            warn!("Instrument service will not be available");
            warn!("Set INSTRUMENT_DB_URL environment variable to enable");
            return Ok(None);
        }
    }

    // Create stores for other environments (same DB, different tables)
    for env in [Environment::Prod, Environment::Virtual] {
        match PostgresInstrumentStore::new(&database_url, env).await {
            Ok(store) => {
                stores.insert(env.to_string(), Arc::new(store));
            }
            Err(e) => {
                warn!("Could not create store for {}: {}", env, e);
            }
        }
    }

    // Create static spot price provider
    let spot_prices = config.instrument.static_prices.as_ref()
        .map(|sp| sp.prices.clone())
        .unwrap_or_else(|| {
            let mut prices = HashMap::new();
            prices.insert("BTC".to_string(), 50000.0);
            prices.insert("ETH".to_string(), 3000.0);
            prices.insert("SOL".to_string(), 100.0);
            prices
        });
    let spot_provider = Arc::new(StaticSpotPriceProvider::new(spot_prices));

    // Create worker if configured
    let worker = if config.instrument.worker.as_ref().map(|w| w.enabled).unwrap_or(true) {
        let worker_config = config.instrument.worker.clone().unwrap_or_default();
        let store = stores.get(&environment.to_string())
            .ok_or_else(|| anyhow::anyhow!("No store for environment"))?;

        let worker = Arc::new(InstrumentWorker::new(
            store.clone(),
            config.instrument.clone(),
            worker_config,
            spot_provider,
            environment,
        ));

        // Start worker in background
        let worker_clone = worker.clone();
        let shutdown_clone = shutdown_rx.clone();
        tokio::spawn(async move {
            worker_clone.run(shutdown_clone).await;
        });

        Some(worker)
    } else {
        None
    };

    // Create API state
    let state = Arc::new(InstrumentApiState {
        stores,
        worker,
    });

    Ok(Some(state))
}

async fn initialize_oms_service(
    config: &MasterConfig,
) -> Result<Option<Arc<OmsApiState>>> {
    let database_url = std::env::var("OMS_DB_URL")
        .unwrap_or_else(|_| {
            config.oms.as_ref()
                .and_then(|o| o.storage.postgres.as_ref())
                .map(|pg| {
                    format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        pg.user,
                        pg.password,
                        pg.host,
                        pg.port,
                        pg.database
                    )
                })
                .unwrap_or_else(|| {
                    "postgresql://postgres:password@localhost:5432/openexchange".to_string()
                })
        });

    info!(
        "Connecting to OMS database at {}",
        database_url.split('@').last().unwrap_or(&database_url)
    );

    match PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!("Connected database");

            let order_store = Arc::new(PostgresOrderStore::new(pool));

            let risk_service_url = get_service_url("risk", 8083);
            info!("Connecting to Risk service at {}", risk_service_url);
            
            let risk_client: Arc<dyn oms::clients::risk::RiskClient> =
                Arc::new(oms::HttpRiskClient::new(&risk_service_url));
            let matching_client: Arc<dyn oms::clients::matching::MatchingClient> =
                Arc::new(MockMatchingClient::new());
            let address_book = AddressBook::new();

            let manager = OrderManager::new(
                order_store,
                risk_client,
                matching_client,
                address_book,
            );

            let state = OmsApiState {
                manager: Arc::new(manager),
            };

            Ok(Some(Arc::new(state)))
        }
        Err(e) => {
            warn!("Could not connect to OMS database: {}", e);
            warn!("OMS service will not be available");
            warn!("Set OMS_DB_URL environment variable to enable");
            Ok(None)
        }
    }
}

/// Initialize OMS service (monolith mode - uses HTTP to Risk)
async fn initialize_oms_service_with_risk(
    config: &MasterConfig,
) -> Result<Option<Arc<OmsApiState>>> {
    // In monolith mode, we still use HTTP client to communicate with Risk Engine
    // This keeps the architecture consistent and allows for easier future separation
    let database_url = std::env::var("OMS_DB_URL")
        .unwrap_or_else(|_| {
            config.oms.as_ref()
                .and_then(|o| o.storage.postgres.as_ref())
                .map(|pg| {
                    format!(
                        "postgresql://{}:{}@{}:{}/{}",
                        pg.user,
                        pg.password,
                        pg.host,
                        pg.port,
                        pg.database
                    )
                })
                .unwrap_or_else(|| {
                    "postgresql://postgres:password@localhost:5432/openexchange".to_string()
                })
        });

    info!(
        "Connecting to OMS database at {}",
        database_url.split('@').last().unwrap_or(&database_url)
    );

    match PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!("Connected database (monolith mode with Risk Engine)");

            let order_store = Arc::new(PostgresOrderStore::new(pool));
            
            // Use HTTP client to connect to Risk Engine (same URL since they're on same server)
            let risk_service_url = "http://localhost:8083";
            let risk_client: Arc<dyn oms::clients::risk::RiskClient> =
                Arc::new(oms::HttpRiskClient::new(risk_service_url));
            
            let matching_client: Arc<dyn oms::clients::matching::MatchingClient> =
                Arc::new(MockMatchingClient::new());
            let address_book = AddressBook::new();

            let manager = OrderManager::new(
                order_store,
                risk_client,
                matching_client,
                address_book,
            );

            let state = OmsApiState {
                manager: Arc::new(manager),
            };

            Ok(Some(Arc::new(state)))
        }
        Err(e) => {
            warn!("Could not connect to OMS database: {}", e);
            warn!("OMS service will not be available");
            warn!("Set OMS_DB_URL environment variable to enable");
            Ok(None)
        }
    }
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

    println!("\n=== Configuration Validation Report ===\n");

    if !report.defaults_applied.is_empty() {
        println!("Defaults Applied ({}):", report.defaults_applied.len());
        for default in &report.defaults_applied {
            println!("  [info] {} = {}", default.field, default.value);
        }
        println!();
    }

    if !report.warnings.is_empty() {
        println!("Warnings ({}):", report.warnings.len());
        for warning in &report.warnings {
            println!("  [warn] [{}] {}", warning.field, warning.message);
        }
        println!();
    }

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

    if config.instrument.generation.is_some() {
        println!("Generation Config: Yes");
        if let Some(gen) = &config.instrument.generation {
            println!("  Assets configured: {:?}", gen.assets.keys().collect::<Vec<_>>());
        }
    } else {
        println!("Generation Config: No");
    }

    Ok(())
}

async fn init_command<P: AsRef<Path>>(output_path: P) -> Result<()> {
    let output_path = output_path.as_ref();
    info!(?output_path, "Initializing new configuration file");

    let config = generate_default_config();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {:?}", parent))?;
    }

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
    println!("     - INSTRUMENT_DB_URL: PostgreSQL connection string");
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
