use clap::{Parser, Subcommand};
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
        /// Path to the configuration file
        #[arg(short, long, default_value = "master_config/master_config.yaml")]
        config: PathBuf,
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

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}