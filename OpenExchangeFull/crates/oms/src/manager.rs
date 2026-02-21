//! Order Manager - core business logic for order handling

use std::sync::Arc;
use uuid::Uuid;
use crate::types::{Order, OrderFill, OrderStatus, Environment};
use crate::store::traits::{OrderStore, OmsResult};
use crate::clients::risk::RiskClient;
use crate::clients::matching::MatchingClient;
use crate::error::OmsError;
use common::addressbook::AddressBook;

/// Order Manager - handles order lifecycle
pub struct OrderManager {
    order_store: Arc<dyn OrderStore>,
    risk_client: Arc<dyn RiskClient>,
    matching_client: Arc<dyn MatchingClient>,
    address_book: Arc<AddressBook>,
}

impl OrderManager {
    /// Create a new OrderManager
    pub fn new(
        order_store: Arc<dyn OrderStore>,
        risk_client: Arc<dyn RiskClient>,
        matching_client: Arc<dyn MatchingClient>,
        address_book: Arc<AddressBook>,
    ) -> Self {
        Self {
            order_store,
            risk_client,
            matching_client,
            address_book,
        }
    }

    /// Submit a new order
    ///
    /// Flow:
    /// 1. Validate order (basic validation)
    /// 2. Store with PendingRisk status
    /// 3. Call risk engine
    /// 4. If approved: update to Open, send to matching
    /// 5. If rejected: update to Rejected
    pub async fn submit_order(
        &self,
        mut order: Order,
        env: Environment,
    ) -> OmsResult<Order> {
        tracing::info!("Submitting order {} for user {}", order.order_id, order.user_id);

        // Step 1: Basic validation
        self.validate_order(&order)?;

        // Step 2: Store with PendingRisk status
        order.status = OrderStatus::PendingRisk;
        let mut order = self.order_store.create(order, env).await?;

        // Step 3: Check risk
        let risk_result = self.risk_client
            .check_order(&order, &order.instrument_id)
            .await?;

        // Step 4: Update based on risk result
        if risk_result.approved {
            order.status = OrderStatus::Open;
            order.risk_approved_at = Some(chrono::Utc::now());
            order.required_margin = risk_result.required_margin;
            self.order_store.update(&order, env).await?;

            // Step 5: Send to matching engine
            self.matching_client
                .submit_order(&order)
                .await?;

            tracing::info!("Order {} approved and sent to matching", order.order_id);
        } else {
            let rejection_reason = risk_result.reason.clone();
            order.status = OrderStatus::Rejected;
            order.risk_rejection_reason = risk_result.reason;
            self.order_store.update(&order, env).await?;

            tracing::warn!("Order {} rejected by risk: {:?}", order.order_id, rejection_reason);
        }

        Ok(order)
    }

    /// Cancel an order
    pub async fn cancel_order(
        &self,
        order_id: Uuid,
        env: Environment,
    ) -> OmsResult<Order> {
        tracing::info!("Cancelling order {}", order_id);

        // Get order
        let mut order = self.order_store
            .get(order_id, env)
            .await?
            .ok_or(OmsError::NotFound(order_id))?;

        // Check if cancellable
        if !order.can_cancel() {
            return Err(OmsError::OrderNotCancellable(
                format!("Cannot cancel order in {:?} status", order.status)
            ));
        }

        // Release margin if locked
        // Note: In a full implementation, we'd need to store the margin_lock_id 
        // from the RiskCheckResult and release it here
        // For now, we skip this as required_margin is the margin amount, not the lock ID

        // Cancel in matching engine
        self.matching_client.cancel_order(order_id).await?;

        // Update order status
        order.status = OrderStatus::Cancelled;
        order.updated_at = chrono::Utc::now();
        self.order_store.update(&order, env).await?;

        tracing::info!("Order {} cancelled", order_id);

        Ok(order)
    }

    /// Apply a fill from matching engine
    pub async fn apply_fill(
        &self,
        order_id: Uuid,
        fill: OrderFill,
        env: Environment,
    ) -> OmsResult<Order> {
        tracing::info!("Applying fill to order {}: {} @ {}", 
            order_id, fill.quantity, fill.price);

        // Get order
        let mut order = self.order_store
            .get(order_id, env)
            .await?
            .ok_or(OmsError::NotFound(order_id))?;

        // Check if order can receive fills
        if !order.is_active() {
            return Err(OmsError::InvalidState(
                format!("Cannot apply fill to order in {:?} status", order.status)
            ));
        }

        // Apply fill to order
        order.apply_fill(fill.quantity, fill.price);
        self.order_store.update(&order, env).await?;

        // Store fill record
        self.order_store.create_fill(fill, env).await?;

        tracing::info!("Order {} now has {} filled of {} total", 
            order.order_id, order.filled_quantity, order.quantity);

        Ok(order)
    }

    /// Get an order by ID
    pub async fn get_order(
        &self,
        order_id: Uuid,
        env: Environment,
    ) -> OmsResult<Option<Order>> {
        self.order_store.get(order_id, env).await
    }

    /// List orders with filters
    pub async fn list_orders(
        &self,
        user_id: Option<Uuid>,
        instrument_id: Option<&str>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
        limit: u32,
        offset: u32,
    ) -> OmsResult<Vec<Order>> {
        // Apply default limits
        let limit = limit.min(500).max(1);
        
        self.order_store
            .list(user_id, instrument_id, statuses, env, limit, offset)
            .await
    }

    /// Get active orders for a user
    pub async fn get_active_orders(
        &self,
        user_id: Uuid,
        env: Environment,
    ) -> OmsResult<Vec<Order>> {
        self.order_store.get_active_orders(user_id, env).await
    }

    /// Get fills for an order
    pub async fn get_fills(
        &self,
        order_id: Uuid,
        env: Environment,
    ) -> OmsResult<Vec<OrderFill>> {
        self.order_store.get_fills(order_id, env).await
    }

    /// Validate basic order parameters
    fn validate_order(&self, order: &Order) -> OmsResult<()> {
        // Validate quantity
        if order.quantity == 0 {
            return Err(OmsError::ValidationError("Quantity must be greater than 0".to_string()));
        }

        // Validate price for limit orders
        if order.order_type == common::types::OrderType::Limit {
            if order.price.is_none() {
                return Err(OmsError::ValidationError("Limit orders require a price".to_string()));
            }
            if let Some(price) = order.price {
                if price <= 0.0 {
                    return Err(OmsError::ValidationError("Price must be greater than 0".to_string()));
                }
            }
        }

        // Validate market orders have no price
        if order.order_type == common::types::OrderType::Market && order.price.is_some() {
            return Err(OmsError::ValidationError("Market orders should not have a price".to_string()));
        }

        // Validate instrument ID is not empty
        if order.instrument_id.is_empty() {
            return Err(OmsError::ValidationError("Instrument ID is required".to_string()));
        }

        Ok(())
    }

    /// Get the address book
    pub fn address_book(&self) -> &Arc<AddressBook> {
        &self.address_book
    }
}

/// Create an OrderManager with mock clients (for testing/development)
pub fn create_with_mocks(
    order_store: Arc<dyn OrderStore>,
) -> OrderManager {
    OrderManager::new(
        order_store,
        Arc::new(crate::clients::risk::MockRiskClient::new()),
        Arc::new(crate::clients::matching::MockMatchingClient::new()),
        AddressBook::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::memory::InMemoryOrderStore;
    use uuid::Uuid;
    use common::types::{Side, OrderType, TimeInForce};

    fn create_test_order() -> Order {
        Order::new(
            Uuid::new_v4(),
            "BTC-20260315-50000-C".to_string(),
            Side::Buy,
            OrderType::Limit,
            TimeInForce::Gtc,
            Some(150.0),
            10,
        )
    }

    #[tokio::test]
    async fn test_submit_order_approved() {
        let store = Arc::new(InMemoryOrderStore::new());
        let manager = create_with_mocks(store.clone());
        
        let order = create_test_order();
        
        let result = manager.submit_order(order, Environment::Static).await.unwrap();
        
        assert_eq!(result.status, OrderStatus::Open);
    }

    #[tokio::test]
    async fn test_submit_order_rejected() {
        let store = Arc::new(InMemoryOrderStore::new());
        let risk_client = Arc::new(
            crate::clients::risk::MockRiskClient::new()
                .with_approval(false)
                .with_rejection_reason("Insufficient margin")
        );
        
        let manager = OrderManager::new(
            store,
            risk_client,
            Arc::new(crate::clients::matching::MockMatchingClient::new()),
            AddressBook::new(),
        );
        
        let order = create_test_order();
        
        let result = manager.submit_order(order, Environment::Static).await.unwrap();
        
        assert_eq!(result.status, OrderStatus::Rejected);
        assert!(result.risk_rejection_reason.is_some());
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let store = Arc::new(InMemoryOrderStore::new());
        let manager = create_with_mocks(store.clone());
        
        // First submit an order
        let order = create_test_order();
        let order = manager.submit_order(order, Environment::Static).await.unwrap();
        
        // Now cancel it
        let cancelled = manager.cancel_order(order.order_id, Environment::Static).await.unwrap();
        
        assert_eq!(cancelled.status, OrderStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_filled_order_fails() {
        let store = Arc::new(InMemoryOrderStore::new());
        
        // Create and fill an order directly
        let mut order = create_test_order();
        order.status = OrderStatus::Filled;
        store.create(order.clone(), Environment::Static).await.unwrap();
        
        let manager = create_with_mocks(store);
        
        // Try to cancel - should fail
        let result = manager.cancel_order(order.order_id, Environment::Static).await;
        
        assert!(result.is_err());
    }
}
