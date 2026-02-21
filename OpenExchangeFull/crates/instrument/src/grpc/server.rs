//! gRPC server implementation for the instrument service.

use crate::api::handlers::InstrumentApiState;
use crate::db::models::Environment;
use crate::store::{InstrumentQuery, InstrumentStore};
use crate::types::{InstrumentId, InstrumentStatus, OptionType};
use crate::worker::service::InstrumentWorker;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

// Import generated proto types (or define inline)
// For now, we define the message types inline to avoid build.rs complexity

/// gRPC message for an instrument.
#[derive(Debug, Clone, prost::Message)]
pub struct Instrument {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub symbol: String,
    #[prost(string, tag = "3")]
    pub underlying_symbol: String,
    #[prost(string, tag = "4")]
    pub underlying_name: String,
    #[prost(enumeration = "OptionTypeProto", tag = "5")]
    pub option_type: i32,
    #[prost(double, tag = "6")]
    pub strike: f64,
    #[prost(message, optional, tag = "7")]
    pub expiry: Option<prost_types::Timestamp>,
    #[prost(string, tag = "8")]
    pub settlement_currency: String,
    #[prost(double, tag = "9")]
    pub contract_size: f64,
    #[prost(double, tag = "10")]
    pub tick_size: f64,
    #[prost(uint64, tag = "11")]
    pub min_order_size: u64,
    #[prost(enumeration = "InstrumentStatusProto", tag = "12")]
    pub status: i32,
    #[prost(message, optional, tag = "13")]
    pub created_at: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "14")]
    pub updated_at: Option<prost_types::Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, prost::Enumeration)]
pub enum OptionTypeProto {
    Unspecified = 0,
    Call = 1,
    Put = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, prost::Enumeration)]
pub enum InstrumentStatusProto {
    Unspecified = 0,
    Active = 1,
    Inactive = 2,
    Expired = 3,
    Settled = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, prost::Enumeration)]
pub enum EnvironmentProto {
    Unspecified = 0,
    Prod = 1,
    Virtual = 2,
    Static = 3,
}

// Request/Response messages

#[derive(Debug, prost::Message)]
pub struct ListInstrumentsRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
    #[prost(string, optional, tag = "2")]
    pub underlying: Option<String>,
    #[prost(enumeration = "OptionTypeProto", optional, tag = "3")]
    pub option_type: Option<i32>,
    #[prost(enumeration = "InstrumentStatusProto", optional, tag = "4")]
    pub status: Option<i32>,
    #[prost(message, optional, tag = "5")]
    pub expiry_after: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "6")]
    pub expiry_before: Option<prost_types::Timestamp>,
    #[prost(double, optional, tag = "7")]
    pub strike_min: Option<f64>,
    #[prost(double, optional, tag = "8")]
    pub strike_max: Option<f64>,
    #[prost(uint32, tag = "9")]
    pub limit: u32,
    #[prost(uint32, tag = "10")]
    pub offset: u32,
}

#[derive(Debug, prost::Message)]
pub struct ListInstrumentsResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(uint32, tag = "2")]
    pub total_count: u32,
    #[prost(message, repeated, tag = "3")]
    pub instruments: Vec<Instrument>,
    #[prost(string, tag = "4")]
    pub error_message: String,
}

#[derive(Debug, prost::Message)]
pub struct GetInstrumentRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
    #[prost(string, tag = "2")]
    pub id: String,
}

#[derive(Debug, prost::Message)]
pub struct GetInstrumentBySymbolRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
    #[prost(string, tag = "2")]
    pub symbol: String,
}

#[derive(Debug, prost::Message)]
pub struct GetInstrumentResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(message, optional, tag = "2")]
    pub instrument: Option<Instrument>,
    #[prost(string, tag = "3")]
    pub error_message: String,
}

#[derive(Debug, prost::Message)]
pub struct GetStatisticsRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
}

#[derive(Debug, prost::Message)]
pub struct GetStatisticsResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(uint32, tag = "2")]
    pub total: u32,
    #[prost(map = "string, uint32", tag = "3")]
    pub by_status: HashMap<String, u32>,
    #[prost(map = "string, uint32", tag = "4")]
    pub by_underlying: HashMap<String, u32>,
    #[prost(string, tag = "5")]
    pub error_message: String,
}

#[derive(Debug, prost::Message)]
pub struct UpdateStatusRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
    #[prost(string, tag = "2")]
    pub id: String,
    #[prost(enumeration = "InstrumentStatusProto", tag = "3")]
    pub status: i32,
}

#[derive(Debug, prost::Message)]
pub struct UpdateStatusResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(message, optional, tag = "2")]
    pub instrument: Option<Instrument>,
    #[prost(string, tag = "3")]
    pub error_message: String,
}

#[derive(Debug, prost::Message)]
pub struct ForceRegenerateRequest {
    #[prost(enumeration = "EnvironmentProto", tag = "1")]
    pub environment: i32,
    #[prost(string, tag = "2")]
    pub underlying: String,
    #[prost(double, tag = "3")]
    pub spot_price: f64,
}

#[derive(Debug, prost::Message)]
pub struct ForceRegenerateResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    #[prost(uint32, tag = "2")]
    pub instruments_created: u32,
    #[prost(uint32, tag = "3")]
    pub instruments_updated: u32,
    #[prost(string, tag = "4")]
    pub error_message: String,
}

/// gRPC service trait for instrument management.
#[tonic::async_trait]
pub trait InstrumentService: Send + Sync + 'static {
    async fn list_instruments(
        &self,
        request: Request<ListInstrumentsRequest>,
    ) -> Result<Response<ListInstrumentsResponse>, Status>;

    async fn get_instrument(
        &self,
        request: Request<GetInstrumentRequest>,
    ) -> Result<Response<GetInstrumentResponse>, Status>;

    async fn get_instrument_by_symbol(
        &self,
        request: Request<GetInstrumentBySymbolRequest>,
    ) -> Result<Response<GetInstrumentResponse>, Status>;

    async fn get_statistics(
        &self,
        request: Request<GetStatisticsRequest>,
    ) -> Result<Response<GetStatisticsResponse>, Status>;

    async fn update_instrument_status(
        &self,
        request: Request<UpdateStatusRequest>,
    ) -> Result<Response<UpdateStatusResponse>, Status>;

    async fn force_regenerate(
        &self,
        request: Request<ForceRegenerateRequest>,
    ) -> Result<Response<ForceRegenerateResponse>, Status>;
}

/// gRPC server implementation for the instrument service.
pub struct InstrumentGrpcServer {
    state: Arc<InstrumentApiState>,
}

impl InstrumentGrpcServer {
    pub fn new(state: Arc<InstrumentApiState>) -> Self {
        Self { state }
    }

    fn proto_to_env(proto: i32) -> Option<Environment> {
        match EnvironmentProto::try_from(proto).ok()? {
            EnvironmentProto::Prod => Some(Environment::Prod),
            EnvironmentProto::Virtual => Some(Environment::Virtual),
            EnvironmentProto::Static => Some(Environment::Static),
            _ => None,
        }
    }

    fn domain_to_proto(instrument: &crate::types::OptionInstrument) -> Instrument {
        Instrument {
            id: instrument.id.to_string(),
            symbol: instrument.symbol.clone(),
            underlying_symbol: instrument.underlying.symbol.clone(),
            underlying_name: instrument.underlying.name.clone(),
            option_type: match instrument.option_type {
                OptionType::Call => OptionTypeProto::Call as i32,
                OptionType::Put => OptionTypeProto::Put as i32,
            },
            strike: instrument.strike.value(),
            expiry: Some(prost_types::Timestamp {
                seconds: instrument.expiry.timestamp(),
                nanos: 0,
            }),
            settlement_currency: instrument.settlement_currency.clone(),
            contract_size: instrument.contract_size,
            tick_size: instrument.tick_size,
            min_order_size: instrument.min_order_size,
            status: match instrument.status {
                InstrumentStatus::Active => InstrumentStatusProto::Active as i32,
                InstrumentStatus::Inactive => InstrumentStatusProto::Inactive as i32,
                InstrumentStatus::Expired => InstrumentStatusProto::Expired as i32,
                InstrumentStatus::Settled => InstrumentStatusProto::Settled as i32,
                InstrumentStatus::Pending => InstrumentStatusProto::Unspecified as i32,
                InstrumentStatus::Suspended => InstrumentStatusProto::Inactive as i32,
            },
            created_at: Some(prost_types::Timestamp {
                seconds: instrument.created_at.timestamp(),
                nanos: 0,
            }),
            updated_at: Some(prost_types::Timestamp {
                seconds: instrument.updated_at.timestamp(),
                nanos: 0,
            }),
        }
    }
}

#[tonic::async_trait]
impl InstrumentService for InstrumentGrpcServer {
    async fn list_instruments(
        &self,
        request: Request<ListInstrumentsRequest>,
    ) -> Result<Response<ListInstrumentsResponse>, Status> {
        let req = request.into_inner();
        let env_str = Self::proto_to_env(req.environment)
            .ok_or_else(|| Status::invalid_argument("Invalid environment"))?;

        let store = self.state.stores.get(&env_str.to_string())
            .ok_or_else(|| Status::not_found("Environment not found"))?;

        let mut query = InstrumentQuery::new();

        if let Some(underlying) = &req.underlying {
            query = query.with_underlying(underlying);
        }
        if let Some(status) = InstrumentStatusProto::try_from(req.status.unwrap_or(0)).ok() {
            if status != InstrumentStatusProto::Unspecified {
                let s = match status {
                    InstrumentStatusProto::Active => InstrumentStatus::Active,
                    InstrumentStatusProto::Inactive => InstrumentStatus::Inactive,
                    InstrumentStatusProto::Expired => InstrumentStatus::Expired,
                    InstrumentStatusProto::Settled => InstrumentStatus::Settled,
                    _ => InstrumentStatus::Active,
                };
                query = query.with_status(s);
            }
        }
        if let Some(min) = req.strike_min {
            query.strike_min = Some(min);
        }
        if let Some(max) = req.strike_max {
            query.strike_max = Some(max);
        }

        let limit = req.limit.min(1000) as usize;
        query = query.with_pagination(limit, req.offset as usize);

        let instruments = store.list(&query).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let proto_instruments: Vec<Instrument> = instruments.iter()
            .map(Self::domain_to_proto)
            .collect();

        Ok(Response::new(ListInstrumentsResponse {
            success: true,
            total_count: proto_instruments.len() as u32,
            instruments: proto_instruments,
            error_message: String::new(),
        }))
    }

    async fn get_instrument(
        &self,
        request: Request<GetInstrumentRequest>,
    ) -> Result<Response<GetInstrumentResponse>, Status> {
        let req = request.into_inner();
        let env_str = Self::proto_to_env(req.environment)
            .ok_or_else(|| Status::invalid_argument("Invalid environment"))?;

        let store = self.state.stores.get(&env_str.to_string())
            .ok_or_else(|| Status::not_found("Environment not found"))?;

        let id = InstrumentId::new(&req.id);
        let instrument = store.get(&id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetInstrumentResponse {
            success: instrument.is_some(),
            instrument: instrument.as_ref().map(Self::domain_to_proto),
            error_message: if instrument.is_none() { "Not found".to_string() } else { String::new() },
        }))
    }

    async fn get_instrument_by_symbol(
        &self,
        request: Request<GetInstrumentBySymbolRequest>,
    ) -> Result<Response<GetInstrumentResponse>, Status> {
        let req = request.into_inner();
        let env_str = Self::proto_to_env(req.environment)
            .ok_or_else(|| Status::invalid_argument("Invalid environment"))?;

        let store = self.state.stores.get(&env_str.to_string())
            .ok_or_else(|| Status::not_found("Environment not found"))?;

        let instrument = store.get_by_symbol(&req.symbol).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetInstrumentResponse {
            success: instrument.is_some(),
            instrument: instrument.as_ref().map(Self::domain_to_proto),
            error_message: if instrument.is_none() { "Not found".to_string() } else { String::new() },
        }))
    }

    async fn get_statistics(
        &self,
        request: Request<GetStatisticsRequest>,
    ) -> Result<Response<GetStatisticsResponse>, Status> {
        let req = request.into_inner();
        let env_str = Self::proto_to_env(req.environment)
            .ok_or_else(|| Status::invalid_argument("Invalid environment"))?;

        let store = self.state.stores.get(&env_str.to_string())
            .ok_or_else(|| Status::not_found("Environment not found"))?;

        let total = store.count(&InstrumentQuery::new()).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut by_status = HashMap::new();
        for (status, status_enum) in [
            ("active", InstrumentStatus::Active),
            ("inactive", InstrumentStatus::Inactive),
            ("expired", InstrumentStatus::Expired),
            ("settled", InstrumentStatus::Settled),
        ] {
            let count = store.count(&InstrumentQuery::new().with_status(status_enum)).await
                .map_err(|e| Status::internal(e.to_string()))?;
            by_status.insert(status.to_string(), count as u32);
        }

        let mut by_underlying = HashMap::new();
        for symbol in &["BTC", "ETH", "SOL"] {
            let count = store.count(&InstrumentQuery::new().with_underlying(*symbol)).await
                .map_err(|e| Status::internal(e.to_string()))?;
            if count > 0 {
                by_underlying.insert(symbol.to_string(), count as u32);
            }
        }

        Ok(Response::new(GetStatisticsResponse {
            success: true,
            total: total as u32,
            by_status,
            by_underlying,
            error_message: String::new(),
        }))
    }

    async fn update_instrument_status(
        &self,
        request: Request<UpdateStatusRequest>,
    ) -> Result<Response<UpdateStatusResponse>, Status> {
        let req = request.into_inner();
        let env_str = Self::proto_to_env(req.environment)
            .ok_or_else(|| Status::invalid_argument("Invalid environment"))?;

        let store = self.state.stores.get(&env_str.to_string())
            .ok_or_else(|| Status::not_found("Environment not found"))?;

        let new_status = match InstrumentStatusProto::try_from(req.status).ok() {
            Some(InstrumentStatusProto::Active) => InstrumentStatus::Active,
            Some(InstrumentStatusProto::Inactive) => InstrumentStatus::Inactive,
            Some(InstrumentStatusProto::Expired) => InstrumentStatus::Expired,
            Some(InstrumentStatusProto::Settled) => InstrumentStatus::Settled,
            _ => return Err(Status::invalid_argument("Invalid status")),
        };

        let id = InstrumentId::new(&req.id);
        store.update_status(&id, new_status).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let instrument = store.get(&id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateStatusResponse {
            success: instrument.is_some(),
            instrument: instrument.as_ref().map(Self::domain_to_proto),
            error_message: String::new(),
        }))
    }

    async fn force_regenerate(
        &self,
        request: Request<ForceRegenerateRequest>,
    ) -> Result<Response<ForceRegenerateResponse>, Status> {
        let req = request.into_inner();

        let worker = self.state.worker.as_ref()
            .ok_or_else(|| Status::unavailable("Worker not available"))?;

        let (created, updated) = worker
            .force_regenerate(&req.underlying, req.spot_price)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ForceRegenerateResponse {
            success: true,
            instruments_created: created as u32,
            instruments_updated: updated as u32,
            error_message: String::new(),
        }))
    }
}
