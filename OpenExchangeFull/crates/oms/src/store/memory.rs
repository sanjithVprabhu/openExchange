//! In-memory order store implementation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;
use crate::types::{Order, OrderFill, OrderStatus, Environment};
use crate::store::traits::{OrderStore, OmsResult};
use crate::error::OmsError;

/// In-memory order store for testing and development
pub struct InMemoryOrderStore {
    orders: RwLock<HashMap<Environment, HashMap<Uuid, Order>>>,
    fills: RwLock<HashMap<Environment, HashMap<Uuid, Vec<OrderFill>>>>,
    client_order_ids: RwLock<HashMap<Environment, HashMap<(Uuid, String), Uuid>>>,
}

impl InMemoryOrderStore {
    /// Create a new in-memory order store
    pub fn new() -> Self {
        Self {
            orders: RwLock::new(HashMap::new()),
            fills: RwLock::new(HashMap::new()),
            client_order_ids: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryOrderStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OrderStore for InMemoryOrderStore {
    async fn create(&self, order: Order, env: Environment) -> OmsResult<Order> {
        let order_id = order.order_id;
        
        // Store order
        {
            let mut orders = self.orders.write().unwrap();
            orders.entry(env).or_insert_with(HashMap::new).insert(order_id, order.clone());
        }

        // Store client order ID mapping if present
        if let Some(ref client_order_id) = order.client_order_id {
            let mut client_ids = self.client_order_ids.write().unwrap();
            client_ids.entry(env).or_insert_with(HashMap::new)
                .insert((order.user_id, client_order_id.clone()), order_id);
        }

        // Initialize fills list
        {
            let mut fills = self.fills.write().unwrap();
            fills.entry(env).or_insert_with(HashMap::new).insert(order_id, Vec::new());
        }

        Ok(order)
    }

    async fn get(&self, order_id: Uuid, env: Environment) -> OmsResult<Option<Order>> {
        let orders = self.orders.read().unwrap();
        Ok(orders.get(&env).and_then(|m| m.get(&order_id).cloned()))
    }

    async fn get_by_client_order_id(
        &self, 
        user_id: Uuid, 
        client_order_id: &str, 
        env: Environment
    ) -> OmsResult<Option<Order>> {
        // Extract order_id in a separate scope so the lock guard is dropped before await
        let order_id = {
            let client_ids = self.client_order_ids.read().unwrap();
            client_ids
                .get(&env)
                .and_then(|m| m.get(&(user_id, client_order_id.to_string())))
                .copied()
        };

        if let Some(order_id) = order_id {
            self.get(order_id, env).await
        } else {
            Ok(None)
        }
    }

    async fn update(&self, order: &Order, env: Environment) -> OmsResult<()> {
        let mut orders = self.orders.write().unwrap();
        let env_orders = orders.entry(env).or_insert_with(HashMap::new);
        
        if env_orders.contains_key(&order.order_id) {
            env_orders.insert(order.order_id, order.clone());
            Ok(())
        } else {
            Err(OmsError::NotFound(order.order_id))
        }
    }

    async fn list(
        &self,
        user_id: Option<Uuid>,
        instrument_id: Option<&str>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
        limit: u32,
        offset: u32,
    ) -> OmsResult<Vec<Order>> {
        let orders = self.orders.read().unwrap();
        let env_orders = orders.get(&env);
        
        let mut result: Vec<Order> = env_orders
            .map(|m| m.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        // Filter by user
        if let Some(uid) = user_id {
            result.retain(|o| o.user_id == uid);
        }

        // Filter by instrument
        if let Some(iid) = instrument_id {
            result.retain(|o| o.instrument_id == iid);
        }

        // Filter by status
        if let Some(ref status_list) = statuses {
            let status_set: std::collections::HashSet<_> = status_list.iter().collect();
            result.retain(|o| status_set.contains(&o.status));
        }

        // Sort by created_at descending
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply pagination
        let start = offset as usize;
        let end = (offset + limit) as usize;

        Ok(result.into_iter().skip(start).take(end.saturating_sub(start)).collect())
    }

    async fn get_active_orders(&self, user_id: Uuid, env: Environment) -> OmsResult<Vec<Order>> {
        let orders = self.orders.read().unwrap();
        let env_orders = orders.get(&env);
        
        let result: Vec<Order> = env_orders
            .map(|m| {
                m.values()
                    .filter(|o| {
                        o.user_id == user_id && matches!(
                            o.status,
                            OrderStatus::PendingRisk | OrderStatus::Open | OrderStatus::PartiallyFilled
                        )
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        Ok(result)
    }

    async fn get_active_orders_for_instrument(
        &self, 
        instrument_id: &str, 
        env: Environment
    ) -> OmsResult<Vec<Order>> {
        let orders = self.orders.read().unwrap();
        let env_orders = orders.get(&env);
        
        let result: Vec<Order> = env_orders
            .map(|m| {
                m.values()
                    .filter(|o| {
                        o.instrument_id == instrument_id && matches!(
                            o.status,
                            OrderStatus::Open | OrderStatus::PartiallyFilled
                        )
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        Ok(result)
    }

    async fn create_fill(&self, fill: OrderFill, env: Environment) -> OmsResult<OrderFill> {
        let mut fills = self.fills.write().unwrap();
        let env_fills = fills.entry(env).or_insert_with(HashMap::new);
        
        env_fills
            .entry(fill.order_id)
            .or_insert_with(Vec::new)
            .push(fill.clone());

        Ok(fill)
    }

    async fn get_fills(&self, order_id: Uuid, env: Environment) -> OmsResult<Vec<OrderFill>> {
        let fills = self.fills.read().unwrap();
        Ok(fills
            .get(&env)
            .and_then(|m| m.get(&order_id).cloned())
            .unwrap_or_default())
    }

    async fn count(
        &self,
        user_id: Option<Uuid>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
    ) -> OmsResult<u64> {
        let orders = self.orders.read().unwrap();
        let env_orders = orders.get(&env);
        
        let mut count = env_orders.map(|m| m.len()).unwrap_or(0) as u64;

        if let Some(uid) = user_id {
            count = env_orders
                .map(|m| m.values().filter(|o| o.user_id == uid).count() as u64)
                .unwrap_or(0);
        }

        if let Some(ref status_list) = statuses {
            let status_set: std::collections::HashSet<_> = status_list.iter().collect();
            count = env_orders
                .map(|m| {
                    m.values()
                        .filter(|o| status_set.contains(&o.status))
                        .count() as u64
                })
                .unwrap_or(0);
        }

        Ok(count)
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
    async fn test_create_and_get() {
        let store = InMemoryOrderStore::new();
        let order = create_test_order();
        let order_id = order.order_id;
        
        let created = store.create(order, Environment::Static).await.unwrap();
        assert_eq!(created.order_id, order_id);
        
        let retrieved = store.get(order_id, Environment::Static).await.unwrap();
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_list() {
        let store = InMemoryOrderStore::new();
        
        for i in 0..5 {
            let mut order = create_test_order();
            order.instrument_id = format!("BTC-20260315-{}000-C", i);
            store.create(order, Environment::Static).await.unwrap();
        }
        
        let orders = store.list(None, None, None, Environment::Static, 10, 0).await.unwrap();
        assert_eq!(orders.len(), 5);
    }
}
