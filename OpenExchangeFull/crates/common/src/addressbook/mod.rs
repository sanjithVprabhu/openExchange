//! AddressBook - Service discovery for OpenExchange
//!
//! This module provides a centralized address book for service discovery
//! in both monolith and distributed deployment modes.
//!
//! # Features
//!
//! - **URL Resolution**: Priority is ENV > CONFIG > DEFAULT
//! - **Health Checking**: Verify services are up before communication
//! - **Gateway Push**: Gateway distributes registry to all nodes
//! - **Protocol Agnostic**: Works with HTTP, gRPC, or WebSocket
//!
//! # Usage
//!
//! ## Creating an AddressBook
//!
//! ```ignore
//! use common::addressbook::AddressBook;
//!
//! // Create empty address book
//! let address_book = AddressBook::new();
//!
//! // Or create from config with env var override
//! use common::addressbook::{create_from_env, RegistryConfig};
//! let address_book = create_from_env();
//! ```
//!
//! ## Updating Registry
//!
//! ```ignore
//! use common::addressbook::RegistryUpdate;
//!
//! let update = RegistryUpdate {
//!     instrument: Some("http://instrument:8081".to_string()),
//!     oms: Some("http://oms:8082".to_string()),
//!     risk: Some("http://risk:8083".to_string()),
//!     matching: Some("http://matching:8084".to_string()),
//!     ..Default::default()
//! };
//!
//! address_book.update_all(update);
//! ```
//!
//! ## Getting Service URLs
//!
//! ```ignore
//! let instrument_url = address_book.get_instrument_url();
//! let oms_url = address_book.get_oms_url();
//! ```
//!
//! # Architecture
//!
//! ## Monolith Mode
//! In monolith mode, all services run in a single process.
//! AddressBook is primarily used for logging and debugging.
//!
//! ## Distributed Mode
//! In distributed mode, each service runs as a separate process.
//! The Gateway acts as the central registry and pushes URLs to all services:
//!
//! 1. Gateway starts with service URLs from env vars or config
//! 2. Gateway health checks each service
//! 3. Gateway pushes registry to each service via `/internal/registry`
//! 4. Each service stores the registry in its AddressBook
//! 5. Services use AddressBook to discover other services

pub mod registry;
pub mod resolver;
pub mod health;

use std::sync::{Arc, RwLock};

pub use registry::{RegistryUpdate, RegistryResponse, ServiceHealth, AddressRegistry};
pub use resolver::{
    resolve_url, create_from_config, create_from_env, 
    RegistryConfig, ServiceEndpointConfig,
};
pub use health::{
    check_service_health, check_all_services, get_unhealthy_services, batch_health_check,
};

/// AddressBook - Centralized service discovery
///
/// This is the main struct for managing service URLs across the system.
/// It is designed to be shared across all handlers in a service.
pub struct AddressBook {
    inner: Arc<RwLock<AddressRegistry>>,
}

impl AddressBook {
    /// Create a new empty AddressBook
    ///
    /// This creates an address book with no registered services.
    /// Services must be added via `update_all()` before use.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(RwLock::new(AddressRegistry::default())),
        })
    }

    /// Update all addresses from a registry update
    ///
    /// This is typically called when receiving the registry from the gateway.
    pub fn update_all(&self, update: RegistryUpdate) {
        let mut registry = self.inner.write().unwrap();
        
        if let Some(url) = update.gateway {
            registry.gateway = Some(url);
        }
        if let Some(url) = update.instrument {
            registry.instrument = Some(url);
        }
        if let Some(url) = update.oms {
            registry.oms = Some(url);
        }
        if let Some(url) = update.risk {
            registry.risk = Some(url);
        }
        if let Some(url) = update.matching {
            registry.matching = Some(url);
        }
        if let Some(url) = update.settlement {
            registry.settlement = Some(url);
        }
        if let Some(url) = update.wallet {
            registry.wallet = Some(url);
        }
        if let Some(url) = update.market_data {
            registry.market_data = Some(url);
        }
    }

    /// Get the gateway service URL
    pub fn get_gateway_url(&self) -> Option<String> {
        self.inner.read().unwrap().gateway.clone()
    }

    /// Get the instrument service URL
    pub fn get_instrument_url(&self) -> Option<String> {
        self.inner.read().unwrap().instrument.clone()
    }

    /// Get the OMS service URL
    pub fn get_oms_url(&self) -> Option<String> {
        self.inner.read().unwrap().oms.clone()
    }

    /// Get the Risk engine service URL
    pub fn get_risk_url(&self) -> Option<String> {
        self.inner.read().unwrap().risk.clone()
    }

    /// Get the Matching engine service URL
    pub fn get_matching_url(&self) -> Option<String> {
        self.inner.read().unwrap().matching.clone()
    }

    /// Get the Settlement service URL
    pub fn get_settlement_url(&self) -> Option<String> {
        self.inner.read().unwrap().settlement.clone()
    }

    /// Get the Wallet service URL
    pub fn get_wallet_url(&self) -> Option<String> {
        self.inner.read().unwrap().wallet.clone()
    }

    /// Get the Market data service URL
    pub fn get_market_data_url(&self) -> Option<String> {
        self.inner.read().unwrap().market_data.clone()
    }

    /// Check if the address book has the minimum required services
    ///
    /// For OMS, at least the instrument service must be registered
    /// for order validation.
    pub fn is_ready(&self) -> bool {
        let registry = self.inner.read().unwrap();
        registry.has_required_services()
    }

    /// Get all registered URLs for debugging
    pub fn get_all(&self) -> AddressRegistry {
        self.inner.read().unwrap().clone()
    }

    /// Get a URL for a service by name
    ///
    /// # Arguments
    /// * `service_name` - Name of the service (e.g., "instrument", "oms")
    ///
    /// # Returns
    /// The URL if found, None otherwise
    pub fn get_url(&self, service_name: &str) -> Option<String> {
        let registry = self.inner.read().unwrap();
        match service_name.to_lowercase().as_str() {
            "gateway" => registry.gateway.clone(),
            "instrument" => registry.instrument.clone(),
            "oms" => registry.oms.clone(),
            "risk" => registry.risk.clone(),
            "matching" => registry.matching.clone(),
            "settlement" => registry.settlement.clone(),
            "wallet" => registry.wallet.clone(),
            "market_data" => registry.market_data.clone(),
            _ => None,
        }
    }

    /// Check if a specific service is registered
    pub fn has_service(&self, service_name: &str) -> bool {
        self.get_url(service_name).is_some()
    }
}

impl Default for AddressBook {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(AddressRegistry::default())),
        }
    }
}

impl std::fmt::Debug for AddressBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registry = self.inner.read().unwrap();
        f.debug_struct("AddressBook")
            .field("gateway", &registry.gateway)
            .field("instrument", &registry.instrument)
            .field("oms", &registry.oms)
            .field("risk", &registry.risk)
            .field("matching", &registry.matching)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_book_new() {
        let ab = AddressBook::new();
        assert!(!ab.is_ready());
    }

    #[test]
    fn test_address_book_update() {
        let ab = AddressBook::new();
        
        let update = RegistryUpdate {
            instrument: Some("http://localhost:8081".to_string()),
            oms: Some("http://localhost:8082".to_string()),
            ..Default::default()
        };
        
        ab.update_all(update);
        
        assert_eq!(ab.get_instrument_url(), Some("http://localhost:8081".to_string()));
        assert_eq!(ab.get_oms_url(), Some("http://localhost:8082".to_string()));
        assert!(!ab.is_ready()); // Gateway not set
    }

    #[test]
    fn test_address_book_is_ready() {
        let ab = AddressBook::new();
        
        // Without instrument, not ready
        assert!(!ab.is_ready());
        
        // Add instrument
        let update = RegistryUpdate {
            instrument: Some("http://localhost:8081".to_string()),
            ..Default::default()
        };
        ab.update_all(update);
        
        // Now ready
        assert!(ab.is_ready());
    }

    #[test]
    fn test_get_url() {
        let ab = AddressBook::new();
        
        let update = RegistryUpdate {
            instrument: Some("http://instrument:8081".to_string()),
            oms: Some("http://oms:8082".to_string()),
            ..Default::default()
        };
        ab.update_all(update);
        
        assert_eq!(ab.get_url("instrument"), Some("http://instrument:8081".to_string()));
        assert_eq!(ab.get_url("oms"), Some("http://oms:8082".to_string()));
        assert_eq!(ab.get_url("unknown"), None);
    }

    #[test]
    fn test_has_service() {
        let ab = AddressBook::new();
        
        let update = RegistryUpdate {
            instrument: Some("http://localhost:8081".to_string()),
            ..Default::default()
        };
        ab.update_all(update);
        
        assert!(ab.has_service("instrument"));
        assert!(!ab.has_service("oms"));
    }
}
