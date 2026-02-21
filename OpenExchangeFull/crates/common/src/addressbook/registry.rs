//! Registry types for AddressBook
//!
//! This module defines the types used for service discovery and registry updates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Registry update received from gateway
///
/// This is the payload sent by the gateway to all services
/// when pushing the service registry at startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryUpdate {
    /// Gateway service URL
    #[serde(default)]
    pub gateway: Option<String>,
    
    /// Instrument service URL
    #[serde(default)]
    pub instrument: Option<String>,
    
    /// OMS service URL
    #[serde(default)]
    pub oms: Option<String>,
    
    /// Risk engine service URL
    #[serde(default)]
    pub risk: Option<String>,
    
    /// Matching engine service URL
    #[serde(default)]
    pub matching: Option<String>,
    
    /// Settlement service URL
    #[serde(default)]
    pub settlement: Option<String>,
    
    /// Wallet service URL
    #[serde(default)]
    pub wallet: Option<String>,
    
    /// Market data service URL
    #[serde(default)]
    pub market_data: Option<String>,
}

impl Default for RegistryUpdate {
    fn default() -> Self {
        Self {
            gateway: None,
            instrument: None,
            oms: None,
            risk: None,
            matching: None,
            settlement: None,
            wallet: None,
            market_data: None,
        }
    }
}

/// Response returned after a registry update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryResponse {
    /// Whether the update was successful
    pub success: bool,
    
    /// Optional message (error details or status)
    #[serde(default)]
    pub message: Option<String>,
}

impl RegistryResponse {
    pub fn success() -> Self {
        Self {
            success: true,
            message: None,
        }
    }

    pub fn success_with_message(msg: impl Into<String>) -> Self {
        Self {
            success: true,
            message: Some(msg.into()),
        }
    }

    pub fn failure(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            message: Some(msg.into()),
        }
    }
}

/// Health status of a service
#[derive(Debug, Clone)]
pub struct ServiceHealth {
    /// Name of the service
    pub service_name: String,
    
    /// Whether the service is healthy
    pub is_healthy: bool,
    
    /// Last time health was checked
    pub last_check: DateTime<Utc>,
}

impl ServiceHealth {
    pub fn new(service_name: impl Into<String>, is_healthy: bool) -> Self {
        Self {
            service_name: service_name.into(),
            is_healthy,
            last_check: Utc::now(),
        }
    }
}

/// Address registry holding all service URLs
///
/// This is the internal storage for the AddressBook.
#[derive(Debug, Clone, Default)]
pub struct AddressRegistry {
    /// Gateway service URL
    pub gateway: Option<String>,
    /// Instrument service URL
    pub instrument: Option<String>,
    /// OMS service URL
    pub oms: Option<String>,
    /// Risk engine service URL
    pub risk: Option<String>,
    /// Matching engine service URL
    pub matching: Option<String>,
    /// Settlement service URL
    pub settlement: Option<String>,
    /// Wallet service URL
    pub wallet: Option<String>,
    /// Market data service URL
    pub market_data: Option<String>,
}

impl AddressRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if all required services are registered
    pub fn has_required_services(&self) -> bool {
        // For OMS, we need at least instrument for validation
        self.instrument.is_some()
    }

    /// Get all registered URLs as a vector of (name, url) pairs
    pub fn all_services(&self) -> Vec<(&str, Option<&str>)> {
        vec![
            ("gateway", self.gateway.as_deref()),
            ("instrument", self.instrument.as_deref()),
            ("oms", self.oms.as_deref()),
            ("risk", self.risk.as_deref()),
            ("matching", self.matching.as_deref()),
            ("settlement", self.settlement.as_deref()),
            ("wallet", self.wallet.as_deref()),
            ("market_data", self.market_data.as_deref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_update_default() {
        let update = RegistryUpdate::default();
        assert!(update.gateway.is_none());
        assert!(update.instrument.is_none());
    }

    #[test]
    fn test_registry_response_success() {
        let resp = RegistryResponse::success();
        assert!(resp.success);
        assert!(resp.message.is_none());
    }

    #[test]
    fn test_registry_response_failure() {
        let resp = RegistryResponse::failure("test error");
        assert!(!resp.success);
        assert_eq!(resp.message, Some("test error".to_string()));
    }

    #[test]
    fn test_address_registry_has_required() {
        let mut registry = AddressRegistry::new();
        assert!(!registry.has_required_services());
        
        registry.instrument = Some("http://localhost:8081".to_string());
        assert!(registry.has_required_services());
    }
}
