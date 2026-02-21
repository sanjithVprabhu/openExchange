//! URL resolution logic for AddressBook
//!
//! This module provides utilities for resolving service URLs
//! with the correct priority: ENV > CONFIG > DEFAULT

use crate::addressbook::registry::RegistryUpdate;
use crate::addressbook::AddressBook;
use std::sync::Arc;

/// Service endpoint configuration
#[derive(Debug, Clone, Default)]
pub struct ServiceEndpointConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
}

impl ServiceEndpointConfig {
    pub fn url(&self) -> Option<String> {
        match (&self.host, &self.port) {
            (Some(host), Some(port)) => Some(format!("http://{}:{}", host, port)),
            _ => None,
        }
    }
}

/// Registry configuration from config file
#[derive(Debug, Clone, Default)]
pub struct RegistryConfig {
    pub gateway: Option<ServiceEndpointConfig>,
    pub instrument: Option<ServiceEndpointConfig>,
    pub oms: Option<ServiceEndpointConfig>,
    pub risk: Option<ServiceEndpointConfig>,
    pub matching: Option<ServiceEndpointConfig>,
    pub settlement: Option<ServiceEndpointConfig>,
    pub wallet: Option<ServiceEndpointConfig>,
    pub market_data: Option<ServiceEndpointConfig>,
}

/// Resolve a single URL with priority: ENV > CONFIG > DEFAULT
///
/// # Arguments
/// * `env_var` - Environment variable name (e.g., "INSTRUMENT_SERVICE_URL")
/// * `config_url` - URL from config file
/// * `default_url` - Default fallback URL
///
/// # Example
/// ```
/// let url = resolve_url(
///     "INSTRUMENT_SERVICE_URL",
///     Some("http://instrument:8081"),
///     "http://localhost:8081"
/// );
/// ```
pub fn resolve_url(env_var: &str, config_url: Option<&str>, default_url: &str) -> String {
    // Priority 1: Environment variable
    if let Ok(url) = std::env::var(env_var) {
        if !url.trim().is_empty() {
            return normalize_url(&url);
        }
    }

    // Priority 2: Config file
    if let Some(url) = config_url {
        if !url.trim().is_empty() {
            return normalize_url(url);
        }
    }

    // Priority 3: Default
    normalize_url(default_url)
}

/// Normalize a URL by ensuring it has the correct scheme and no trailing slashes
fn normalize_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');
    
    // Add scheme if missing
    if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("http://{}", url)
    } else {
        url.to_string()
    }
}

/// Create an AddressBook from registry config with env var override
///
/// This function reads service URLs from the config,
/// but allows environment variables to override them.
///
/// # Arguments
/// * `config` - Reference to the registry configuration
///
/// # Returns
/// A new AddressBook with URLs resolved from env vars, config, or defaults
pub fn create_from_config(config: &RegistryConfig) -> Arc<AddressBook> {
    let address_book = AddressBook::new();

    // Helper to get URL from config
    let get_url = |env_var: &str, config_endpoint: Option<&ServiceEndpointConfig>, default: &str| -> String {
        let config_url = config_endpoint.and_then(|e| e.url());
        resolve_url(env_var, config_url.as_deref(), default)
    };

    // Resolve each service URL
    let update = RegistryUpdate {
        gateway: Some(get_url("GATEWAY_SERVICE_URL", config.gateway.as_ref(), "http://localhost:8080")),
        instrument: Some(get_url("INSTRUMENT_SERVICE_URL", config.instrument.as_ref(), "http://localhost:8081")),
        oms: Some(get_url("OMS_SERVICE_URL", config.oms.as_ref(), "http://localhost:8082")),
        risk: Some(get_url("RISK_SERVICE_URL", config.risk.as_ref(), "http://localhost:8083")),
        matching: Some(get_url("MATCHING_SERVICE_URL", config.matching.as_ref(), "http://localhost:8084")),
        settlement: Some(get_url("SETTLEMENT_SERVICE_URL", config.settlement.as_ref(), "http://localhost:8085")),
        wallet: Some(get_url("WALLET_SERVICE_URL", config.wallet.as_ref(), "http://localhost:8086")),
        market_data: Some(get_url("MARKET_DATA_SERVICE_URL", config.market_data.as_ref(), "http://localhost:8087")),
    };

    address_book.update_all(update);
    address_book
}

/// Create an AddressBook from environment variables only
///
/// This is useful when config file is not available.
/// Each service URL must be set via environment variable.
///
/// Environment variables:
/// - GATEWAY_SERVICE_URL
/// - INSTRUMENT_SERVICE_URL
/// - OMS_SERVICE_URL
/// - RISK_SERVICE_URL
/// - MATCHING_SERVICE_URL
/// - SETTLEMENT_SERVICE_URL
/// - WALLET_SERVICE_URL
/// - MARKET_DATA_SERVICE_URL
pub fn create_from_env() -> Arc<AddressBook> {
    let address_book = AddressBook::new();

    let update = RegistryUpdate {
        gateway: Some(resolve_url("GATEWAY_SERVICE_URL", None, "http://localhost:8080")),
        instrument: Some(resolve_url("INSTRUMENT_SERVICE_URL", None, "http://localhost:8081")),
        oms: Some(resolve_url("OMS_SERVICE_URL", None, "http://localhost:8082")),
        risk: Some(resolve_url("RISK_SERVICE_URL", None, "http://localhost:8083")),
        matching: Some(resolve_url("MATCHING_SERVICE_URL", None, "http://localhost:8084")),
        settlement: Some(resolve_url("SETTLEMENT_SERVICE_URL", None, "http://localhost:8085")),
        wallet: Some(resolve_url("WALLET_SERVICE_URL", None, "http://localhost:8086")),
        market_data: Some(resolve_url("MARKET_DATA_SERVICE_URL", None, "http://localhost:8087")),
    };

    address_book.update_all(update);
    address_book
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url_env_priority() {
        // Set environment variable
        std::env::set_var("TEST_SERVICE_URL", "http://env-service:9000");
        
        let url = resolve_url("TEST_SERVICE_URL", Some("http://config:8000"), "http://default:7000");
        assert_eq!(url, "http://env-service:9000");
        
        std::env::remove_var("TEST_SERVICE_URL");
    }

    #[test]
    fn test_resolve_url_config_fallback() {
        let url = resolve_url("TEST_SERVICE_URL_2", Some("http://config:8000"), "http://default:7000");
        assert_eq!(url, "http://config:8000");
    }

    #[test]
    fn test_resolve_url_default() {
        let url = resolve_url("TEST_SERVICE_URL_3", None, "http://default:7000");
        assert_eq!(url, "http://default:7000");
    }

    #[test]
    fn test_normalize_url() {
        assert_eq!(normalize_url("localhost:8081"), "http://localhost:8081");
        assert_eq!(normalize_url("http://localhost:8081/"), "http://localhost:8081");
        assert_eq!(normalize_url("https://localhost:8081"), "https://localhost:8081");
    }

    #[test]
    fn test_create_from_env() {
        std::env::set_var("INSTRUMENT_SERVICE_URL", "http://custom:9999");
        
        let ab = create_from_env();
        assert_eq!(ab.get_instrument_url(), Some("http://custom:9999".to_string()));
        
        std::env::remove_var("INSTRUMENT_SERVICE_URL");
    }

    #[test]
    fn test_registry_config() {
        let config = RegistryConfig {
            instrument: Some(ServiceEndpointConfig {
                host: Some("config-host".to_string()),
                port: Some(9000),
            }),
            ..Default::default()
        };

        let ab = create_from_config(&config);
        assert_eq!(ab.get_instrument_url(), Some("http://config-host:9000".to_string()));
    }
}
