use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, interval};
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;
use crate::Address;

use super::{CacheManager, CacheConfig};
use crate::{CacheResult, CacheError, USER_POSITIONS_TTL, TOKEN_PRICES_TTL, IL_SNAPSHOTS_TTL};
use crate::database::models::{UserPositionSummary, IlSnapshot, TokenPrice};

#[derive(Debug, Clone)]
pub enum CacheStrategy {
    WriteThrough,
    WriteBack,
    WriteAround,
    ReadThrough,
    CacheAside,
}

#[derive(Debug, Clone)]
pub struct CacheStrategyConfig {
    pub strategy: CacheStrategy,
    pub batch_size: usize,
    pub flush_interval_seconds: u64,
    pub enable_warming: bool,
    pub warming_batch_size: usize,
}

impl Default for CacheStrategyConfig {
    fn default() -> Self {
        Self {
            strategy: CacheStrategy::CacheAside,
            batch_size: 100,
            flush_interval_seconds: 300, // 5 minutes
            enable_warming: true,
            warming_batch_size: 50,
        }
    }
}

pub struct CacheStrategies {
    cache_manager: Arc<CacheManager>,
    config: CacheStrategyConfig,
}

impl CacheStrategies {
    pub fn new(cache_manager: Arc<CacheManager>, config: Option<CacheStrategyConfig>) -> Self {
        Self {
            cache_manager,
            config: config.unwrap_or_default(),
        }
    }

    // Write-Through Strategy: Write to cache and database simultaneously
    pub async fn write_through_user_positions(
        &self,
        user_address: &str,
        positions: &[UserPositionSummary],
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<()> {
        // Write to database first
        for position in positions {
            if let Err(e) = crate::database::queries::upsert_user_position_summary(db_pool, position).await {
                return Err(CacheError::OperationError(format!("Database write failed: {}", e)));
            }
        }

        // Then write to cache
        self.cache_manager.set_user_positions(user_address, positions).await?;
        
        tracing::debug!("Write-through completed for user: {}", user_address);
        Ok(())
    }

    // Write-Back Strategy: Write to cache immediately, database later
    pub async fn write_back_user_positions(
        &self,
        user_address: &str,
        positions: &[UserPositionSummary],
    ) -> CacheResult<()> {
        // Mark as dirty in cache with special metadata
        let cache_key = format!("user_positions_dirty:{}", user_address);
        self.cache_manager.set(&cache_key, &true, USER_POSITIONS_TTL).await?;
        
        // Write to cache
        self.cache_manager.set_user_positions(user_address, positions).await?;
        
        tracing::debug!("Write-back cached for user: {}", user_address);
        Ok(())
    }

    // Write-Around Strategy: Write to database only, bypass cache
    pub async fn write_around_user_positions(
        &self,
        user_address: &str,
        positions: &[UserPositionSummary],
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<()> {
        // Write only to database
        for position in positions {
            if let Err(e) = crate::database::queries::upsert_user_position_summary(db_pool, position).await {
                return Err(CacheError::OperationError(format!("Database write failed: {}", e)));
            }
        }

        // Invalidate cache to ensure consistency
        self.cache_manager.invalidate_user_positions(user_address).await;
        
        tracing::debug!("Write-around completed for user: {}", user_address);
        Ok(())
    }

    // Read-Through Strategy: Read from cache, if miss then read from database and cache
    pub async fn read_through_user_positions(
        &self,
        user_address: &str,
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<Vec<UserPositionSummary>> {
        // Try cache first
        if let Some(positions) = self.cache_manager.get_user_positions(user_address).await? {
            tracing::debug!("Cache hit for user positions: {}", user_address);
            return Ok(positions);
        }

        // Cache miss, read from database
        let positions = crate::database::queries::get_user_positions(db_pool, user_address).await
            .map_err(|e| CacheError::OperationError(format!("Database read failed: {}", e)))?;

        // Cache the result
        self.cache_manager.set_user_positions(user_address, &positions).await?;
        
        tracing::debug!("Read-through completed for user: {}", user_address);
        Ok(positions)
    }

    // Cache-Aside Strategy: Application manages cache explicitly
    pub async fn cache_aside_get_user_positions(
        &self,
        user_address: &str,
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<Vec<UserPositionSummary>> {
        // Check cache first
        if let Some(positions) = self.cache_manager.get_user_positions(user_address).await? {
            return Ok(positions);
        }

        // Cache miss, read from database
        let positions = crate::database::queries::get_user_positions(db_pool, user_address).await
            .map_err(|e| CacheError::OperationError(format!("Database read failed: {}", e)))?;

        // Explicitly cache the result
        if let Err(e) = self.cache_manager.set_user_positions(user_address, &positions).await {
            tracing::warn!("Failed to cache user positions: {}", e);
        }

        Ok(positions)
    }

    // Token Price Caching Strategies
    pub async fn cache_token_price_with_strategy(
        &self,
        token_address: &Address,
        price: Decimal,
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<()> {
        match self.config.strategy {
            CacheStrategy::WriteThrough => {
                // Write to database first
                let token_price = TokenPrice {
                    token_address: token_address.to_string(),
                    price_usd: Some(price),
                    price_eth: None,
                    block_number: 0,
                    timestamp: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                
                if let Err(e) = crate::database::queries::upsert_token_price(
                    db_pool, 
                    &token_price.token_address, 
                    token_price.price_usd.unwrap_or_default(), 
                    token_price.price_eth, 
                    token_price.block_number
                ).await {
                    return Err(CacheError::OperationError(format!("Database write failed: {}", e)));
                }

                // Then cache
                self.cache_manager.set_token_price(token_address, price).await;
            }
            CacheStrategy::WriteBack => {
                // Cache immediately, mark as dirty
                let dirty_key = format!("token_price_dirty:{}", token_address);
                self.cache_manager.set(&dirty_key, &true, TOKEN_PRICES_TTL).await?;
                self.cache_manager.set_token_price(token_address, price).await;
            }
            CacheStrategy::WriteAround => {
                // Write only to database
                if let Err(e) = crate::database::queries::upsert_token_price(
                    db_pool, 
                    &token_address.to_string(), 
                    price, 
                    None, 
                    0
                ).await {
                    return Err(CacheError::OperationError(format!("Database write failed: {}", e)));
                }
            }
            _ => {
                // Default to cache-aside
                self.cache_manager.set_token_price(token_address, price).await;
            }
        }

        Ok(())
    }

    // IL Snapshot Caching
    pub async fn cache_il_snapshot_with_strategy(
        &self,
        position_id: i64,
        snapshot: &IlSnapshot,
        db_pool: &sqlx::PgPool,
    ) -> CacheResult<()> {
        match self.config.strategy {
            CacheStrategy::WriteThrough => {
                // Write to database first
                if let Err(e) = crate::database::queries::insert_il_snapshot(db_pool, snapshot).await {
                    return Err(CacheError::OperationError(format!("Database write failed: {}", e)));
                }
                
                // Then cache
                self.cache_manager.set_il_snapshot(position_id, snapshot).await?;
            }
            CacheStrategy::WriteBack => {
                // Cache immediately
                self.cache_manager.set_il_snapshot(position_id, snapshot).await?;
                
                // Mark as dirty for later database write
                let dirty_key = format!("il_snapshot_dirty:{}", position_id);
                self.cache_manager.set(&dirty_key, &true, IL_SNAPSHOTS_TTL).await?;
            }
            _ => {
                // Default behavior
                self.cache_manager.set_il_snapshot(position_id, snapshot).await?;
            }
        }

        Ok(())
    }

    // Cache Warming: Preload frequently accessed data
    pub async fn warm_cache(&self, db_pool: &sqlx::PgPool) -> CacheResult<()> {
        if !self.config.enable_warming {
            return Ok(());
        }

        tracing::info!("Starting cache warming process");

        // Warm user positions for active users
        let active_users = self.get_active_users(db_pool).await?;
        for chunk in active_users.chunks(self.config.warming_batch_size) {
            for user_address in chunk {
                if let Ok(positions) = crate::database::queries::get_user_positions(db_pool, user_address).await {
                    if let Err(e) = self.cache_manager.set_user_positions(user_address, &positions).await {
                        tracing::warn!("Failed to warm cache for user {}: {}", user_address, e);
                    }
                }
            }
            
            // Small delay to avoid overwhelming the system
            sleep(Duration::from_millis(100)).await;
        }

        // Warm token prices for popular tokens
        let popular_tokens = self.get_popular_tokens(db_pool).await?;
        for token_address in popular_tokens {
            if let Ok(addr) = token_address.parse::<Address>() {
                if let Ok(Some(token_price)) = crate::database::queries::get_latest_token_price(db_pool, &addr).await {
                    if let Some(price) = token_price.price_usd {
                        self.cache_manager.set_token_price(&addr, price).await;
                    }
                }
            }
        }

        tracing::info!("Cache warming completed");
        Ok(())
    }

    // Periodic cache maintenance
    pub async fn start_maintenance_task(&self, db_pool: sqlx::PgPool) {
        let cache_manager = self.cache_manager.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(config.flush_interval_seconds));
            
            loop {
                interval.tick().await;
                
                // Flush dirty entries for write-back strategy
                if matches!(config.strategy, CacheStrategy::WriteBack) {
                    if let Err(e) = Self::flush_dirty_entries(&cache_manager, &db_pool).await {
                        tracing::error!("Failed to flush dirty cache entries: {}", e);
                    }
                }
                
                // Cleanup expired L1 cache entries
                // This is handled internally by the cache manager
                
                tracing::debug!("Cache maintenance cycle completed");
            }
        });
    }

    // Flush dirty entries to database (for write-back strategy)
    async fn flush_dirty_entries(cache_manager: &CacheManager, db_pool: &sqlx::PgPool) -> CacheResult<()> {
        // This would need to be implemented based on how dirty entries are tracked
        // For now, we'll just log that this would happen
        tracing::debug!("Flushing dirty cache entries to database");
        Ok(())
    }

    // Helper methods
    async fn get_active_users(&self, db_pool: &sqlx::PgPool) -> CacheResult<Vec<String>> {
        let users = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT user_address FROM user_positions_summary 
             WHERE updated_at > NOW() - INTERVAL '24 hours' 
             LIMIT 100"
        )
        .fetch_all(db_pool)
        .await
        .map_err(|e| CacheError::OperationError(format!("Failed to get active users: {}", e)))?;

        Ok(users)
    }

    async fn get_popular_tokens(&self, db_pool: &sqlx::PgPool) -> CacheResult<Vec<String>> {
        let tokens = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT token_address FROM token_prices 
             WHERE timestamp > NOW() - INTERVAL '1 hour' 
             ORDER BY timestamp DESC 
             LIMIT 50"
        )
        .fetch_all(db_pool)
        .await
        .map_err(|e| CacheError::OperationError(format!("Failed to get popular tokens: {}", e)))?;

        Ok(tokens)
    }

    // Batch operations for better performance
    pub async fn batch_cache_user_positions(
        &self,
        user_positions: Vec<(String, Vec<UserPositionSummary>)>,
    ) -> CacheResult<()> {
        for (user_address, positions) in user_positions {
            self.cache_manager.set_user_positions(&user_address, &positions).await?;
        }
        Ok(())
    }

    pub async fn batch_invalidate_users(&self, user_addresses: &[String]) {
        for user_address in user_addresses {
            self.cache_manager.invalidate_user_positions(user_address).await;
        }
    }

    // Cache statistics and monitoring
    pub async fn get_cache_statistics(&self) -> serde_json::Value {
        let metrics = self.cache_manager.get_metrics().await;
        
        serde_json::json!({
            "strategy": format!("{:?}", self.config.strategy),
            "l1_hit_rate": if metrics.total_requests > 0 {
                metrics.l1_hits as f64 / metrics.total_requests as f64
            } else { 0.0 },
            "l2_hit_rate": if metrics.total_requests > 0 {
                metrics.l2_hits as f64 / metrics.total_requests as f64
            } else { 0.0 },
            "total_requests": metrics.total_requests,
            "l1_size": metrics.l1_size,
            "config": {
                "batch_size": self.config.batch_size,
                "flush_interval_seconds": self.config.flush_interval_seconds,
                "enable_warming": self.config.enable_warming
            }
        })
    }
}
