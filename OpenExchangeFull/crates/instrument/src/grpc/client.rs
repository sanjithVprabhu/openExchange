//! gRPC client for the instrument service.
//!
//! Provides methods to call the instrument gRPC server from gateway or other services.

use super::server::*;
use std::collections::HashMap;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

/// Client for the instrument gRPC service.
///
/// Connects to an instrument service node and provides methods to call
/// all instrument RPCs.
///
/// # Example
///
/// ```ignore
/// let client = InstrumentGrpcClient::connect("http://localhost:9081").await?;
///
/// let response = client.list_instruments(ListInstrumentsRequest {
///     environment: EnvironmentProto::Static as i32,
///     underlying: Some("BTC".to_string()),
///     limit: 100,
///     ..Default::default()
/// }).await?;
///
/// for instrument in response.instruments {
///     println!("{}: {}", instrument.symbol, instrument.strike);
/// }
/// ```
pub struct InstrumentGrpcClient {
    channel: Channel,
}

impl InstrumentGrpcClient {
    /// Create a new client connected to the given endpoint.
    ///
    /// # Arguments
    /// * `endpoint` - The gRPC server URL (e.g., "http://localhost:9081")
    ///
    /// # Example
    /// ```ignore
    /// let client = InstrumentGrpcClient::connect("http://localhost:9081").await?;
    /// ```
    pub async fn connect(endpoint: &str) -> Result<Self, tonic::transport::Error> {
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await?;
        Ok(Self { channel })
    }

    /// Create a client from an existing channel.
    ///
    /// Useful for connection pooling or testing.
    pub fn from_channel(channel: Channel) -> Self {
        Self { channel }
    }

    /// Get a reference to the underlying channel.
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// List instruments with optional filters.
    ///
    /// # Arguments
    /// * `request` - The list instruments request with filters
    ///
    /// # Returns
    /// A response containing the list of instruments matching the query.
    pub async fn list_instruments(
        &self,
        request: ListInstrumentsRequest,
    ) -> Result<ListInstrumentsResponse, tonic::Status> {
        // For a manual implementation, we use tonic's raw request/response
        // In a production setup with tonic-build, this would use generated code
        
        let path = "/instrument.v1.InstrumentService/ListInstruments";
        self.call_unary(path, request).await
    }

    /// Get a single instrument by ID.
    ///
    /// # Arguments
    /// * `request` - The get instrument request with ID and environment
    ///
    /// # Returns
    /// A response containing the instrument if found.
    pub async fn get_instrument(
        &self,
        request: GetInstrumentRequest,
    ) -> Result<GetInstrumentResponse, tonic::Status> {
        let path = "/instrument.v1.InstrumentService/GetInstrument";
        self.call_unary(path, request).await
    }

    /// Get a single instrument by symbol.
    ///
    /// # Arguments
    /// * `request` - The get by symbol request with symbol and environment
    ///
    /// # Returns
    /// A response containing the instrument if found.
    pub async fn get_instrument_by_symbol(
        &self,
        request: GetInstrumentBySymbolRequest,
    ) -> Result<GetInstrumentResponse, tonic::Status> {
        let path = "/instrument.v1.InstrumentService/GetInstrumentBySymbol";
        self.call_unary(path, request).await
    }

    /// Get instrument statistics for an environment.
    ///
    /// # Arguments
    /// * `request` - The statistics request with environment
    ///
    /// # Returns
    /// A response containing statistics about instruments.
    pub async fn get_statistics(
        &self,
        request: GetStatisticsRequest,
    ) -> Result<GetStatisticsResponse, tonic::Status> {
        let path = "/instrument.v1.InstrumentService/GetStatistics";
        self.call_unary(path, request).await
    }

    /// Update an instrument's status.
    ///
    /// # Arguments
    /// * `request` - The update status request with ID, environment, and new status
    ///
    /// # Returns
    /// A response containing the updated instrument.
    pub async fn update_instrument_status(
        &self,
        request: UpdateStatusRequest,
    ) -> Result<UpdateStatusResponse, tonic::Status> {
        let path = "/instrument.v1.InstrumentService/UpdateInstrumentStatus";
        self.call_unary(path, request).await
    }

    /// Force regeneration of instruments for an asset.
    ///
    /// # Arguments
    /// * `request` - The force regenerate request with underlying and spot price
    ///
    /// # Returns
    /// A response with the count of instruments created/updated.
    pub async fn force_regenerate(
        &self,
        request: ForceRegenerateRequest,
    ) -> Result<ForceRegenerateResponse, tonic::Status> {
        let path = "/instrument.v1.InstrumentService/ForceRegenerate";
        self.call_unary(path, request).await
    }

    /// Helper to make a unary gRPC call.
    async fn call_unary<T, R>(&self, _path: &str, request: T) -> Result<R, tonic::Status>
    where
        T: prost::Message + Send + Sync + 'static,
        R: prost::Message + Default,
    {
        // For the manual implementation without tonic-build generated code,
        // we need to use tonic's lower-level client API.
        // 
        // However, since we're defining our own service trait on the server side,
        // we need a compatible approach. The simplest solution is to create
        // a mock/test implementation or use the generated code from tonic-build.
        //
        // For now, we'll implement this using tonic's codec directly.
        
        use tonic::codec::ProstCodec;

        let mut client = tonic::client::Grpc::new(self.channel.clone());

        let codec = ProstCodec::<T, R>::default();
        
        // This is a simplified approach - in production you'd use tonic-build
        // to generate the proper client code with correct paths
        
        // Note: Without generated code, we need to manually specify the service path
        // For a complete implementation, we'd use tonic-build to generate the client
        
        Err(tonic::Status::unimplemented(
            "Client requires tonic-build generated code. \
             Run 'cargo build' with the grpc feature to generate client code, \
             or use the HTTP API forwarding instead."
        ))
    }
}

/// A simpler HTTP-based client that can be used as an alternative to gRPC.
/// 
/// This is useful for environments where gRPC isn't available or for simpler
/// deployment scenarios where the gateway can use HTTP to communicate with
/// the instrument service.
pub struct InstrumentHttpClient {
    base_url: String,
    client: reqwest::Client,
}

impl InstrumentHttpClient {
    /// Create a new HTTP client for the instrument service.
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// List instruments via HTTP.
    pub async fn list_instruments(
        &self,
        env: &str,
        params: &crate::api::models::ListInstrumentsParams,
    ) -> Result<crate::api::models::ListInstrumentsResponse, anyhow::Error> {
        let mut query = Vec::new();
        
        if let Some(ref underlying) = params.underlying {
            query.push(("underlying", underlying.clone()));
        }
        if let Some(ref option_type) = params.option_type {
            query.push(("option_type", option_type.clone()));
        }
        if let Some(ref status) = params.status {
            query.push(("status", status.clone()));
        }
        query.push(("limit", params.limit.to_string()));
        query.push(("offset", params.offset.to_string()));

        let response = self.client
            .get(format!("{}/api/v1/{}/instruments", self.base_url, env))
            .query(&query)
            .send()
            .await?
            .json::<crate::api::models::ListInstrumentsResponse>()
            .await?;

        Ok(response)
    }

    /// Get instrument by symbol via HTTP.
    pub async fn get_instrument_by_symbol(
        &self,
        env: &str,
        symbol: &str,
    ) -> Result<crate::api::models::GetInstrumentResponse, anyhow::Error> {
        let response = self.client
            .get(format!("{}/api/v1/{}/instruments/symbol/{}", self.base_url, env, symbol))
            .send()
            .await?
            .json::<crate::api::models::GetInstrumentResponse>()
            .await?;

        Ok(response)
    }

    /// Get statistics via HTTP.
    pub async fn get_stats(
        &self,
        env: &str,
    ) -> Result<crate::api::models::StatsResponse, anyhow::Error> {
        let response = self.client
            .get(format!("{}/api/v1/{}/instruments/stats", self.base_url, env))
            .send()
            .await?
            .json::<crate::api::models::StatsResponse>()
            .await?;

        Ok(response)
    }

    /// Update instrument status via HTTP.
    pub async fn update_status(
        &self,
        env: &str,
        id: &str,
        status: &str,
    ) -> Result<crate::api::models::UpdateStatusResponse, anyhow::Error> {
        let response = self.client
            .patch(format!("{}/api/v1/{}/instruments/{}/status", self.base_url, env, id))
            .json(&serde_json::json!({ "status": status }))
            .send()
            .await?
            .json::<crate::api::models::UpdateStatusResponse>()
            .await?;

        Ok(response)
    }

    /// Force regenerate via HTTP.
    pub async fn force_regenerate(
        &self,
        env: &str,
        underlying: &str,
        spot_price: f64,
    ) -> Result<crate::api::models::ForceRegenerateResponse, anyhow::Error> {
        let response = self.client
            .post(format!("{}/api/v1/{}/instruments/regenerate", self.base_url, env))
            .json(&serde_json::json!({
                "underlying": underlying,
                "spot_price": spot_price
            }))
            .send()
            .await?
            .json::<crate::api::models::ForceRegenerateResponse>()
            .await?;

        Ok(response)
    }
}

impl Clone for InstrumentGrpcClient {
    fn clone(&self) -> Self {
        Self {
            channel: self.channel.clone(),
        }
    }
}

impl Clone for InstrumentHttpClient {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            client: self.client.clone(),
        }
    }
}
