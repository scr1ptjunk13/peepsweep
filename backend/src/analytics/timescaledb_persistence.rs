use crate::analytics::live_pnl_engine::{PnLSnapshot, PositionPnL};
use crate::analytics::pnl_persistence::*;
use crate::risk_management::types::RiskError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// TimescaleDB implementation for P&L persistence
#[derive(Debug, Clone)]
pub struct TimescaleDBPersistence {
    pool: PgPool,
    config: TimescaleDBConfig,
    compression_stats: Arc<RwLock<CompressionStats>>,
}

/// TimescaleDB configuration
#[derive(Debug, Clone)]
pub struct TimescaleDBConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
    pub statement_timeout_seconds: u64,
    pub enable_compression: bool,
    pub compression_interval_days: u32,
    pub chunk_time_interval_hours: u32,
    pub retention_policy_days: u32,
}

impl Default for TimescaleDBConfig {
    fn default() -> Self {
        Self {
            database_url: "postgresql://localhost:5432/hyperdex_analytics".to_string(),
            max_connections: 20,
            connection_timeout_seconds: 30,
            statement_timeout_seconds: 60,
            enable_compression: true,
            compression_interval_days: 7,
            chunk_time_interval_hours: 24,
            retention_policy_days: 365,
        }
    }
}

/// Compression statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompressionStats {
    pub total_compressed_chunks: u64,
    pub compression_ratio: f64,
    pub space_saved_bytes: u64,
    pub last_compression_time: Option<DateTime<Utc>>,
}

impl TimescaleDBPersistence {
    /// Create new TimescaleDB persistence layer
    pub async fn new(config: TimescaleDBConfig) -> Result<Self, RiskError> {
        let pool = PgPool::connect(&config.database_url)
            .await
            .map_err(|e| RiskError::DatabaseError(format!("Failed to connect to TimescaleDB: {}", e)))?;

        let persistence = Self {
            pool,
            config,
            compression_stats: Arc::new(RwLock::new(CompressionStats::default())),
        };

        // Initialize database schema
        persistence.initialize_schema().await?;

        info!("TimescaleDB persistence initialized successfully");
        Ok(persistence)
    }

    /// Initialize TimescaleDB schema with hypertables and compression
    async fn initialize_schema(&self) -> Result<(), RiskError> {
        // Create P&L snapshots hypertable
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS pnl_snapshots (
                id BIGSERIAL,
                user_id UUID NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                total_unrealized_pnl_usd DECIMAL(20,8) NOT NULL,
                total_realized_pnl_usd DECIMAL(20,8) NOT NULL,
                total_pnl_usd DECIMAL(20,8) NOT NULL,
                total_unrealized_pnl_eth DECIMAL(20,8) NOT NULL,
                total_realized_pnl_eth DECIMAL(20,8) NOT NULL,
                total_pnl_eth DECIMAL(20,8) NOT NULL,
                total_unrealized_pnl_btc DECIMAL(20,8) NOT NULL,
                total_realized_pnl_btc DECIMAL(20,8) NOT NULL,
                total_pnl_btc DECIMAL(20,8) NOT NULL,
                total_portfolio_value_usd DECIMAL(20,8) NOT NULL,
                daily_change_usd DECIMAL(20,8) NOT NULL,
                daily_change_percent DECIMAL(10,4) NOT NULL,
                calculation_duration_ms BIGINT NOT NULL,
                PRIMARY KEY (timestamp, user_id)
            );
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to create pnl_snapshots table: {}", e)))?;

        // Create hypertable
        sqlx::query(r#"
            SELECT create_hypertable('pnl_snapshots', 'timestamp', 
                chunk_time_interval => INTERVAL '24 hours',
                if_not_exists => TRUE);
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to create pnl_snapshots hypertable: {}", e)))?;

        // Create position P&L history hypertable
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS position_pnl_history (
                id BIGSERIAL,
                user_id UUID NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                token_address VARCHAR(42) NOT NULL,
                chain_id BIGINT NOT NULL,
                symbol VARCHAR(20) NOT NULL,
                balance DECIMAL(30,18) NOT NULL,
                entry_price_usd DECIMAL(20,8) NOT NULL,
                current_price_usd DECIMAL(20,8) NOT NULL,
                unrealized_pnl_usd DECIMAL(20,8) NOT NULL,
                realized_pnl_usd DECIMAL(20,8) NOT NULL,
                total_pnl_usd DECIMAL(20,8) NOT NULL,
                position_value_usd DECIMAL(20,8) NOT NULL,
                price_change_24h_percent DECIMAL(10,4) NOT NULL,
                PRIMARY KEY (timestamp, user_id, token_address, chain_id)
            );
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to create position_pnl_history table: {}", e)))?;

        // Create hypertable for position history
        sqlx::query(r#"
            SELECT create_hypertable('position_pnl_history', 'timestamp',
                chunk_time_interval => INTERVAL '24 hours',
                if_not_exists => TRUE);
        "#)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to create position_pnl_history hypertable: {}", e)))?;

        // Create indexes for performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_pnl_snapshots_user_time ON pnl_snapshots (user_id, timestamp DESC);")
            .execute(&self.pool)
            .await
            .map_err(|e| RiskError::DatabaseError(format!("Failed to create pnl_snapshots index: {}", e)))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_position_pnl_user_token ON position_pnl_history (user_id, token_address, timestamp DESC);")
            .execute(&self.pool)
            .await
            .map_err(|e| RiskError::DatabaseError(format!("Failed to create position_pnl_history index: {}", e)))?;

        // Setup compression if enabled
        if self.config.enable_compression {
            self.setup_compression().await?;
        }

        // Setup retention policy
        self.setup_retention_policy().await?;

        info!("TimescaleDB schema initialized with hypertables and compression");
        Ok(())
    }

    /// Setup compression policies
    async fn setup_compression(&self) -> Result<(), RiskError> {
        let compression_interval = format!("INTERVAL '{} days'", self.config.compression_interval_days);

        // Add compression policy for P&L snapshots
        sqlx::query(&format!(r#"
            SELECT add_compression_policy('pnl_snapshots', {}, if_not_exists => TRUE);
        "#, compression_interval))
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to add compression policy for pnl_snapshots: {}", e)))?;

        // Add compression policy for position history
        sqlx::query(&format!(r#"
            SELECT add_compression_policy('position_pnl_history', {}, if_not_exists => TRUE);
        "#, compression_interval))
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to add compression policy for position_pnl_history: {}", e)))?;

        info!("TimescaleDB compression policies configured");
        Ok(())
    }

    /// Setup data retention policies
    async fn setup_retention_policy(&self) -> Result<(), RiskError> {
        let retention_interval = format!("INTERVAL '{} days'", self.config.retention_policy_days);

        // Add retention policy for P&L snapshots
        sqlx::query(&format!(r#"
            SELECT add_retention_policy('pnl_snapshots', {}, if_not_exists => TRUE);
        "#, retention_interval))
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to add retention policy for pnl_snapshots: {}", e)))?;

        // Add retention policy for position history
        sqlx::query(&format!(r#"
            SELECT add_retention_policy('position_pnl_history', {}, if_not_exists => TRUE);
        "#, retention_interval))
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to add retention policy for position_pnl_history: {}", e)))?;

        info!("TimescaleDB retention policies configured for {} days", self.config.retention_policy_days);
        Ok(())
    }

    /// Get compression statistics
    pub async fn get_compression_stats(&self) -> Result<CompressionStats, RiskError> {
        let row = sqlx::query(r#"
            SELECT 
                COUNT(*) as compressed_chunks,
                AVG(compression_ratio) as avg_compression_ratio,
                SUM(uncompressed_bytes - compressed_bytes) as space_saved
            FROM timescaledb_information.compressed_chunk_stats
            WHERE hypertable_name IN ('pnl_snapshots', 'position_pnl_history');
        "#)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to get compression stats: {}", e)))?;

        let compressed_chunks: i64 = row.get("compressed_chunks");
        let avg_compression_ratio: Option<f64> = row.get("avg_compression_ratio");
        let space_saved: Option<i64> = row.get("space_saved");

        let mut stats = self.compression_stats.write().await;
        stats.total_compressed_chunks = compressed_chunks as u64;
        stats.compression_ratio = avg_compression_ratio.unwrap_or(0.0);
        stats.space_saved_bytes = space_saved.unwrap_or(0) as u64;
        stats.last_compression_time = Some(Utc::now());

        Ok(stats.clone())
    }
}

#[async_trait::async_trait]
impl crate::analytics::pnl_websocket::PersistenceInterface for TimescaleDBPersistence {
    async fn get_latest_pnl_snapshot(&self, user_id: uuid::Uuid) -> Result<Option<crate::analytics::live_pnl_engine::PnLSnapshot>, crate::risk_management::types::RiskError> {
        let end_time = chrono::Utc::now();
        let start_time = end_time - chrono::Duration::hours(1);
        let snapshots = self.get_pnl_snapshots(user_id, start_time, end_time).await?;
        Ok(snapshots.into_iter().last())
    }
    
    async fn get_pnl_history(&self, query: &crate::analytics::pnl_websocket::PnLHistoryQuery) -> Result<Vec<crate::analytics::live_pnl_engine::PnLSnapshot>, crate::risk_management::types::RiskError> {
        self.get_pnl_snapshots(query.user_id, query.start_time, query.end_time).await
    }
    
    async fn get_aggregated_pnl_data(&self, query: &crate::analytics::pnl_websocket::AggregatedPnLQuery) -> Result<Vec<crate::analytics::pnl_persistence::AggregatedPnLData>, crate::risk_management::types::RiskError> {
        // Stub implementation for compilation
        Ok(vec![])
    }
    
    async fn get_position_history(&self, user_id: uuid::Uuid, token_address: &str, chain_id: u64, start_time: chrono::DateTime<chrono::Utc>, end_time: chrono::DateTime<chrono::Utc>) -> Result<Vec<crate::analytics::data_models::PositionPnL>, crate::risk_management::types::RiskError> {
        // Stub implementation for compilation
        Ok(vec![])
    }
    
    async fn get_persistence_stats(&self) -> Result<crate::analytics::pnl_persistence::PersistenceStats, crate::risk_management::types::RiskError> {
        // Return basic stats
        Ok(crate::analytics::pnl_persistence::PersistenceStats {
            total_snapshots_stored: 0,
            total_position_history_stored: 0,
            successful_inserts: 0,
            failed_inserts: 0,
            cache_hits: 0,
            cache_misses: 0,
            cleanup_operations: 0,
            records_cleaned: 0,
            average_insert_time_ms: 0.0,
            last_cleanup_time: None,
        })
    }
}

#[async_trait::async_trait]
impl PostgresInterface for TimescaleDBPersistence {
    /// Insert P&L snapshot with multi-currency support
    async fn insert_pnl_snapshot(&self, snapshot: &PnLSnapshot) -> Result<(), RiskError> {
        // Convert to multi-currency P&L data
        let multi_currency_pnl = self.convert_to_multi_currency_pnl(snapshot).await?;

        sqlx::query(r#"
            INSERT INTO pnl_snapshots (
                user_id, timestamp, total_unrealized_pnl_usd, total_realized_pnl_usd, total_pnl_usd,
                total_unrealized_pnl_eth, total_realized_pnl_eth, total_pnl_eth,
                total_unrealized_pnl_btc, total_realized_pnl_btc, total_pnl_btc,
                total_portfolio_value_usd, daily_change_usd, daily_change_percent, calculation_duration_ms
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#)
        .bind(snapshot.user_id)
        .bind(snapshot.timestamp)
        .bind(snapshot.total_unrealized_pnl_usd)
        .bind(snapshot.total_realized_pnl_usd)
        .bind(snapshot.total_pnl_usd)
        .bind(multi_currency_pnl.unrealized_pnl_eth)
        .bind(multi_currency_pnl.realized_pnl_eth)
        .bind(multi_currency_pnl.total_pnl_eth)
        .bind(multi_currency_pnl.unrealized_pnl_btc)
        .bind(multi_currency_pnl.realized_pnl_btc)
        .bind(multi_currency_pnl.total_pnl_btc)
        .bind(snapshot.total_portfolio_value_usd)
        .bind(snapshot.daily_change_usd)
        .bind(snapshot.daily_change_percent)
        .bind(snapshot.calculation_duration_ms as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to insert P&L snapshot: {}", e)))?;

        debug!("Inserted P&L snapshot for user {} with multi-currency data", snapshot.user_id);
        Ok(())
    }

    /// Get P&L snapshots with multi-currency data
    async fn get_pnl_snapshots(&self, user_id: Uuid, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Vec<PnLSnapshot>, RiskError> {
        let rows = sqlx::query(r#"
            SELECT user_id, timestamp, total_unrealized_pnl_usd, total_realized_pnl_usd, total_pnl_usd,
                   total_unrealized_pnl_eth, total_realized_pnl_eth, total_pnl_eth,
                   total_unrealized_pnl_btc, total_realized_pnl_btc, total_pnl_btc,
                   total_portfolio_value_usd, daily_change_usd, daily_change_percent, calculation_duration_ms
            FROM pnl_snapshots 
            WHERE user_id = $1 AND timestamp >= $2 AND timestamp <= $3
            ORDER BY timestamp DESC
        "#)
        .bind(user_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to get P&L snapshots: {}", e)))?;

        let mut snapshots = Vec::new();
        for row in rows {
            let snapshot = crate::analytics::live_pnl_engine::PnLSnapshot {
                user_id: row.get("user_id"),
                timestamp: row.get("timestamp"),
                positions: Vec::new(), // Will be populated separately if needed
                total_unrealized_pnl_usd: row.get("total_unrealized_pnl_usd"),
                total_realized_pnl_usd: row.get("total_realized_pnl_usd"),
                total_pnl_usd: row.get("total_pnl_usd"),
                total_portfolio_value_usd: row.get("total_portfolio_value_usd"),
                daily_change_usd: row.get("daily_change_usd"),
                daily_change_percent: row.get("daily_change_percent"),
                calculation_duration_ms: row.get::<i64, _>("calculation_duration_ms") as u64,
            };
            snapshots.push(snapshot);
        }

        Ok(snapshots)
    }

    /// Get latest P&L snapshot
    async fn get_latest_pnl_snapshot(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError> {
        let row = sqlx::query(r#"
            SELECT user_id, timestamp, total_unrealized_pnl_usd, total_realized_pnl_usd, total_pnl_usd,
                   total_portfolio_value_usd, daily_change_usd, daily_change_percent, calculation_duration_ms
            FROM pnl_snapshots 
            WHERE user_id = $1 
            ORDER BY timestamp DESC 
            LIMIT 1
        "#)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to get latest P&L snapshot: {}", e)))?;

        if let Some(row) = row {
            let snapshot = crate::analytics::live_pnl_engine::PnLSnapshot {
                user_id: row.get("user_id"),
                timestamp: row.get("timestamp"),
                positions: Vec::new(),
                total_unrealized_pnl_usd: row.get("total_unrealized_pnl_usd"),
                total_realized_pnl_usd: row.get("total_realized_pnl_usd"),
                total_pnl_usd: row.get("total_pnl_usd"),
                total_portfolio_value_usd: row.get("total_portfolio_value_usd"),
                daily_change_usd: row.get("daily_change_usd"),
                daily_change_percent: row.get("daily_change_percent"),
                calculation_duration_ms: row.get::<i64, _>("calculation_duration_ms") as u64,
            };
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    /// Insert position P&L history
    async fn insert_position_pnl_history(&self, user_id: Uuid, position_pnl: &PositionPnL) -> Result<(), RiskError> {
        sqlx::query(r#"
            INSERT INTO position_pnl_history (
                user_id, timestamp, token_address, chain_id, symbol, balance,
                entry_price_usd, current_price_usd, unrealized_pnl_usd, realized_pnl_usd,
                total_pnl_usd, position_value_usd, price_change_24h_percent
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#)
        .bind(user_id)
        .bind(position_pnl.last_updated)
        .bind(&position_pnl.token_address)
        .bind(position_pnl.chain_id as i64)
        .bind(&position_pnl.symbol)
        .bind(position_pnl.balance)
        .bind(position_pnl.entry_price_usd)
        .bind(position_pnl.current_price_usd)
        .bind(position_pnl.unrealized_pnl_usd)
        .bind(position_pnl.realized_pnl_usd)
        .bind(position_pnl.total_pnl_usd)
        .bind(position_pnl.position_value_usd)
        .bind(position_pnl.price_change_24h_percent)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to insert position P&L history: {}", e)))?;

        Ok(())
    }

    /// Get position P&L history
    async fn get_position_pnl_history(&self, user_id: Uuid, token_address: &str, chain_id: u64, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Vec<PositionPnL>, RiskError> {
        let rows = sqlx::query(r#"
            SELECT timestamp, token_address, chain_id, symbol, balance,
                   entry_price_usd, current_price_usd, unrealized_pnl_usd, realized_pnl_usd,
                   total_pnl_usd, position_value_usd, price_change_24h_percent
            FROM position_pnl_history 
            WHERE user_id = $1 AND token_address = $2 AND chain_id = $3 
                  AND timestamp >= $4 AND timestamp <= $5
            ORDER BY timestamp DESC
        "#)
        .bind(user_id)
        .bind(token_address)
        .bind(chain_id as i64)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to get position P&L history: {}", e)))?;

        let mut position_history = Vec::new();
        for row in rows {
            let position_pnl = crate::analytics::live_pnl_engine::PositionPnL {
                token_address: row.get("token_address"),
                chain_id: row.get::<i64, _>("chain_id") as u64,
                symbol: row.get("symbol"),
                balance: row.get("balance"),
                entry_price_usd: row.get("entry_price_usd"),
                current_price_usd: row.get("current_price_usd"),
                unrealized_pnl_usd: row.get("unrealized_pnl_usd"),
                realized_pnl_usd: row.get("realized_pnl_usd"),
                total_pnl_usd: row.get("total_pnl_usd"),
                position_value_usd: row.get("position_value_usd"),
                price_change_24h_percent: row.get("price_change_24h_percent"),
                last_updated: row.get("timestamp"),
            };
            position_history.push(position_pnl);
        }

        Ok(position_history)
    }

    /// Cleanup old snapshots
    async fn cleanup_old_snapshots(&self, retention_days: u32) -> Result<u64, RiskError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);

        let result = sqlx::query(r#"
            DELETE FROM pnl_snapshots WHERE timestamp < $1
        "#)
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .map_err(|e| RiskError::DatabaseError(format!("Failed to cleanup old P&L snapshots: {}", e)))?;

        let deleted_count = result.rows_affected();

        info!("Cleaned up {} old P&L snapshots older than {} days", deleted_count, retention_days);
        Ok(deleted_count)
    }
}

impl TimescaleDBPersistence {
    /// Convert P&L snapshot to multi-currency format
    async fn convert_to_multi_currency_pnl(&self, snapshot: &PnLSnapshot) -> Result<MultiCurrencyPnL, RiskError> {
        // Get current ETH and BTC prices
        let eth_price = self.get_eth_price().await?;
        let btc_price = self.get_btc_price().await?;

        let unrealized_pnl_eth = if eth_price > Decimal::ZERO {
            snapshot.total_unrealized_pnl_usd / eth_price
        } else {
            Decimal::ZERO
        };

        let realized_pnl_eth = if eth_price > Decimal::ZERO {
            snapshot.total_realized_pnl_usd / eth_price
        } else {
            Decimal::ZERO
        };

        let unrealized_pnl_btc = if btc_price > Decimal::ZERO {
            snapshot.total_unrealized_pnl_usd / btc_price
        } else {
            Decimal::ZERO
        };

        let realized_pnl_btc = if btc_price > Decimal::ZERO {
            snapshot.total_realized_pnl_usd / btc_price
        } else {
            Decimal::ZERO
        };

        Ok(MultiCurrencyPnL {
            unrealized_pnl_eth,
            realized_pnl_eth,
            total_pnl_eth: unrealized_pnl_eth + realized_pnl_eth,
            unrealized_pnl_btc,
            realized_pnl_btc,
            total_pnl_btc: unrealized_pnl_btc + realized_pnl_btc,
        })
    }

    /// Get current ETH price in USD
    async fn get_eth_price(&self) -> Result<Decimal, RiskError> {
        // In production, this would fetch from a price oracle
        // For now, using a reasonable estimate
        Ok(Decimal::new(3200, 0)) // $3200
    }

    /// Get current BTC price in USD
    async fn get_btc_price(&self) -> Result<Decimal, RiskError> {
        // In production, this would fetch from a price oracle
        // For now, using a reasonable estimate
        Ok(Decimal::new(65000, 0)) // $65000
    }
}

/// Multi-currency P&L data
#[derive(Debug, Clone)]
pub struct MultiCurrencyPnL {
    pub unrealized_pnl_eth: Decimal,
    pub realized_pnl_eth: Decimal,
    pub total_pnl_eth: Decimal,
    pub unrealized_pnl_btc: Decimal,
    pub realized_pnl_btc: Decimal,
    pub total_pnl_btc: Decimal,
}
