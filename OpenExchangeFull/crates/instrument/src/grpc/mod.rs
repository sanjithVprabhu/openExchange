//! gRPC service for instrument management.
//!
//! Provides the instrument service for distributed deployment.

pub mod server;
pub mod client;

pub use server::InstrumentGrpcServer;
pub use client::InstrumentGrpcClient;
