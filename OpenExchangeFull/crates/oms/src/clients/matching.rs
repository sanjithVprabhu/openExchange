//! Matching client - trait and implementations

use async_trait::async_trait;
use uuid::Uuid;
use crate::types::Order;
use crate::store::traits::OmsResult;

/// Client trait for Matching Engine - protocol agnostic
#[async_trait]
pub trait MatchingClient: Send + Sync {
    /// Submit an order to the matching engine
    ///
    /// The order should already be in Open status (risk approved).
    /// The matching engine will:
    /// 1. Add to order book
    /// 2. Match against existing orders
    /// 3. Generate fill events
    async fn submit_order(&self, order: &Order) -> OmsResult<()>;
    
    /// Cancel an order in the matching engine
    ///
    /// Removes the order from the order book.
    /// Returns Ok if order was removed or didn't exist.
    async fn cancel_order(&self, order_id: Uuid) -> OmsResult<()>;
    
    /// Modify an order (cancel and replace)
    ///
    /// Atomically cancels old order and submits new one.
    async fn modify_order(
        &self,
        old_order_id: Uuid,
        new_order: &Order,
    ) -> OmsResult<()>;
}

// ==================== Mock Implementation ====================

/// Mock matching client for testing
pub struct MockMatchingClient {
    submitted_orders: std::sync::Mutex<Vec<Uuid>>,
    cancelled_orders: std::sync::Mutex<Vec<Uuid>>,
}

impl MockMatchingClient {
    /// Create a new mock matching client
    pub fn new() -> Self {
        Self {
            submitted_orders: std::sync::Mutex::new(Vec::new()),
            cancelled_orders: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get list of submitted order IDs
    pub fn get_submitted_orders(&self) -> Vec<Uuid> {
        self.submitted_orders.lock().unwrap().clone()
    }

    /// Get list of cancelled order IDs
    pub fn get_cancelled_orders(&self) -> Vec<Uuid> {
        self.cancelled_orders.lock().unwrap().clone()
    }

    /// Clear all tracked orders
    pub fn clear(&self) {
        self.submitted_orders.lock().unwrap().clear();
        self.cancelled_orders.lock().unwrap().clear();
    }
}

impl Default for MockMatchingClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MatchingClient for MockMatchingClient {
    async fn submit_order(&self, order: &Order) -> OmsResult<()> {
        // Simulate some async delay
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        
        self.submitted_orders.lock().unwrap().push(order.order_id);
        
        tracing::debug!("Mock matching: submitted order {}", order.order_id);
        
        Ok(())
    }

    async fn cancel_order(&self, order_id: Uuid) -> OmsResult<()> {
        // Simulate some async delay
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        
        self.cancelled_orders.lock().unwrap().push(order_id);
        
        tracing::debug!("Mock matching: cancelled order {}", order_id);
        
        Ok(())
    }

    async fn modify_order(
        &self,
        old_order_id: Uuid,
        new_order: &Order,
    ) -> OmsResult<()> {
        // Simulate cancel then submit
        self.cancel_order(old_order_id).await?;
        self.submit_order(new_order).await
    }
}

// ==================== HTTP Implementation ====================

#[cfg(feature = "client")]
pub mod http {
    use async_trait::async_trait;
    use reqwest::Client;
    use uuid::Uuid;
    use crate::types::Order;
    use crate::error::OmsError;
    use crate::store::traits::OmsResult;
    use super::MatchingClient;

    /// HTTP-based matching client
    pub struct HttpMatchingClient {
        client: Client,
        base_url: String,
    }

    impl HttpMatchingClient {
        /// Create a new HTTP matching client
        pub fn new(base_url: &str) -> Self {
            Self {
                client: Client::new(),
                base_url: base_url.trim_end_matches('/').to_string(),
            }
        }
    }

    #[async_trait]
    impl MatchingClient for HttpMatchingClient {
        async fn submit_order(&self, order: &Order) -> OmsResult<()> {
            let url = format!("{}/api/v1/internal/orders", self.base_url);
            
            let response = self.client
                .post(&url)
                .json(&order)
                .send()
                .await
                .map_err(|e| OmsError::MatchingUnavailable(e.to_string()))?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(OmsError::MatchingUnavailable(error_text));
            }

            Ok(())
        }

        async fn cancel_order(&self, order_id: Uuid) -> OmsResult<()> {
            let url = format!("{}/api/v1/internal/orders/{}", self.base_url, order_id);
            
            let response = self.client
                .delete(&url)
                .send()
                .await
                .map_err(|e| OmsError::MatchingUnavailable(e.to_string()))?;

            // 404 is ok - order might not be in the book
            if !response.status().is_success() && response.status().as_u16() != 404 {
                let error_text = response.text().await.unwrap_or_default();
                return Err(OmsError::MatchingUnavailable(error_text));
            }

            Ok(())
        }

        async fn modify_order(
            &self,
            old_order_id: Uuid,
            new_order: &Order,
        ) -> OmsResult<()> {
            // Cancel old
            self.cancel_order(old_order_id).await?;
            
            // Submit new
            self.submit_order(new_order).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_mock_submit() {
        let client = MockMatchingClient::new();
        let order = create_test_order();
        
        client.submit_order(&order).await.unwrap();
        
        let submitted = client.get_submitted_orders();
        assert!(submitted.contains(&order.order_id));
    }

    #[tokio::test]
    async fn test_mock_cancel() {
        let client = MockMatchingClient::new();
        let order = create_test_order();
        
        client.submit_order(&order).await.unwrap();
        client.cancel_order(order.order_id).await.unwrap();
        
        let cancelled = client.get_cancelled_orders();
        assert!(cancelled.contains(&order.order_id));
    }
}
