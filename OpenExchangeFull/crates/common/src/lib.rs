//! Common types and utilities for OpenExchange
//!
//! This crate provides shared types, traits, and utilities used across
//! all OpenExchange crates.
//!
//! # Modules
//!
//! - [`error`] - Common error types
//! - [`types`] - Shared domain types (OrderId, Side, Symbol, etc.)
//! - [`addressbook`] - Service discovery and URL resolution

pub mod addressbook;
pub mod error;
pub mod types;

pub use addressbook::{
    AddressBook, RegistryUpdate, RegistryResponse, ServiceHealth, AddressRegistry,
    resolve_url, create_from_config, create_from_env, RegistryConfig, ServiceEndpointConfig,
    check_service_health, check_all_services, get_unhealthy_services, batch_health_check,
};
pub use error::{Error, Result};
pub use types::*;
