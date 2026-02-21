//! OrderStore trait definition

use async_trait::async_trait;
use uuid::Uuid;
use crate::types::{Order, OrderFill, OrderStatus, Environment};
use crate::error::OmsError;

/// OrderStore trait - defines the interface for order storage
///
/// This trait allows different storage implementations (in-memory, PostgreSQL, etc.)
/// to be swapped without changing the business logic.
#[async_trait]
pub trait OrderStore: Send + Sync {
    /// Create a new order
    ///
    /// # Arguments
    /// * `order` - The order to create
    /// * `env` - The environment (prod/virtual/static)
    ///
    /// # Returns
    /// The created order with ID assigned
    async fn create(&self, order: Order, env: Environment) -> OmsResult<Order>;
    
    /// Get an order by ID
    ///
    /// # Arguments
    /// * `order_id` - The order ID to look up
    /// * `env` - The environment
    ///
    /// # Returns
    /// The order if found, None otherwise
    async fn get(&self, order_id: Uuid, env: Environment) -> OmsResult<Option<Order>>;
    
    /// Get an order by client order ID
    ///
    /// # Arguments
    /// * `user_id` - The user ID
    /// * `client_order_id` - The client-specified order ID
    /// * `env` - The environment
    ///
    /// # Returns
    /// The order if found, None otherwise
    async fn get_by_client_order_id(
        &self, 
        user_id: Uuid, 
        client_order_id: &str, 
        env: Environment
    ) -> OmsResult<Option<Order>>;
    
    /// Update an existing order
    ///
    /// # Arguments
    /// * `order` - The order to update
    /// * `env` - The environment
    async fn update(&self, order: &Order, env: Environment) -> OmsResult<()>;
    
    /// List orders with filters
    ///
    /// # Arguments
    /// * `user_id` - Filter by user (None for all users)
    /// * `instrument_id` - Filter by instrument
    /// * `statuses` - Filter by statuses
    /// * `env` - The environment
    /// * `limit` - Maximum number of results
    /// * `offset` - Pagination offset
    async fn list(
        &self,
        user_id: Option<Uuid>,
        instrument_id: Option<&str>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
        limit: u32,
        offset: u32,
    ) -> OmsResult<Vec<Order>>;
    
    /// Get active orders for a user
    ///
    /// # Arguments
    /// * `user_id` - The user ID
    /// * `env` - The environment
    async fn get_active_orders(&self, user_id: Uuid, env: Environment) -> OmsResult<Vec<Order>>;
    
    /// Get active orders for an instrument
    ///
    /// # Arguments
    /// * `instrument_id` - The instrument ID
    /// * `env` - The environment
    async fn get_active_orders_for_instrument(
        &self, 
        instrument_id: &str, 
        env: Environment
    ) -> OmsResult<Vec<Order>>;
    
    /// Create a fill record
    ///
    /// # Arguments
    /// * `fill` - The fill to create
    /// * `env` - The environment
    async fn create_fill(&self, fill: OrderFill, env: Environment) -> OmsResult<OrderFill>;
    
    /// Get fills for an order
    ///
    /// # Arguments
    /// * `order_id` - The order ID
    /// * `env` - The environment
    async fn get_fills(&self, order_id: Uuid, env: Environment) -> OmsResult<Vec<OrderFill>>;
    
    /// Count orders matching filters
    ///
    /// # Arguments
    /// * `user_id` - Filter by user
    /// * `statuses` - Filter by statuses
    /// * `env` - The environment
    async fn count(
        &self,
        user_id: Option<Uuid>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
    ) -> OmsResult<u64>;
}

/// Result type for OrderStore operations
pub type OmsResult<T> = std::result::Result<T, OmsError>;
