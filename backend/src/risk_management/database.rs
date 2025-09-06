use crate::risk_management::types::{
    TradeEvent, UserPositions, RiskMetrics, RiskAlert, UserId, TokenAddress, TradeId
};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::{PgPool, Row, FromRow};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

/// Database configuration for TimescaleDB
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub connection_url: String,
    pub max_connections: u32,
    pub connection_timeout_ms: u64,
    pub query_timeout_ms: u64,
    pub enable_ssl: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            connection_url: "postgresql://user:password@localhost:5432/hyperdex".to_string(),
            max_connections: 20,
            connection_timeout_ms: 5000,
            query_timeout_ms: 30000,
            enable_ssl: false,
        }
    }
}

/// TimescaleDB database manager for risk management
pub struct RiskDatabase {
    pool: PgPool,
    config: DatabaseConfig,
}

/// Database representation of trade events
#[derive(Debug, Clone, FromRow)]
pub struct TradeEventRow {
    pub event_id: String,
    pub user_id: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub dex: String,
    pub gas_used: i64,
    pub gas_price: Decimal,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database representation of user positions
#[derive(Debug, Clone, FromRow)]
pub struct UserPositionRow {
    pub user_id: String,
    pub token_address: String,
    pub amount: Decimal,
    pub avg_cost: Decimal,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
    pub pnl: Option<Decimal>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Database representation of risk metrics
#[derive(Debug, Clone, FromRow)]
pub struct RiskMetricsRow {
    pub user_id: String,
    pub total_exposure_usd: Decimal,
    pub concentration_risk: Decimal,
    pub var_95: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: Decimal,
    pub win_rate: Decimal,
    pub avg_trade_size: Decimal,
    pub calculated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl RiskDatabase {
    /// Create a new database connection from URL
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let config = DatabaseConfig {
            connection_url: database_url.to_string(),
            ..Default::default()
        };
        Self::new_with_config(config).await
    }

    /// Create a new database connection with config
    pub async fn new_with_config(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_millis(config.connection_timeout_ms))
            .connect(&config.connection_url)
            .await?;

        Ok(Self { pool, config })
    }

    /// Initialize database schema with TimescaleDB hypertables
    pub async fn initialize_schema(&self) -> Result<(), sqlx::Error> {
        // Create trade_events hypertable - match the schema from init.sql
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS trade_events (
                event_id VARCHAR(255) PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                token_in VARCHAR(42) NOT NULL,
                token_out VARCHAR(42) NOT NULL,
                amount_in DECIMAL(78, 18) NOT NULL,
                amount_out DECIMAL(78, 18) NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                dex VARCHAR(50) NOT NULL,
                gas_used BIGINT NOT NULL,
                gas_price DECIMAL(78, 18) NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
        "#)
        .execute(&self.pool)
        .await?;

        // Convert to hypertable (TimescaleDB extension) - ignore if already exists
        let _ = sqlx::query("SELECT create_hypertable('trade_events', 'timestamp', if_not_exists => TRUE)")
            .execute(&self.pool)
            .await;

        // Create indexes for fast queries - ignore if they exist
        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_trade_events_user_time ON trade_events (user_id, timestamp DESC)")
            .execute(&self.pool)
            .await;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_trade_events_tokens ON trade_events (token_in, token_out)")
            .execute(&self.pool)
            .await;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_trade_events_dex ON trade_events (dex, timestamp DESC)")
            .execute(&self.pool)
            .await;

        // Create user_positions table - match schema from init.sql
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS user_positions (
                user_id VARCHAR(255) NOT NULL,
                token_address VARCHAR(42) NOT NULL,
                amount DECIMAL(78, 18) NOT NULL,
                avg_cost DECIMAL(78, 18) NOT NULL,
                last_updated TIMESTAMPTZ DEFAULT NOW(),
                pnl DECIMAL(78, 18) DEFAULT 0,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                PRIMARY KEY (user_id, token_address)
            )
        "#)
        .execute(&self.pool)
        .await?;

        // Create risk_metrics table - match schema from init.sql
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS risk_metrics (
                user_id VARCHAR(255) PRIMARY KEY,
                total_exposure_usd DECIMAL(78, 18) NOT NULL,
                concentration_risk DECIMAL(5, 4) NOT NULL,
                var_95 DECIMAL(78, 18) NOT NULL,
                max_drawdown DECIMAL(5, 4) NOT NULL,
                sharpe_ratio DECIMAL(10, 4) NOT NULL,
                win_rate DECIMAL(5, 4) NOT NULL,
                avg_trade_size DECIMAL(78, 18) NOT NULL,
                calculated_at TIMESTAMPTZ DEFAULT NOW()
            )
        "#)
        .execute(&self.pool)
        .await?;

        // Create risk_alerts table - match schema from init.sql
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS risk_alerts (
                alert_id VARCHAR(255) PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                rule_name VARCHAR(100) NOT NULL,
                severity VARCHAR(20) NOT NULL,
                message TEXT NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL,
                trade_id VARCHAR(255),
                resolved BOOLEAN DEFAULT FALSE,
                resolved_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ DEFAULT NOW()
            )
        "#)
        .execute(&self.pool)
        .await?;

        let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_risk_alerts_user_time ON risk_alerts (user_id, timestamp DESC)")
            .execute(&self.pool)
            .await;

        Ok(())
    }

    /// Store a trade event in the database
    pub async fn store_trade_event(&self, event: &TradeEvent) -> Result<(), sqlx::Error> {
        let start = Instant::now();
        
        let timestamp = chrono::DateTime::from_timestamp_millis(event.timestamp as i64)
            .unwrap_or_else(chrono::Utc::now);

        sqlx::query(r#"
            INSERT INTO trade_events (
                event_id, user_id, token_in, token_out, amount_in, amount_out,
                timestamp, dex, gas_used, gas_price
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#)
        .bind(&event.trade_id)
        .bind(&event.user_id)
        .bind(&event.token_in)
        .bind(&event.token_out)
        .bind(event.amount_in)
        .bind(event.amount_out)
        .bind(timestamp)
        .bind(&event.dex_source)  // Maps to 'dex' column
        .bind(event.gas_used)
        .bind(Decimal::from(20))
        .execute(&self.pool)
        .await?;

        // Target: <5ms for trade event storage
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(5) {
            log::warn!("Slow trade event storage: {:?}", elapsed);
        }

        Ok(())
    }

    /// Store multiple trade events in batch
    pub async fn store_trade_events_batch(&self, events: &[TradeEvent]) -> Result<(), sqlx::Error> {
        if events.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO trade_events (event_id, user_id, token_in, token_out, amount_in, amount_out, timestamp, dex, gas_used, gas_price) "
        );

        query_builder.push_values(events, |mut b, event| {
            let timestamp = chrono::DateTime::from_timestamp_millis(event.timestamp as i64)
                .unwrap_or_else(chrono::Utc::now);
            
            b.push_bind(&event.trade_id)  // event_id
                .push_bind(&event.user_id)  // user_id
                .push_bind(&event.token_in)  // token_in
                .push_bind(&event.token_out)  // token_out
                .push_bind(event.amount_in)  // amount_in
                .push_bind(event.amount_out)  // amount_out
                .push_bind(timestamp)  // timestamp
                .push_bind(&event.dex_source)  // dex
                .push_bind(event.gas_used)  // gas_used
                .push_bind(Decimal::from(20));  // gas_price (default value)
        });

        query_builder.build().execute(&self.pool).await?;

        // Target: <10ms for batch storage
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(10) {
            log::warn!("Slow batch trade event storage: {:?} for {} events", elapsed, events.len());
        }

        Ok(())
    }

    /// Update user position in database
    pub async fn update_user_position(&self, user_id: UserId, positions: &UserPositions) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Delete existing positions for user
        sqlx::query("DELETE FROM user_positions WHERE user_id = $1")
            .bind(user_id.to_string())
            .execute(&mut *tx)
            .await?;

        // Insert updated positions
        for (token_address, token_balance) in &positions.balances {
            sqlx::query(r#"
                INSERT INTO user_positions (user_id, token_address, amount, avg_cost, pnl)
                VALUES ($1, $2, $3, $4, $5)
            "#)
            .bind(user_id.to_string())
            .bind(token_address)
            .bind(token_balance.balance)
            .bind(token_balance.value_usd / token_balance.balance.max(Decimal::ONE))
            .bind(positions.pnl)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Get user positions from database (original method)
    pub async fn get_user_positions_original(&self, user_id: UserId) -> Result<Option<UserPositions>, sqlx::Error> {
        let rows: Vec<UserPositionRow> = sqlx::query_as(
            "SELECT * FROM user_positions WHERE user_id = $1"
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let mut balances = HashMap::new();
        let mut total_pnl = Decimal::ZERO;
        let mut last_updated = 0u64;

        for row in rows {
            let last_updated_ts = row.last_updated.unwrap_or_else(chrono::Utc::now).timestamp_millis() as u64;
            let pnl_value = row.pnl.unwrap_or_default();
            
            balances.insert(row.token_address.clone(), crate::risk_management::types::TokenBalance {
                token_address: row.token_address.clone(),
                balance: row.amount,
                value_usd: row.avg_cost * row.amount, // Approximate USD value
                last_updated: last_updated_ts,
            });
            total_pnl += pnl_value;
            last_updated = last_updated.max(last_updated_ts);
        }

        Ok(Some(UserPositions {
            balances,
            pnl: total_pnl,
            last_updated,
        }))
    }

    /// Store risk metrics in database
    pub async fn store_risk_metrics(&self, user_id: UserId, metrics: &RiskMetrics) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
            INSERT INTO risk_metrics (
                user_id, total_exposure_usd, concentration_risk, var_95,
                max_drawdown, sharpe_ratio, win_rate, avg_trade_size
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (user_id) DO UPDATE SET
                total_exposure_usd = EXCLUDED.total_exposure_usd,
                concentration_risk = EXCLUDED.concentration_risk,
                var_95 = EXCLUDED.var_95,
                max_drawdown = EXCLUDED.max_drawdown,
                sharpe_ratio = EXCLUDED.sharpe_ratio,
                win_rate = EXCLUDED.win_rate,
                avg_trade_size = EXCLUDED.avg_trade_size,
                calculated_at = NOW()
        "#)
        .bind(user_id.to_string())
        .bind(metrics.total_exposure_usd)
        .bind(metrics.concentration_risk)
        .bind(metrics.var_95)
        .bind(metrics.max_drawdown)
        .bind(metrics.sharpe_ratio)
        .bind(metrics.win_rate)
        .bind(metrics.avg_trade_size)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get risk metrics from database
    pub async fn get_risk_metrics(&self, user_id: UserId) -> Result<Option<RiskMetrics>, sqlx::Error> {
        let row: Option<RiskMetricsRow> = sqlx::query_as(
            "SELECT * FROM risk_metrics WHERE user_id = $1"
        )
        .bind(user_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| RiskMetrics {
            total_exposure_usd: r.total_exposure_usd,
            concentration_risk: r.concentration_risk,
            var_95: r.var_95,
            max_drawdown: r.max_drawdown,
            sharpe_ratio: r.sharpe_ratio,
            win_rate: r.win_rate,
            avg_trade_size: r.avg_trade_size,
        }))
    }

    /// Store risk alert in database
    pub async fn store_risk_alert(&self, alert: &RiskAlert) -> Result<(), sqlx::Error> {
        let timestamp = chrono::DateTime::from_timestamp_millis(alert.timestamp as i64)
            .unwrap_or_else(chrono::Utc::now);

        sqlx::query(r#"
            INSERT INTO risk_alerts (
                alert_id, user_id, rule_name, severity, message, timestamp, trade_id
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#)
        .bind(&alert.alert_id)
        .bind(alert.user_id)
        .bind(&alert.rule_name)
        .bind(format!("{:?}", alert.severity))
        .bind(&alert.message)
        .bind(timestamp)
        .bind(alert.trade_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get recent alerts for a user
    pub async fn get_user_alerts(&self, user_id: UserId, limit: i64) -> Result<Vec<RiskAlert>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM risk_alerts WHERE user_id = $1 ORDER BY timestamp DESC LIMIT $2"
        )
        .bind(user_id.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut alerts = Vec::new();
        for row in rows {
            let severity = match row.get::<String, _>("severity").as_str() {
                "Low" => crate::risk_management::types::AlertSeverity::Low,
                "Medium" => crate::risk_management::types::AlertSeverity::Medium,
                "High" => crate::risk_management::types::AlertSeverity::High,
                "Critical" => crate::risk_management::types::AlertSeverity::Critical,
                _ => crate::risk_management::types::AlertSeverity::Medium,
            };

            alerts.push(RiskAlert {
                user_id: row.get("user_id"),
                alert_id: row.get("alert_id"),
                rule_name: row.get("rule_name"),
                severity,
                message: row.get("message"),
                timestamp: row.get::<chrono::DateTime<chrono::Utc>, _>("timestamp").timestamp_millis() as u64,
                trade_id: row.get("trade_id"),
            });
        }

        Ok(alerts)
    }

    /// Get trade history for a user (simplified signature for tests)
    pub async fn get_user_trade_history(
        &self, 
        user_id: &str, 
        limit: i64
    ) -> Result<Vec<TradeEvent>, sqlx::Error> {
        self.get_user_trade_history_with_offset(user_id, limit, 0).await
    }

    /// Get trade history for a user with offset
    pub async fn get_user_trade_history_with_offset(
        &self, 
        user_id: &str, 
        limit: i64, 
        offset: i64
    ) -> Result<Vec<TradeEvent>, sqlx::Error> {
        let rows: Vec<TradeEventRow> = sqlx::query_as(
            "SELECT * FROM trade_events WHERE user_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let trades = rows.into_iter().map(|row| TradeEvent {
            user_id: uuid::Uuid::parse_str(&row.user_id).unwrap_or_default(),
            trade_id: uuid::Uuid::parse_str(&row.event_id).unwrap_or_default(),
            token_in: row.token_in,
            token_out: row.token_out,
            amount_in: row.amount_in,
            amount_out: row.amount_out,
            timestamp: row.timestamp.timestamp_millis() as u64,
            dex_source: row.dex,
            gas_used: Decimal::from(row.gas_used),
        }).collect();

        Ok(trades)
    }

    /// Health check for database connection
    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Store user positions (simplified signature for tests)
    pub async fn store_user_positions(&self, user_id: &str, positions: &UserPositions) -> Result<(), sqlx::Error> {
        let user_uuid = uuid::Uuid::parse_str(user_id)
            .map_err(|_| sqlx::Error::RowNotFound)?;
        self.update_user_position(user_uuid, positions).await
    }

    /// Get user positions (simplified signature for tests)
    pub async fn get_user_positions(&self, user_id: &str) -> Result<Option<UserPositions>, sqlx::Error> {
        let user_uuid = uuid::Uuid::parse_str(user_id)
            .map_err(|_| sqlx::Error::RowNotFound)?;
        self.get_user_positions_by_uuid(user_uuid).await
    }

    /// Get user positions by UUID
    pub async fn get_user_positions_by_uuid(&self, user_id: UserId) -> Result<Option<UserPositions>, sqlx::Error> {
        self.get_user_positions_original(user_id).await
    }

    /// Create continuous aggregates for fast analytics
    pub async fn create_continuous_aggregates(&self) -> Result<(), sqlx::Error> {
        // User daily P&L aggregate
        sqlx::query(r#"
            CREATE MATERIALIZED VIEW IF NOT EXISTS user_daily_pnl
            WITH (timescaledb.continuous) AS
            SELECT 
                time_bucket('1 day', time) AS day,
                user_id,
                SUM(pnl) as daily_pnl,
                COUNT(*) as trade_count,
                AVG(pnl) as avg_pnl,
                SUM(amount_in) as total_volume_in,
                SUM(amount_out) as total_volume_out
            FROM trade_events
            GROUP BY day, user_id
            WITH NO DATA
        "#)
        .execute(&self.pool)
        .await?;

        // Token volume aggregate
        sqlx::query(r#"
            CREATE MATERIALIZED VIEW IF NOT EXISTS token_hourly_volume
            WITH (timescaledb.continuous) AS
            SELECT 
                time_bucket('1 hour', time) AS hour,
                token_in as token,
                SUM(amount_in) as volume,
                COUNT(*) as trade_count,
                COUNT(DISTINCT user_id) as unique_users
            FROM trade_events
            GROUP BY hour, token
            WITH NO DATA
        "#)
        .execute(&self.pool)
        .await?;

        // Set up refresh policies for real-time updates
        sqlx::query(r#"
            SELECT add_continuous_aggregate_policy('user_daily_pnl',
                start_offset => INTERVAL '7 days',
                end_offset => INTERVAL '1 hour',
                schedule_interval => INTERVAL '1 hour')
        "#)
        .execute(&self.pool)
        .await?;

        sqlx::query(r#"
            SELECT add_continuous_aggregate_policy('token_hourly_volume',
                start_offset => INTERVAL '3 days',
                end_offset => INTERVAL '1 hour',
                schedule_interval => INTERVAL '30 minutes')
        "#)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get database connection pool for custom queries
    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    /// Health check for database connection (duplicate removed)
    pub async fn health_check_bool(&self) -> Result<bool, sqlx::Error> {
        let row: (i32,) = sqlx::query_as("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(row.0 == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::types::*;
    use std::collections::HashMap;

    // Note: These tests require a running TimescaleDB instance
    // Run with: docker run -d --name timescaledb -p 5432:5432 -e POSTGRES_PASSWORD=password timescale/timescaledb:latest-pg14

    async fn create_test_database() -> RiskDatabase {
        let config = DatabaseConfig {
            connection_url: "postgresql://postgres:password@localhost:5432/postgres".to_string(),
            ..Default::default()
        };
        
        RiskDatabase::new(&config.connection_url).await.expect("Failed to create test database")
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB running
    async fn test_database_initialization() {
        let db = create_test_database().await;
        let result = db.initialize_schema().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB running
    async fn test_trade_event_storage() {
        let db = create_test_database().await;
        db.initialize_schema().await.unwrap();

        let trade_event = TradeEvent {
            user_id: Uuid::new_v4(),
            trade_id: Uuid::new_v4(),
            token_in: "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
            token_out: "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
            amount_in: Decimal::from(1000),
            amount_out: Decimal::from(1900),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            dex_source: "uniswap_v3".to_string(),
            gas_used: Decimal::from(21000),
        };

        let result = db.store_trade_event(&trade_event).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB running
    async fn test_batch_trade_event_storage() {
        let db = create_test_database().await;
        db.initialize_schema().await.unwrap();

        let user_id = Uuid::new_v4();
        let events = vec![
            TradeEvent {
                user_id,
                trade_id: Uuid::new_v4(),
                token_in: "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
                token_out: "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
                amount_in: Decimal::from(1000),
                amount_out: Decimal::from(1900),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                dex_source: "uniswap_v3".to_string(),
                gas_used: Decimal::from(21000),
            },
            TradeEvent {
                user_id,
                trade_id: Uuid::new_v4(),
                token_in: "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
                token_out: "0xC0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
                amount_in: Decimal::from(500),
                amount_out: Decimal::from(950),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                dex_source: "sushiswap".to_string(),
                gas_used: Decimal::from(25000),
            },
        ];

        let result = db.store_trade_events_batch(&events).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB running
    async fn test_user_positions_storage_and_retrieval() {
        let db = create_test_database().await;
        db.initialize_schema().await.unwrap();

        let user_id = Uuid::new_v4();
        let mut balances = HashMap::new();
        balances.insert(
            "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
            TokenBalance {
                token_address: "0x1234".to_string(),
                balance: Decimal::from(1000),
                value_usd: Decimal::from(1900000),
                last_updated: 1234567890,
            }
        );

        let positions = UserPositions {
            balances,
            pnl: Decimal::from(100),
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        };

        // Store positions
        let result = db.update_user_position(user_id, &positions).await;
        assert!(result.is_ok());

        // Retrieve positions
        let retrieved = db.get_user_positions(&user_id.to_string()).await.unwrap();
        assert!(retrieved.is_some());
        
        let retrieved_positions = retrieved.unwrap();
        assert_eq!(retrieved_positions.pnl, Decimal::from(100));
        assert_eq!(retrieved_positions.balances.len(), 1);
    }

    #[tokio::test]
    #[ignore] // Requires TimescaleDB running
    async fn test_health_check() {
        let db = create_test_database().await;
        let result = db.health_check().await;
        assert!(result.is_ok());
        result.unwrap();
    }
}
