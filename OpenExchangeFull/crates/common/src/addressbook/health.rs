//! Health checking utilities for AddressBook
//!
//! This module provides utilities for checking if services are healthy
//! before pushing the registry.

use crate::addressbook::registry::ServiceHealth;
use crate::addressbook::AddressBook;
use reqwest::Client;
use std::time::Duration;

/// Check if a service is healthy by calling its health endpoint
///
/// # Arguments
/// * `base_url` - Base URL of the service (e.g., "http://localhost:8081")
///
/// # Returns
/// * `true` if service responds with 2xx status
/// * `false` otherwise
pub async fn check_service_health(base_url: &str) -> bool {
    let client = match Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let health_url = format!("{}/health", base_url.trim_end_matches('/'));

    match client.get(&health_url).send().await {
        Ok(response) => response.status().is_success(),
        Err(e) => {
            tracing::debug!("Health check failed for {}: {}", health_url, e);
            false
        }
    }
}

/// Check if a service is healthy with custom timeout
///
/// # Arguments
/// * `base_url` - Base URL of the service
/// * `timeout_secs` - Timeout in seconds
pub async fn check_service_health_with_timeout(base_url: &str, timeout_secs: u64) -> bool {
    let client = match Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let health_url = format!("{}/health", base_url.trim_end_matches('/'));

    match client.get(&health_url).send().await {
        Ok(response) => response.status().is_success(),
        Err(e) => {
            tracing::debug!("Health check failed for {}: {}", health_url, e);
            false
        }
    }
}

/// Check multiple services and return their health status
///
/// # Arguments
/// * `address_book` - The address book containing service URLs
///
/// # Returns
/// A vector of ServiceHealth for each registered service
pub async fn check_all_services(address_book: &AddressBook) -> Vec<ServiceHealth> {
    let services = [
        ("gateway", address_book.get_gateway_url()),
        ("instrument", address_book.get_instrument_url()),
        ("oms", address_book.get_oms_url()),
        ("risk", address_book.get_risk_url()),
        ("matching", address_book.get_matching_url()),
    ];

    let mut results = Vec::new();

    for (name, url) in services {
        if let Some(url) = url {
            let is_healthy = check_service_health(&url).await;
            results.push(ServiceHealth::new(name, is_healthy));
        }
    }

    results
}

/// Check services and return only unhealthy ones
///
/// # Arguments
/// * `address_book` - The address book containing service URLs
///
/// # Returns
/// A vector of service names that are unhealthy
pub async fn get_unhealthy_services(address_book: &AddressBook) -> Vec<String> {
    let health_status = check_all_services(address_book).await;
    
    health_status
        .into_iter()
        .filter(|h| !h.is_healthy)
        .map(|h| h.service_name)
        .collect()
}

/// Wait for a service to become healthy
///
/// # Arguments
/// * `base_url` - Base URL of the service
/// * `max_attempts` - Maximum number of retry attempts
/// * `delay_ms` - Delay between attempts in milliseconds
///
/// # Returns
/// * `true` if service becomes healthy
/// * `false` if max attempts reached
pub async fn wait_for_service(
    base_url: &str, 
    max_attempts: u32, 
    delay_ms: u64,
) -> bool {
    for attempt in 1..=max_attempts {
        if check_service_health(base_url).await {
            return true;
        }
        
        if attempt < max_attempts {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }
    
    false
}

/// Health check result for a batch of services
#[derive(Debug)]
pub struct BatchHealthCheckResult {
    /// Total number of services checked
    pub total: usize,
    /// Number of healthy services
    pub healthy: usize,
    /// Number of unhealthy services
    pub unhealthy: usize,
    /// Detailed health status for each service
    pub services: Vec<ServiceHealth>,
}

impl BatchHealthCheckResult {
    /// Check if all services are healthy
    pub fn all_healthy(&self) -> bool {
        self.unhealthy == 0
    }

    /// Check if any service is healthy
    pub fn any_healthy(&self) -> bool {
        self.healthy > 0
    }

    /// Get percentage of healthy services
    pub fn healthy_percentage(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        (self.healthy as f64 / self.total as f64) * 100.0
    }
}

/// Perform batch health check on all registered services
pub async fn batch_health_check(address_book: &AddressBook) -> BatchHealthCheckResult {
    let services = check_all_services(address_book).await;
    
    let total = services.len();
    let healthy = services.iter().filter(|s| s.is_healthy).count();
    let unhealthy = total - healthy;

    BatchHealthCheckResult {
        total,
        healthy,
        unhealthy,
        services,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_service_health_invalid_url() {
        let result = check_service_health("http://invalid-host-that-does-not-exist:9999").await;
        // Should return false for unreachable service
        assert!(!result);
    }
}
