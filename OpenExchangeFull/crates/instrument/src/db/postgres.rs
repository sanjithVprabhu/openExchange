//! PostgreSQL implementation of the `InstrumentStore` trait.
//!
//! This store uses separate tables per environment (prod, virtual, static)
//! and supports batch operations for efficient instrument generation.

use crate::db::models::{Environment, GenerationState, GenerationStateRow, InstrumentRow};
use crate::error::{InstrumentError, InstrumentResult};
use crate::store::{InstrumentQuery, InstrumentStore};
use crate::types::{InstrumentId, InstrumentStatus, OptionInstrument};
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::str::FromStr;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// PostgreSQL-backed instrument store.
///
/// Supports three environments with separate tables:
/// - `instruments_prod`
/// - `instruments_virtual`
/// - `instruments_static`
#[derive(Debug, Clone)]
pub struct PostgresInstrumentStore {
    pool: PgPool,
    environment: Environment,
}

impl PostgresInstrumentStore {
    /// Create a new PostgresInstrumentStore with a connection pool.
    pub async fn new(
        database_url: &str,
        environment: Environment,
    ) -> Result<Self, InstrumentError> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to connect to database: {}", e)))?;

        info!(
            "Connected to PostgreSQL for environment '{}', table '{}'",
            environment,
            environment.table_name()
        );

        Ok(Self { pool, environment })
    }

    /// Create from an existing connection pool.
    pub fn from_pool(pool: PgPool, environment: Environment) -> Self {
        Self { pool, environment }
    }

    /// Get the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the environment.
    pub fn environment(&self) -> Environment {
        self.environment
    }

    /// Get the table name for the current environment.
    fn table_name(&self) -> &'static str {
        self.environment.table_name()
    }

    /// Run the migration SQL to create tables.
    pub async fn run_migrations(&self) -> InstrumentResult<()> {
        let migration_sql = include_str!("../../../../migrations/001_create_instruments.sql");
        sqlx::raw_sql(migration_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Migration failed: {}", e)))?;
        info!("Database migrations completed successfully");
        Ok(())
    }

    // =========================================================================
    // Generation State Operations
    // =========================================================================

    /// Get the generation state for an asset in this environment.
    pub async fn get_generation_state(
        &self,
        asset_symbol: &str,
    ) -> InstrumentResult<Option<GenerationState>> {
        let row = sqlx::query_as::<_, GenerationStateRow>(
            "SELECT * FROM generation_state WHERE environment = $1 AND asset_symbol = $2",
        )
        .bind(self.environment.as_str())
        .bind(asset_symbol)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InstrumentError::StorageError(format!("Failed to get generation state: {}", e)))?;

        Ok(row.map(|r| r.to_domain()))
    }

    /// Upsert (insert or update) generation state for an asset.
    pub async fn upsert_generation_state(
        &self,
        state: &GenerationState,
    ) -> InstrumentResult<()> {
        let bd = |v: f64| -> sqlx::types::BigDecimal {
            sqlx::types::BigDecimal::from_str(&format!("{:.8}", v)).unwrap_or_default()
        };

        sqlx::query(
            r#"
            INSERT INTO generation_state (
                environment, asset_symbol,
                upper_reference, lower_reference,
                upper_trigger, lower_trigger,
                max_strike, min_strike,
                last_spot_price
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (environment, asset_symbol) DO UPDATE SET
                upper_reference = EXCLUDED.upper_reference,
                lower_reference = EXCLUDED.lower_reference,
                upper_trigger = EXCLUDED.upper_trigger,
                lower_trigger = EXCLUDED.lower_trigger,
                max_strike = EXCLUDED.max_strike,
                min_strike = EXCLUDED.min_strike,
                last_spot_price = EXCLUDED.last_spot_price,
                updated_at = NOW()
            "#,
        )
        .bind(self.environment.as_str())
        .bind(&state.asset_symbol)
        .bind(bd(state.upper_reference))
        .bind(bd(state.lower_reference))
        .bind(bd(state.upper_trigger))
        .bind(bd(state.lower_trigger))
        .bind(bd(state.max_strike))
        .bind(bd(state.min_strike))
        .bind(bd(state.last_spot_price))
        .execute(&self.pool)
        .await
        .map_err(|e| {
            InstrumentError::StorageError(format!("Failed to upsert generation state: {}", e))
        })?;

        debug!(
            "Upserted generation state for {} in {}",
            state.asset_symbol,
            self.environment
        );
        Ok(())
    }

    // =========================================================================
    // Bulk Operations
    // =========================================================================

    /// Update status of multiple instruments by underlying and strike range.
    pub async fn update_status_bulk(
        &self,
        underlying: &str,
        status: InstrumentStatus,
        strike_min: Option<f64>,
        strike_max: Option<f64>,
    ) -> InstrumentResult<u64> {
        let table = self.table_name();
        let mut query = format!(
            "UPDATE {} SET status = $1 WHERE underlying_symbol = $2",
            table
        );
        let mut param_idx = 3;

        if strike_min.is_some() {
            query.push_str(&format!(" AND strike_value >= ${}", param_idx));
            param_idx += 1;
        }
        if strike_max.is_some() {
            query.push_str(&format!(" AND strike_value <= ${}", param_idx));
        }

        let mut q = sqlx::query(&query)
            .bind(status.as_db_str())
            .bind(underlying);

        if let Some(min) = strike_min {
            let bd = sqlx::types::BigDecimal::from_str(&format!("{:.8}", min)).unwrap_or_default();
            q = q.bind(bd);
        }
        if let Some(max) = strike_max {
            let bd = sqlx::types::BigDecimal::from_str(&format!("{:.8}", max)).unwrap_or_default();
            q = q.bind(bd);
        }

        let result = q
            .execute(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to update status bulk: {}", e)))?;

        Ok(result.rows_affected())
    }

    /// Mark all active instruments that have expired as expired.
    pub async fn mark_expired_by_time(&self) -> InstrumentResult<u64> {
        let table = self.table_name();
        let query = format!(
            "UPDATE {} SET status = 'expired' WHERE status = 'active' AND expiry <= NOW()",
            table
        );

        let result = sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to mark expired: {}", e))
            })?;

        let count = result.rows_affected();
        if count > 0 {
            info!("Marked {} instruments as expired in {}", count, table);
        }
        Ok(count)
    }

    /// Set instruments to active if they are in the given strike range,
    /// and inactive if they are outside the range (for a given underlying).
    pub async fn update_active_range(
        &self,
        underlying: &str,
        active_min: f64,
        active_max: f64,
    ) -> InstrumentResult<(u64, u64)> {
        let table = self.table_name();
        let bd_min =
            sqlx::types::BigDecimal::from_str(&format!("{:.8}", active_min)).unwrap_or_default();
        let bd_max =
            sqlx::types::BigDecimal::from_str(&format!("{:.8}", active_max)).unwrap_or_default();

        // Set active for instruments in range that are inactive
        let activate_query = format!(
            "UPDATE {} SET status = 'active' \
             WHERE underlying_symbol = $1 \
             AND status = 'inactive' \
             AND strike_value >= $2 \
             AND strike_value <= $3 \
             AND expiry > NOW()",
            table
        );
        let activated = sqlx::query(&activate_query)
            .bind(underlying)
            .bind(&bd_min)
            .bind(&bd_max)
            .execute(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to activate: {}", e)))?
            .rows_affected();

        // Set inactive for instruments out of range that are active
        let deactivate_query = format!(
            "UPDATE {} SET status = 'inactive' \
             WHERE underlying_symbol = $1 \
             AND status = 'active' \
             AND (strike_value < $2 OR strike_value > $3)",
            table
        );
        let deactivated = sqlx::query(&deactivate_query)
            .bind(underlying)
            .bind(&bd_min)
            .bind(&bd_max)
            .execute(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to deactivate: {}", e)))?
            .rows_affected();

        if activated > 0 || deactivated > 0 {
            debug!(
                "{}: activated {} / deactivated {} instruments for {}",
                table, activated, deactivated, underlying
            );
        }

        Ok((activated, deactivated))
    }
}

#[async_trait]
impl InstrumentStore for PostgresInstrumentStore {
    #[instrument(skip(self))]
    async fn get(&self, id: &InstrumentId) -> InstrumentResult<Option<OptionInstrument>> {
        let table = self.table_name();
        let uuid = Uuid::parse_str(id.as_str())
            .map_err(|e| InstrumentError::Internal(format!("Invalid UUID: {}", e)))?;

        let query = format!("SELECT * FROM {} WHERE id = $1", table);
        let row = sqlx::query_as::<_, InstrumentRow>(&query)
            .bind(uuid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to get instrument: {}", e)))?;

        Ok(row.map(|r| r.to_domain()))
    }

    #[instrument(skip(self))]
    async fn get_by_symbol(&self, symbol: &str) -> InstrumentResult<Option<OptionInstrument>> {
        let table = self.table_name();
        let query = format!("SELECT * FROM {} WHERE symbol = $1", table);
        let row = sqlx::query_as::<_, InstrumentRow>(&query)
            .bind(symbol)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to get instrument by symbol: {}", e))
            })?;

        Ok(row.map(|r| r.to_domain()))
    }

    #[instrument(skip(self))]
    async fn list(&self, query: &InstrumentQuery) -> InstrumentResult<Vec<OptionInstrument>> {
        let table = self.table_name();
        let mut sql = format!("SELECT * FROM {} WHERE 1=1", table);
        let mut params: Vec<String> = Vec::new();
        let mut param_idx = 1u32;

        if let Some(ref underlying) = query.underlying {
            sql.push_str(&format!(" AND underlying_symbol = ${}", param_idx));
            params.push(underlying.clone());
            param_idx += 1;
        }

        if let Some(option_type) = query.option_type {
            sql.push_str(&format!(" AND option_type = ${}", param_idx));
            params.push(option_type.as_db_str().to_string());
            param_idx += 1;
        }

        if let Some(status) = query.status {
            sql.push_str(&format!(" AND status = ${}", param_idx));
            params.push(status.as_db_str().to_string());
            param_idx += 1;
        }

        // Build the query with dynamic params using raw SQL approach
        // For simplicity, we reconstruct with inline safe params for dates/decimals
        sql.push_str(" ORDER BY expiry ASC, strike_value ASC, option_type ASC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        // Build the query dynamically
        let rows = self
            .execute_list_query(&sql, query)
            .await?;

        Ok(rows.into_iter().map(|r| r.to_domain()).collect())
    }

    #[instrument(skip(self))]
    async fn count(&self, query: &InstrumentQuery) -> InstrumentResult<usize> {
        let table = self.table_name();
        let mut sql = format!("SELECT COUNT(*) as count FROM {} WHERE 1=1", table);

        if let Some(ref underlying) = query.underlying {
            sql.push_str(&format!(" AND underlying_symbol = '{}'", underlying.replace('\'', "''")));
        }
        if let Some(option_type) = query.option_type {
            sql.push_str(&format!(" AND option_type = '{}'", option_type.as_db_str()));
        }
        if let Some(status) = query.status {
            sql.push_str(&format!(" AND status = '{}'", status.as_db_str()));
        }
        if let Some(expiry_after) = query.expiry_after {
            sql.push_str(&format!(" AND expiry > '{}'", expiry_after.to_rfc3339()));
        }
        if let Some(expiry_before) = query.expiry_before {
            sql.push_str(&format!(" AND expiry < '{}'", expiry_before.to_rfc3339()));
        }
        if let Some(strike_min) = query.strike_min {
            sql.push_str(&format!(" AND strike_value >= {}", strike_min));
        }
        if let Some(strike_max) = query.strike_max {
            sql.push_str(&format!(" AND strike_value <= {}", strike_max));
        }

        let row = sqlx::query(&sql)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| InstrumentError::StorageError(format!("Failed to count: {}", e)))?;

        let count: i64 = row.get("count");
        Ok(count as usize)
    }

    #[instrument(skip(self, instrument))]
    async fn save(&self, instrument: OptionInstrument) -> InstrumentResult<()> {
        let table = self.table_name();
        let row = InstrumentRow::from_domain(&instrument);

        let bd = |v: f64, prec: usize| -> sqlx::types::BigDecimal {
            sqlx::types::BigDecimal::from_str(&format!("{:.prec$}", v, prec = prec))
                .unwrap_or_default()
        };

        let query = format!(
            r#"
            INSERT INTO {} (
                id, symbol, underlying_symbol, underlying_name, underlying_decimals,
                option_type, exercise_style, strike_value, strike_decimals,
                expiry, settlement_currency, contract_size, tick_size, min_order_size,
                status, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
            table
        );

        sqlx::query(&query)
            .bind(row.id)
            .bind(&row.symbol)
            .bind(&row.underlying_symbol)
            .bind(&row.underlying_name)
            .bind(row.underlying_decimals)
            .bind(&row.option_type)
            .bind(&row.exercise_style)
            .bind(&row.strike_value)
            .bind(row.strike_decimals)
            .bind(row.expiry)
            .bind(&row.settlement_currency)
            .bind(&row.contract_size)
            .bind(&row.tick_size)
            .bind(row.min_order_size)
            .bind(&row.status)
            .bind(row.created_at)
            .bind(row.updated_at)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if e.to_string().contains("duplicate key") || e.to_string().contains("unique") {
                    InstrumentError::AlreadyExists(instrument.symbol.clone())
                } else {
                    InstrumentError::StorageError(format!("Failed to save instrument: {}", e))
                }
            })?;

        debug!("Saved instrument: {}", instrument.symbol);
        Ok(())
    }

    #[instrument(skip(self, instruments))]
    async fn save_batch(&self, instruments: Vec<OptionInstrument>) -> InstrumentResult<()> {
        if instruments.is_empty() {
            return Ok(());
        }

        let table = self.table_name();
        let total = instruments.len();

        // Process in chunks for better performance
        let chunk_size = 500;
        let mut saved = 0u64;
        let mut skipped = 0u64;

        for chunk in instruments.chunks(chunk_size) {
            // Build a multi-row INSERT with ON CONFLICT DO NOTHING
            let mut query = format!(
                r#"INSERT INTO {} (
                    id, symbol, underlying_symbol, underlying_name, underlying_decimals,
                    option_type, exercise_style, strike_value, strike_decimals,
                    expiry, settlement_currency, contract_size, tick_size, min_order_size,
                    status, created_at, updated_at
                ) VALUES "#,
                table
            );

            let mut param_idx = 1u32;
            let mut first = true;

            for _ in chunk {
                if !first {
                    query.push_str(", ");
                }
                query.push_str(&format!(
                    "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                    param_idx, param_idx + 1, param_idx + 2, param_idx + 3, param_idx + 4,
                    param_idx + 5, param_idx + 6, param_idx + 7, param_idx + 8, param_idx + 9,
                    param_idx + 10, param_idx + 11, param_idx + 12, param_idx + 13,
                    param_idx + 14, param_idx + 15, param_idx + 16
                ));
                param_idx += 17;
                first = false;
            }

            query.push_str(" ON CONFLICT (symbol) DO NOTHING");

            let mut q = sqlx::query(&query);

            for instrument in chunk {
                let row = InstrumentRow::from_domain(instrument);
                q = q
                    .bind(row.id)
                    .bind(row.symbol)
                    .bind(row.underlying_symbol)
                    .bind(row.underlying_name)
                    .bind(row.underlying_decimals)
                    .bind(row.option_type)
                    .bind(row.exercise_style)
                    .bind(row.strike_value)
                    .bind(row.strike_decimals)
                    .bind(row.expiry)
                    .bind(row.settlement_currency)
                    .bind(row.contract_size)
                    .bind(row.tick_size)
                    .bind(row.min_order_size)
                    .bind(row.status)
                    .bind(row.created_at)
                    .bind(row.updated_at);
            }

            let result = q.execute(&self.pool).await.map_err(|e| {
                InstrumentError::StorageError(format!("Failed to save batch: {}", e))
            })?;

            saved += result.rows_affected();
            skipped += chunk.len() as u64 - result.rows_affected();
        }

        info!(
            "Batch insert to {}: {} saved, {} skipped (duplicates), {} total",
            table, saved, skipped, total
        );
        Ok(())
    }

    #[instrument(skip(self, instrument))]
    async fn update(&self, instrument: OptionInstrument) -> InstrumentResult<()> {
        let table = self.table_name();
        let row = InstrumentRow::from_domain(&instrument);

        let query = format!(
            r#"
            UPDATE {} SET
                symbol = $2,
                underlying_symbol = $3,
                underlying_name = $4,
                underlying_decimals = $5,
                option_type = $6,
                exercise_style = $7,
                strike_value = $8,
                strike_decimals = $9,
                expiry = $10,
                settlement_currency = $11,
                contract_size = $12,
                tick_size = $13,
                min_order_size = $14,
                status = $15
            WHERE id = $1
            "#,
            table
        );

        let result = sqlx::query(&query)
            .bind(row.id)
            .bind(&row.symbol)
            .bind(&row.underlying_symbol)
            .bind(&row.underlying_name)
            .bind(row.underlying_decimals)
            .bind(&row.option_type)
            .bind(&row.exercise_style)
            .bind(&row.strike_value)
            .bind(row.strike_decimals)
            .bind(row.expiry)
            .bind(&row.settlement_currency)
            .bind(&row.contract_size)
            .bind(&row.tick_size)
            .bind(row.min_order_size)
            .bind(&row.status)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to update instrument: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(InstrumentError::NotFound(instrument.id.to_string()));
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn update_status(
        &self,
        id: &InstrumentId,
        status: InstrumentStatus,
    ) -> InstrumentResult<()> {
        let table = self.table_name();
        let uuid = Uuid::parse_str(id.as_str())
            .map_err(|e| InstrumentError::Internal(format!("Invalid UUID: {}", e)))?;

        let query = format!("UPDATE {} SET status = $1 WHERE id = $2", table);

        let result = sqlx::query(&query)
            .bind(status.as_db_str())
            .bind(uuid)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to update status: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(InstrumentError::NotFound(id.to_string()));
        }

        debug!("Updated instrument {} status to {}", id, status);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete(&self, id: &InstrumentId) -> InstrumentResult<()> {
        let table = self.table_name();
        let uuid = Uuid::parse_str(id.as_str())
            .map_err(|e| InstrumentError::Internal(format!("Invalid UUID: {}", e)))?;

        let query = format!("DELETE FROM {} WHERE id = $1", table);

        let result = sqlx::query(&query)
            .bind(uuid)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to delete instrument: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(InstrumentError::NotFound(id.to_string()));
        }

        Ok(())
    }
}

// =========================================================================
// Private helper methods
// =========================================================================

impl PostgresInstrumentStore {
    /// Execute a dynamic list query with filters.
    async fn execute_list_query(
        &self,
        _base_sql: &str,
        query: &InstrumentQuery,
    ) -> InstrumentResult<Vec<InstrumentRow>> {
        let table = self.table_name();

        // Build query with safe parameter binding
        // We use a simpler approach: construct the WHERE clause with sanitized values
        let mut sql = format!("SELECT * FROM {} WHERE 1=1", table);

        if let Some(ref underlying) = query.underlying {
            sql.push_str(&format!(
                " AND underlying_symbol = '{}'",
                underlying.replace('\'', "''")
            ));
        }
        if let Some(option_type) = query.option_type {
            sql.push_str(&format!(" AND option_type = '{}'", option_type.as_db_str()));
        }
        if let Some(status) = query.status {
            sql.push_str(&format!(" AND status = '{}'", status.as_db_str()));
        }
        if let Some(expiry_after) = query.expiry_after {
            sql.push_str(&format!(" AND expiry > '{}'", expiry_after.to_rfc3339()));
        }
        if let Some(expiry_before) = query.expiry_before {
            sql.push_str(&format!(" AND expiry < '{}'", expiry_before.to_rfc3339()));
        }
        if let Some(strike_min) = query.strike_min {
            sql.push_str(&format!(" AND strike_value >= {}", strike_min));
        }
        if let Some(strike_max) = query.strike_max {
            sql.push_str(&format!(" AND strike_value <= {}", strike_max));
        }

        sql.push_str(" ORDER BY expiry ASC, strike_value ASC, option_type ASC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let rows = sqlx::query_as::<_, InstrumentRow>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                InstrumentError::StorageError(format!("Failed to list instruments: {}", e))
            })?;

        Ok(rows)
    }
}
