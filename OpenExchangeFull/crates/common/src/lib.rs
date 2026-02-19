//! Common types and utilities for OpenExchange
//!
//! This crate provides shared types, traits, and utilities used across
//! all OpenExchange crates.
//!
//! # Modules
//!
//! - [`error`] - Common error types
//! - [`types`] - Shared domain types (OrderId, Side, Symbol, etc.)

pub mod error;
pub mod types;

pub use error::{Error, Result};
pub use types::*;
