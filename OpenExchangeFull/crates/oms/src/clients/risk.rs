//! Risk client - trait and implementations

use async_trait::async_trait;
use uuid::Uuid;
use crate::types::Order;
use crate::store::traits::OmsResult;

/// Risk check result from the risk engine
#[derive(Debug, Clone)]
pub struct RiskCheckResult {
    /// Whether the order is approved
    pub approved: bool,
    /// Reason for rejection if not approved
    pub reason: Option<String>,
    /// Required margin for this order
    pub required_margin: Option<f64>,
    /// Margin lock ID (for later release)
    pub margin_lock_id: Option<String>,
}

/// Client trait for Risk Engine - protocol agnostic
#[async_trait]
pub trait RiskClient: Send + Sync {
    /// Check if an order passes risk requirements
    ///
    /// This checks:
    /// - User has sufficient margin
    /// - Order doesn't exceed position limits
    /// - Order doesn't violate risk parameters
    ///
    /// If approved, margin is locked.
    async fn check_order(
        &self,
        order: &Order,
        instrument_id: &str,
    ) -> OmsResult<RiskCheckResult>;
    
    /// Release margin lock on order cancellation
    async fn release_margin_lock(
        &self,
        margin_lock_id: &str,
    ) -> OmsResult<()>;
}

// ==================== Mock Implementation ====================

/// Mock risk client for testing
pub struct MockRiskClient {
    always_approve: bool,
    margin_rate: f64,
    rejection_reason: Option<String>,
}

impl MockRiskClient {
    /// Create a new mock risk client
    pub fn new() -> Self {
        Self {
            always_approve: true,
            margin_rate: 0.10,
            rejection_reason: None,
        }
    }

    /// Configure to always approve orders
    pub fn with_approval(mut self, approve: bool) -> Self {
        self.always_approve = approve;
        self
    }

    /// Configure margin rate
    pub fn with_margin_rate(mut self, rate: f64) -> Self {
        self.margin_rate = rate;
        self
    }

    /// Configure rejection reason
    pub fn with_rejection_reason(mut self, reason: impl Into<String>) -> Self {
        self.rejection_reason = Some(reason.into());
        self
    }
}

impl Default for MockRiskClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RiskClient for MockRiskClient {
    async fn check_order(
        &self,
        order: &Order,
        _instrument_id: &str,
    ) -> OmsResult<RiskCheckResult> {
        // Simulate some async delay
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Calculate required margin
        let price = order.price.unwrap_or(100.0);
        let notional = price * order.quantity as f64;
        let required_margin = notional * self.margin_rate;

        Ok(RiskCheckResult {
            approved: self.always_approve,
            reason: if self.always_approve {
                None
            } else {
                self.rejection_reason.clone()
            },
            required_margin: Some(required_margin),
            margin_lock_id: if self.always_approve {
                Some(Uuid::new_v4().to_string())
            } else {
                None
            },
        })
    }

    async fn release_margin_lock(&self, _margin_lock_id: &str) -> OmsResult<()> {
        // Simulate some async delay
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        Ok(())
    }
}

// ==================== HTTP Implementation ====================

#[cfg(feature = "client")]
pub mod http {
    use async_trait::async_trait;
    use reqwest::Client;
    use crate::types::Order;
    use crate::error::OmsError;
    use crate::store::traits::OmsResult;
    use super::RiskClient;
    use super::RiskCheckResult;

    /// HTTP-based risk client
    pub struct HttpRiskClient {
        client: Client,
        base_url: String,
    }

    impl HttpRiskClient {
        /// Create a new HTTP risk client
        pub fn new(base_url: &str) -> Self {
            Self {
                client: Client::new(),
                base_url: base_url.trim_end_matches('/').to_string(),
            }
        }
    }

    #[async_trait]
    impl RiskClient for HttpRiskClient {
        async fn check_order(
            &self,
            order: &Order,
            instrument_id: &str,
        ) -> OmsResult<RiskCheckResult> {
            let url = format!("{}/api/v1/internal/risk/check", self.base_url);
            
            let response = self.client
                .post(&url)
                .json(&serde_json::json!({
                    "order": order,
                    "instrument_id": instrument_id
                }))
                .send()
                .await
                .map_err(|e| OmsError::RiskUnavailable(e.to_string()))?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(OmsError::RiskRejected(error_text));
            }

            response
                .json::<RiskCheckResult>()
                .await
                .map_err(|e| OmsError::RiskUnavailable(e.to_string()))
        }

        async fn release_margin_lock(&self, margin_lock_id: &str) -> OmsResult<()> {
            let url = format!("{}/api/v1/internal/risk/release", self.base_url);
            
            self.client
                .post(&url)
                .json(&serde_json::json!({ "margin_lock_id": margin_lock_id }))
                .send()
                .await
                .map_err(|e| OmsError::RiskUnavailable(e.to_string()))?;

            Ok(())
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
    async fn test_mock_approve() {
        let client = MockRiskClient::new().with_approval(true);
        let order = create_test_order();
        
        let result = client.check_order(&order, "BTC").await.unwrap();
        
        assert!(result.approved);
        assert!(result.required_margin.is_some());
        assert!(result.margin_lock_id.is_some());
    }

    #[tokio::test]
    async fn test_mock_reject() {
        let client = MockRiskClient::new()
            .with_approval(false)
            .with_rejection_reason("Insufficient margin");
        
        let order = create_test_order();
        
        let result = client.check_order(&order, "BTC").await.unwrap();
        
        assert!(!result.approved);
        assert_eq!(result.reason, Some("Insufficient margin".to_string()));
    }
}
