//! PostgreSQL order store implementation

#[cfg(feature = "postgres")]
use async_trait::async_trait;
#[cfg(feature = "postgres")]
use sqlx::{postgres::PgPool, Row};
#[cfg(feature = "postgres")]
use std::sync::Arc;
#[cfg(feature = "postgres")]
use uuid::Uuid;
#[cfg(feature = "postgres")]
use crate::types::{Order, OrderFill, OrderStatus, Environment};
#[cfg(feature = "postgres")]
use crate::store::traits::{OrderStore, OmsResult};
#[cfg(feature = "postgres")]
use crate::error::OmsError;

/// PostgreSQL order store
#[cfg(feature = "postgres")]
pub struct PostgresOrderStore {
    pool: Arc<PgPool>,
}

#[cfg(feature = "postgres")]
impl PostgresOrderStore {
    /// Create a new PostgreSQL order store
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    /// Get table name for environment
    fn table_name(&self, env: Environment) -> String {
        format!("orders_{}", env.table_suffix())
    }

    /// Get fills table name for environment
    fn fills_table_name(&self, env: Environment) -> String {
        format!("order_fills_{}", env.table_suffix())
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl OrderStore for PostgresOrderStore {
    async fn create(&self, order: Order, env: Environment) -> OmsResult<Order> {
        let table = self.table_name(env);
        
        let result = sqlx::query(&format!(
            r#"
            INSERT INTO {} (
                order_id, user_id, instrument_id, side, order_type, time_in_force,
                price, quantity, filled_quantity, avg_fill_price, status,
                client_order_id, risk_approved_at, risk_rejection_reason,
                required_margin, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            RETURNING order_id
            "#,
            table
        ))
            .bind(order.order_id)
            .bind(order.user_id)
            .bind(&order.instrument_id)
            .bind(format!("{:?}", order.side).to_lowercase())
            .bind(format!("{:?}", order.order_type).to_lowercase())
            .bind(format!("{:?}", order.time_in_force).to_lowercase())
            .bind(order.price)
            .bind(order.quantity as i32)
            .bind(order.filled_quantity as i32)
            .bind(order.avg_fill_price)
            .bind(format!("{:?}", order.status).to_lowercase())
            .bind(&order.client_order_id)
            .bind(order.risk_approved_at)
            .bind(&order.risk_rejection_reason)
            .bind(order.required_margin)
            .bind(order.created_at)
            .bind(order.updated_at)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        let _order_id: Uuid = result.get("order_id");
        Ok(order)
    }

    async fn get(&self, order_id: Uuid, env: Environment) -> OmsResult<Option<Order>> {
        let table = self.table_name(env);
        
        let result = sqlx::query(&format!(
            "SELECT * FROM {} WHERE order_id = $1",
            table
        ))
            .bind(order_id)
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        match result {
            Some(row) => Ok(Some(self.row_to_order(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_by_client_order_id(
        &self, 
        user_id: Uuid, 
        client_order_id: &str, 
        env: Environment
    ) -> OmsResult<Option<Order>> {
        let table = self.table_name(env);
        
        let result = sqlx::query(&format!(
            "SELECT * FROM {} WHERE user_id = $1 AND client_order_id = $2",
            table
        ))
            .bind(user_id)
            .bind(client_order_id)
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        match result {
            Some(row) => Ok(Some(self.row_to_order(&row)?)),
            None => Ok(None),
        }
    }

    async fn update(&self, order: &Order, env: Environment) -> OmsResult<()> {
        let table = self.table_name(env);
        
        sqlx::query(&format!(
            r#"
            UPDATE {} SET
                filled_quantity = $1,
                avg_fill_price = $2,
                status = $3,
                risk_approved_at = $4,
                risk_rejection_reason = $5,
                required_margin = $6,
                updated_at = $7
            WHERE order_id = $8
            "#,
            table
        ))
            .bind(order.filled_quantity as i32)
            .bind(order.avg_fill_price)
            .bind(format!("{:?}", order.status).to_lowercase())
            .bind(order.risk_approved_at)
            .bind(&order.risk_rejection_reason)
            .bind(order.required_margin)
            .bind(order.updated_at)
            .bind(order.order_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        Ok(())
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
        let table = self.table_name(env);
        
        let mut conditions: Vec<String> = Vec::new();

        if user_id.is_some() {
            conditions.push("user_id = $1".to_string());
        }

        if instrument_id.is_some() {
            conditions.push("instrument_id = $2".to_string());
        }

        if let Some(ref status_list) = statuses {
            let status_strs: Vec<String> = status_list.iter()
                .map(|s| format!("'{}'", format!("{:?}", s).to_lowercase()))
                .collect();
            conditions.push(format!("status IN ({})", status_strs.join(", ")));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let query = format!(
            "SELECT * FROM {} {} ORDER BY created_at DESC LIMIT {} OFFSET {}",
            table, where_clause, limit, offset
        );

        // Simple query without complex params for now
        let rows = sqlx::query(&query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        rows.iter()
            .map(|row| self.row_to_order(row))
            .collect()
    }

    async fn get_active_orders(&self, user_id: Uuid, env: Environment) -> OmsResult<Vec<Order>> {
        let table = self.table_name(env);
        
        let rows = sqlx::query(&format!(
            "SELECT * FROM {} WHERE user_id = $1 AND status IN ('pending_risk', 'open', 'partially_filled')",
            table
        ))
            .bind(user_id)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        rows.iter()
            .map(|row| self.row_to_order(row))
            .collect()
    }

    async fn get_active_orders_for_instrument(
        &self, 
        instrument_id: &str, 
        env: Environment
    ) -> OmsResult<Vec<Order>> {
        let table = self.table_name(env);
        
        let rows = sqlx::query(&format!(
            "SELECT * FROM {} WHERE instrument_id = $1 AND status IN ('open', 'partially_filled')",
            table
        ))
            .bind(instrument_id)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        rows.iter()
            .map(|row| self.row_to_order(row))
            .collect()
    }

    async fn create_fill(&self, fill: OrderFill, env: Environment) -> OmsResult<OrderFill> {
        let table = self.fills_table_name(env);
        
        sqlx::query(&format!(
            r#"
            INSERT INTO {} (
                fill_id, order_id, trade_id, quantity, price,
                counterparty_order_id, fee, fee_currency, is_maker,
                executed_at, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
            table
        ))
            .bind(fill.fill_id)
            .bind(fill.order_id)
            .bind(fill.trade_id)
            .bind(fill.quantity as i32)
            .bind(fill.price)
            .bind(fill.counterparty_order_id)
            .bind(fill.fee)
            .bind(&fill.fee_currency)
            .bind(fill.is_maker)
            .bind(fill.executed_at)
            .bind(fill.created_at)
            .execute(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        Ok(fill)
    }

    async fn get_fills(&self, order_id: Uuid, env: Environment) -> OmsResult<Vec<OrderFill>> {
        let table = self.fills_table_name(env);
        
        let rows = sqlx::query(&format!(
            "SELECT * FROM {} WHERE order_id = $1 ORDER BY executed_at ASC",
            table
        ))
            .bind(order_id)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        rows.iter()
            .map(|row| self.row_to_fill(row))
            .collect()
    }

    async fn count(
        &self,
        user_id: Option<Uuid>,
        statuses: Option<Vec<OrderStatus>>,
        env: Environment,
    ) -> OmsResult<u64> {
        let table = self.table_name(env);
        
        let query = if user_id.is_some() || statuses.is_some() {
            let mut conditions: Vec<String> = Vec::new();
            
            if user_id.is_some() {
                conditions.push("user_id IS NOT NULL".to_string());
            }
            
            if let Some(ref status_list) = statuses {
                let status_strs: Vec<String> = status_list.iter()
                    .map(|s| format!("'{}'", format!("{:?}", s).to_lowercase()))
                    .collect();
                conditions.push(format!("status IN ({})", status_strs.join(", ")));
            }
            
            format!("SELECT COUNT(*) FROM {} WHERE {}", table, conditions.join(" AND "))
        } else {
            format!("SELECT COUNT(*) FROM {}", table)
        };

        let row = sqlx::query(&query)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| OmsError::StorageError(e.to_string()))?;

        let count: i64 = row.get("count");
        Ok(count as u64)
    }
}

#[cfg(feature = "postgres")]
impl PostgresOrderStore {
    fn row_to_order(&self, row: &sqlx::postgres::PgRow) -> OmsResult<Order> {
        use common::types::{Side, OrderType, TimeInForce};
        
        let side_str: String = row.get("side");
        let order_type_str: String = row.get("order_type");
        let time_in_force_str: String = row.get("time_in_force");
        let status_str: String = row.get("status");

        let side = match side_str.as_str() {
            "buy" => Side::Buy,
            _ => Side::Sell,
        };

        let order_type = match order_type_str.as_str() {
            "limit" => OrderType::Limit,
            _ => OrderType::Market,
        };

        let time_in_force = match time_in_force_str.as_str() {
            "gtc" => TimeInForce::Gtc,
            "ioc" => TimeInForce::Ioc,
            "fok" => TimeInForce::Fok,
            "day" => TimeInForce::Day,
            _ => TimeInForce::Gtc,
        };

        let status = match status_str.as_str() {
            "pending_risk" => OrderStatus::PendingRisk,
            "open" => OrderStatus::Open,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "cancelled" => OrderStatus::Cancelled,
            "rejected" => OrderStatus::Rejected,
            "expired" => OrderStatus::Expired,
            _ => OrderStatus::PendingRisk,
        };

        Ok(Order {
            order_id: row.get("order_id"),
            user_id: row.get("user_id"),
            instrument_id: row.get("instrument_id"),
            side,
            order_type,
            time_in_force,
            price: row.get("price"),
            quantity: row.get::<i32, _>("quantity") as u32,
            filled_quantity: row.get::<i32, _>("filled_quantity") as u32,
            avg_fill_price: row.get("avg_fill_price"),
            status,
            client_order_id: row.get("client_order_id"),
            risk_approved_at: row.get("risk_approved_at"),
            risk_rejection_reason: row.get("risk_rejection_reason"),
            required_margin: row.get("required_margin"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    fn row_to_fill(&self, row: &sqlx::postgres::PgRow) -> OmsResult<OrderFill> {
        Ok(OrderFill {
            fill_id: row.get("fill_id"),
            order_id: row.get("order_id"),
            trade_id: row.get("trade_id"),
            quantity: row.get::<i32, _>("quantity") as u32,
            price: row.get("price"),
            counterparty_order_id: row.get("counterparty_order_id"),
            fee: row.get("fee"),
            fee_currency: row.get("fee_currency"),
            is_maker: row.get("is_maker"),
            executed_at: row.get("executed_at"),
            created_at: row.get("created_at"),
        })
    }
}
