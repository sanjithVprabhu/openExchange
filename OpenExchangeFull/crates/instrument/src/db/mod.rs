//! Database layer for instrument storage.
//!
//! This module provides PostgreSQL-backed implementations of the `InstrumentStore` trait.
//! It supports three environments (prod, virtual, static) with separate tables.

pub mod models;
pub mod postgres;

pub use models::*;
pub use postgres::PostgresInstrumentStore;
