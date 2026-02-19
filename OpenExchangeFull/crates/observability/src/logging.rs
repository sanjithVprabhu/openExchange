//! Logging initialization and configuration
//!
//! This module provides utilities for initializing the tracing-based
//! logging system with various output formats.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Log output format
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable format with colors (default)
    #[default]
    Pretty,
    /// JSON format for structured logging (better for log aggregation)
    Json,
    /// Compact format (less verbose than pretty)
    Compact,
}

impl LogFormat {
    /// Parse from string (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pretty" => Some(Self::Pretty),
            "json" => Some(Self::Json),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

impl std::str::FromStr for LogFormat {
    type Err = String;
    
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown log format: {}", s))
    }
}

/// Initialize the logging system
///
/// This sets up the tracing subscriber with the specified format.
/// The log level can be controlled via the `RUST_LOG` environment variable.
///
/// # Arguments
///
/// * `service_name` - Name of the service for log identification
/// * `format` - Output format (pretty, json, or compact)
///
/// # Example
///
/// ```ignore
/// use observability::{init_logging, LogFormat};
///
/// init_logging("gateway", LogFormat::Pretty)?;
/// tracing::info!("Service started");
/// ```
///
/// # Environment Variables
///
/// * `RUST_LOG` - Controls log level (e.g., `info`, `debug`, `server=debug,info`)
pub fn init_logging(service_name: &str, format: LogFormat) -> anyhow::Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    match format {
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_target(true)
                        .with_thread_ids(false)
                        .with_file(true)
                        .with_line_number(true)
                        .with_ansi(true),
                )
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().compact())
                .init();
        }
    }

    tracing::info!(
        service = service_name,
        format = ?format,
        "Logging initialized"
    );

    Ok(())
}

/// Initialize logging with default settings (pretty format, info level)
pub fn init_default_logging(service_name: &str) -> anyhow::Result<()> {
    init_logging(service_name, LogFormat::Pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_str() {
        assert_eq!(LogFormat::parse("pretty"), Some(LogFormat::Pretty));
        assert_eq!(LogFormat::parse("PRETTY"), Some(LogFormat::Pretty));
        assert_eq!(LogFormat::parse("json"), Some(LogFormat::Json));
        assert_eq!(LogFormat::parse("compact"), Some(LogFormat::Compact));
        assert_eq!(LogFormat::parse("invalid"), None);
        
        // Test FromStr trait
        assert_eq!("pretty".parse::<LogFormat>(), Ok(LogFormat::Pretty));
        assert!("invalid".parse::<LogFormat>().is_err());
    }
}
