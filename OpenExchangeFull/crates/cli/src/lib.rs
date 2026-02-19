use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "openx")]
#[command(about = "OpenExchange - A white-label crypto options exchange")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the exchange with the given configuration
    Start {
        /// Deployment mode (monolith or specific service)
        #[arg(short, long, value_enum, default_value = "monolith")]
        mode: DeploymentMode,
        
        /// Path to the configuration file
        #[arg(short, long, default_value = "master_config/master_config.yaml")]
        config: PathBuf,
        
        /// Override HTTP port
        #[arg(long)]
        http: Option<u16>,
        
        /// Override gRPC port
        #[arg(long)]
        grpc: Option<u16>,
        
        /// Override WebSocket port
        #[arg(long)]
        ws: Option<u16>,
    },
    
    /// Validate configuration without starting the exchange
    Validate {
        /// Path to the configuration file
        #[arg(short, long, default_value = "master_config/master_config.yaml")]
        config: PathBuf,
    },
    
    /// Initialize a new configuration file with all defaults
    Init {
        /// Output path for the new configuration file
        #[arg(short, long, default_value = "master_config.yaml")]
        output: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum DeploymentMode {
    /// Run all services in one process (monolith)
    Monolith,
    
    /// API Gateway service - Frontend-facing proxy
    Gateway,
    
    /// Instrument service - Reference data management
    Instrument,
    
    /// OMS service - Order Management System
    Oms,
    
    /// Matching service - Order matching and trade execution
    Matching,
    
    /// Risk service - Risk management and margin calculations
    Risk,
    
    /// Settlement service - Clearing and settlement
    Settlement,
    
    /// Wallet service - Balance management
    Wallet,
    
    /// Market Data service - Price feeds and market data
    Market,
}

impl DeploymentMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeploymentMode::Monolith => "monolith",
            DeploymentMode::Gateway => "gateway",
            DeploymentMode::Instrument => "instrument",
            DeploymentMode::Oms => "oms",
            DeploymentMode::Matching => "matching",
            DeploymentMode::Risk => "risk",
            DeploymentMode::Settlement => "settlement",
            DeploymentMode::Wallet => "wallet",
            DeploymentMode::Market => "market",
        }
    }
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}