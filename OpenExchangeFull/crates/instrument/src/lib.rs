//! # Instrument Crate
//!
//! This crate provides the core instrument layer for the OpenExchange options exchange.
//! It is the foundation upon which all other modules depend.
//!
//! ## Key Components
//!
//! - **Domain Types**: `OptionInstrument`, `OptionType`, `ExerciseStyle`, `InstrumentId`
//! - **Traits**: `InstrumentStore` for storage abstraction, `InstrumentGenerator` for generation
//! - **Generation**: Automatic generation of option instruments from config (expiry schedules)
//! - **In-Memory Store**: Default implementation for testing and development
//!
//! ## Architecture
//!
//! Following the trait-based architecture, this crate defines TRAITS that adapters implement.
//! The core business logic never imports infrastructure (Postgres, Redis, etc.).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Instrument Crate                         │
//! │  ┌─────────────────┐  ┌──────────────────────────────────┐ │
//! │  │  Domain Types   │  │           Traits                 │ │
//! │  │ OptionInstrument│  │  InstrumentStore                 │ │
//! │  │ OptionType      │  │  InstrumentGenerator             │ │
//! │  │ ExerciseStyle   │  │  InstrumentService               │ │
//! │  └─────────────────┘  └──────────────────────────────────┘ │
//! │  ┌─────────────────────────────────────────────────────────┐│
//! │  │              Generation Logic                           ││
//! │  │  - Generate strikes from spot price                     ││
//! │  │  - Generate expiries from config schedules              ││
//! │  │  - Create instrument symbols (BTC-20240315-50000-C)     ││
//! │  └─────────────────────────────────────────────────────────┘│
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  Storage Adapters (external)                │
//! │  PostgresInstrumentStore │ RedisInstrumentStore │ etc.      │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod error;
pub mod generator;
pub mod service;
pub mod store;
pub mod types;

#[cfg(feature = "postgres")]
pub mod db;

#[cfg(feature = "postgres")]
pub mod worker;

#[cfg(feature = "api")]
pub mod api;

#[cfg(feature = "grpc")]
pub mod grpc;

// Re-export main types for convenience
pub use error::{InstrumentError, InstrumentResult};
pub use generator::{
    ExpiryGenerator, GenerationStateManager, GridStrikeGenerator, InstrumentGenerator,
    StrikeGenerator,
};
pub use service::InstrumentService;
pub use store::{InMemoryInstrumentStore, InstrumentStore};
pub use types::{
    ExerciseStyle, InstrumentId, InstrumentStatus, OptionInstrument, OptionType, Strike,
    UnderlyingAsset,
};

#[cfg(feature = "postgres")]
pub use db::{
    models::{Environment, GenerationState},
    PostgresInstrumentStore,
};

#[cfg(feature = "postgres")]
pub use worker::{InstrumentWorker, service::SpotPriceProvider, service::StaticSpotPriceProvider};

#[cfg(feature = "grpc")]
pub use grpc::{
    server::{InstrumentGrpcServer, InstrumentService},
    client::{InstrumentGrpcClient, InstrumentHttpClient},
};
