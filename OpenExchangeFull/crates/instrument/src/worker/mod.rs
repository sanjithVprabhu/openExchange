//! Background worker for instrument generation.
//!
//! The worker runs as a background service that:
//! - Generates instruments on startup
//! - Periodically checks spot prices and displacement triggers
//! - Extends the instrument range when triggers are crossed
//! - Updates active/inactive status based on spot price
//! - Marks expired instruments

pub mod service;

pub use service::InstrumentWorker;
