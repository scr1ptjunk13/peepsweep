// src/database/mod.rs - Database connection management and utilities
use sqlx::{PgPool, postgres::{PgPoolOptions, PgConnectOptions}, ConnectOptions, Executor};
use std::time::Duration;
use tracing::{info, warn, error};
use crate::{DatabaseResult, DatabaseError};

pub mod models;
pub mod queries;

/// Database connection manager with optimized settings
pub struct Database;

impl Database {
    /// Initialize database connection pool with production-ready settings
    pub async fn connect(database_url: &str) -> DatabaseResult<PgPool> {
        info!("Initializing database connection pool...");
        
        // Simple connection without complex options
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;
        
        info!("✅ Database connection pool initialized");
        Ok(pool)
    }
    
    /// Run database migrations
    pub async fn migrate(pool: &PgPool) -> DatabaseResult<()> {
        info!("Running database migrations...");
        
        // Check if migrations table exists, create if not
        let migration_check = sqlx::query(
            "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_name = '_sqlx_migrations')"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;
        
        // Run migrations from embedded files
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;
        
        info!("✅ Database migrations completed successfully");
        Ok(())
    }
    
    /// Comprehensive health check with connection pool stats
    pub async fn health_check(pool: &PgPool) -> DatabaseResult<HealthStatus> {
        // Test basic connectivity
        let start_time = std::time::Instant::now();
        
        let version_result = sqlx::query_scalar::<_, String>("SELECT version()")
            .fetch_one(pool)
            .await
            .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;
        
        let response_time = start_time.elapsed();
        
        // Get connection pool statistics
        let pool_status = PoolStatus {
            size: pool.size() as u32,
            idle: pool.num_idle() as u32,
            used: (pool.size() as u32).saturating_sub(pool.num_idle() as u32),
            max_size: 50, // From our pool configuration
        };
        
        // Test materialized view exists
        let mv_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT FROM pg_matviews WHERE matviewname = 'user_positions_summary')"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        
        // Test partition tables exist
        let partitions_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pg_tables WHERE tablename LIKE 'positions_v%_%'"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        
        let health_status = HealthStatus {
            connected: true,
            version: version_result,
            response_time_ms: response_time.as_millis() as u64,
            pool_status,
            materialized_view_exists: mv_exists,
            partition_tables_count: partitions_count,
            last_check: chrono::Utc::now(),
        };
        
        if response_time.as_millis() > 100 {
            warn!("Database response time is high: {}ms", response_time.as_millis());
        }
        
        Ok(health_status)
    }
    
    /// Refresh materialized views for performance
    pub async fn refresh_materialized_views(pool: &PgPool) -> DatabaseResult<()> {
        info!("Refreshing materialized views...");
        
        let start_time = std::time::Instant::now();
        
        // Refresh user positions summary view
        sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY user_positions_summary")
            .execute(pool)
            .await
            .map_err(|e| DatabaseError::QueryFailed(format!("Failed to refresh materialized view: {}", e)))?;
        
        let duration = start_time.elapsed();
        info!("✅ Materialized views refreshed in {:?}", duration);
        
        Ok(())
    }
    
    /// Get database statistics for monitoring
    pub async fn get_statistics(pool: &PgPool) -> DatabaseResult<DatabaseStats> {
        // Get table sizes
        let table_stats = sqlx::query_as::<_, TableStat>(
            r#"
            SELECT 
                schemaname,
                tablename,
                pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size,
                pg_total_relation_size(schemaname||'.'||tablename) as size_bytes
            FROM pg_tables 
            WHERE schemaname = 'public' 
            AND tablename LIKE 'positions_%'
            ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC
            "#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        
        // Get total positions count
        let total_positions = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM user_positions_summary"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        
        // Get active users count (positions updated in last 24h)
        let active_users = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT user_address) FROM user_positions_summary WHERE updated_at > NOW() - INTERVAL '24 hours'"
        )
        .fetch_one(pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
        
        Ok(DatabaseStats {
            table_stats,
            total_positions,
            active_users,
            last_updated: chrono::Utc::now(),
        })
    }
    
    /// Execute transaction with retry logic
    pub async fn execute_with_retry<F, T>(
        pool: &PgPool,
        operation: F,
        max_retries: u32,
    ) -> DatabaseResult<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = DatabaseResult<T>> + Send>>,
    {
        let mut attempts = 0;
        
        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }
                    
                    warn!("Database operation failed (attempt {}), retrying: {:?}", attempts, e);
                    tokio::time::sleep(Duration::from_millis(100 * attempts as u64)).await;
                }
            }
        }
    }
}

// ============================================================================
// HEALTH CHECK TYPES
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthStatus {
    pub connected: bool,
    pub version: String,
    pub response_time_ms: u64,
    pub pool_status: PoolStatus,
    pub materialized_view_exists: bool,
    pub partition_tables_count: i64,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PoolStatus {
    pub size: u32,
    pub idle: u32,
    pub used: u32,
    pub max_size: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStats {
    pub table_stats: Vec<TableStat>,
    pub total_positions: i64,
    pub active_users: i64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct TableStat {
    pub schemaname: String,
    pub tablename: String,
    pub size: String,
    pub size_bytes: i64,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Initialize database with proper error handling
pub async fn init(database_url: &str) -> DatabaseResult<PgPool> {
    let pool = Database::connect(database_url).await?;
    Database::migrate(&pool).await?;
    Ok(pool)
}

/// Simple health check for backwards compatibility
pub async fn health_check(pool: &PgPool) -> DatabaseResult<()> {
    Database::health_check(pool).await.map(|_| ())
}
