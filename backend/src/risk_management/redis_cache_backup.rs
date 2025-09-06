use crate::risk_management::types::{RiskMetrics, UserPositions, UserId, RiskError};
use redis::{Client, Commands, RedisResult, aio::ConnectionManager};
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Redis configuration for caching
#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    pub connection_url: String,
    pub connection_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub max_pool_size: u32,
    pub default_ttl_seconds: u64,
    pub risk_metrics_ttl_seconds: u64,
    pub position_ttl_seconds: u64,
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            connection_url: "redis://localhost:6379".to_string(),
            connection_timeout_ms: 5000,
            command_timeout_ms: 1000,
            max_pool_size: 20,
            default_ttl_seconds: 300,      // 5 minutes
            risk_metrics_ttl_seconds: 30,  // 30 seconds
            position_ttl_seconds: 300,     // 5 minutes
        }
    }
}

/// Serializable risk metrics for Redis storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRiskMetrics {
    pub total_exposure_usd: String,
    pub concentration_risk: String,
    pub var_95: String,
    pub max_drawdown: String,
    pub sharpe_ratio: String,
    pub win_rate: String,
    pub avg_trade_size: String,
    pub cached_at: u64,
}

impl From<&RiskMetrics> for CachedRiskMetrics {
    fn from(metrics: &RiskMetrics) -> Self {
        Self {
            total_exposure_usd: metrics.total_exposure_usd.to_string(),
            concentration_risk: metrics.concentration_risk.to_string(),
            var_95: metrics.var_95.to_string(),
            max_drawdown: metrics.max_drawdown.to_string(),
            sharpe_ratio: metrics.sharpe_ratio.to_string(),
            win_rate: metrics.win_rate.to_string(),
            avg_trade_size: metrics.avg_trade_size.to_string(),
            cached_at: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

impl TryFrom<CachedRiskMetrics> for RiskMetrics {
    type Error = RiskError;

    fn try_from(cached: CachedRiskMetrics) -> Result<Self, Self::Error> {
        Ok(Self {
            total_exposure_usd: cached.total_exposure_usd.parse()
                .map_err(|_| RiskError::SerializationError("Invalid total_exposure_usd".to_string()))?,
            concentration_risk: cached.concentration_risk.parse()
                .map_err(|_| RiskError::SerializationError("Invalid concentration_risk".to_string()))?,
            var_95: cached.var_95.parse()
                .map_err(|_| RiskError::SerializationError("Invalid var_95".to_string()))?,
            max_drawdown: cached.max_drawdown.parse()
                .map_err(|_| RiskError::SerializationError("Invalid max_drawdown".to_string()))?,
            sharpe_ratio: cached.sharpe_ratio.parse()
                .map_err(|_| RiskError::SerializationError("Invalid sharpe_ratio".to_string()))?,
            win_rate: cached.win_rate.parse()
                .map_err(|_| RiskError::SerializationError("Invalid win_rate".to_string()))?,
            avg_trade_size: cached.avg_trade_size.parse()
                .map_err(|_| RiskError::SerializationError("Invalid avg_trade_size".to_string()))?,
        })
    }
}

/// High-performance Redis cache for risk management data
pub struct RiskCache {
    connection: ConnectionManager,
    config: RedisCacheConfig,
}

impl RiskCache {
    /// Create a new Redis cache connection
    pub async fn new(config: RedisCacheConfig) -> Result<Self, RiskError> {
        let client = Client::open(config.connection_url.as_str())
            .map_err(|e| RiskError::CacheError(format!("Redis client creation failed: {}", e)))?;

        let connection = timeout(
            Duration::from_millis(config.connection_timeout_ms),
            ConnectionManager::new(client)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis connection timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Redis connection failed: {}", e)))?;

        Ok(Self { connection, config })
    }

    /// Cache risk metrics for a user
    pub async fn cache_risk_metrics(&mut self, user_id: UserId, metrics: &RiskMetrics) -> Result<(), RiskError> {
        let start = Instant::now();
        let key = format!("risk:{}:metrics", user_id);
        let cached_metrics = CachedRiskMetrics::from(metrics);
        
        let serialized = serde_json::to_string(&cached_metrics)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize metrics: {}", e)))?;

        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(key, serialized, self.config.risk_metrics_ttl_seconds as usize)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to cache metrics: {}", e)))?;

        // Target: <1ms for cache operations
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(1) {
            log::warn!("Slow risk metrics caching: {:?}", elapsed);
        }

        Ok(())
    }

    /// Get cached risk metrics for a user
    pub async fn get_cached_metrics(&mut self, user_id: UserId) -> Result<Option<RiskMetrics>, RiskError> {
        let start = Instant::now();
        let key = format!("risk:{}:metrics", user_id);

        let cached: RedisResult<Option<String>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached metrics: {}", e)))?;

        let result = match cached_data {
            Some(data) => {
                let cached_metrics: CachedRiskMetrics = serde_json::from_str(&data)
                    .map_err(|e| RiskError::SerializationError(format!("Failed to deserialize metrics: {}", e)))?;
                Some(cached_metrics.try_into()?)
            }
            None => None
        };

        // Target: <0.5ms for cache retrieval
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(1) {
            log::warn!("Slow risk metrics retrieval: {:?}", elapsed);
        }

        Ok(result)
    }

    /// Cache user positions
    pub async fn cache_user_positions(&mut self, user_id: UserId, positions: &UserPositions) -> Result<(), RiskError> {
        let key = format!("position:{}:current", user_id);
        
        let serialized = serde_json::to_string(positions)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize positions: {}", e)))?;

        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(key, serialized, self.config.position_ttl_seconds as usize)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to cache positions: {}", e)))?;
        Ok(())
    }

    /// Get cached user positions
    pub async fn get_cached_positions(&mut self, user_id: UserId) -> Result<Option<UserPositions>, RiskError> {
        let key = format!("position:{}:current", user_id);

        let cached: RedisResult<Option<String>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached positions: {}", e)))?;

        match cached_data {
            Some(data) => {
                match serde_json::from_str::<UserPositions>(&data) {
                    Ok(positions) => Ok(Some(positions)),
                    Err(e) => Err(RiskError::SerializationError(format!("Failed to deserialize positions: {}", e))),
                }
            }
            None => Ok(None)
        }
    }

    /// Batch update multiple user positions for efficiency
    pub async fn batch_update_positions(&mut self, updates: Vec<(UserId, UserPositions)>) -> Result<(), RiskError> {
        if updates.is_empty() {
            return Ok(());
        }

        let start = Instant::now();
        let mut pipe = redis::pipe();

        for (user_id, position) in updates {
            let key = format!("position:{}:current", user_id);
            let serialized = serde_json::to_string(&position)
                .map_err(|e| RiskError::SerializationError(format!("Failed to serialize position: {}", e)))?;
            pipe.set_ex(key, serialized, self.config.position_ttl_seconds as usize);
        }

        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms * 2), // Allow more time for batch
            pipe.query_async(&mut self.connection)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis batch command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to batch update positions: {}", e)))?;

        // Target: <5ms for batch operations
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(5) {
            log::warn!("Slow batch position update: {:?}", elapsed);
        }

        Ok(())
    }

    /// Cache price data for tokens
    pub async fn cache_token_price(&mut self, token_address: &str, price: Decimal) -> Result<(), RiskError> {
        let key = format!("price:{}:current", token_address);
        let price_data = serde_json::json!({
            "price": price.to_string(),
            "updated_at": chrono::Utc::now().timestamp_millis()
        });

        let serialized = price_data.to_string();

        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(key, serialized, 60) // 1 minute TTL for prices
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to cache price: {}", e)))?;
        Ok(())
    }

    /// Get cached token price
    pub async fn get_cached_price(&mut self, token_address: &str) -> Result<Option<Decimal>, RiskError> {
        let key = format!("price:{}:current", token_address);

        let cached: RedisResult<Option<String>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached price: {}", e)))?;

        match cached_data {
            Some(data) => {
                let price_data: serde_json::Value = serde_json::from_str(&data)
                    .map_err(|e| RiskError::SerializationError(format!("Failed to deserialize price: {}", e)))?;
                
                let price_str = price_data["price"].as_str()
                    .ok_or_else(|| RiskError::SerializationError("Invalid price format".to_string()))?;
                
                match serde_json::from_str::<Decimal>(&price_str) {
                    Ok(price) => Ok(Some(price)),
                    Err(e) => Err(RiskError::SerializationError(format!("Failed to parse price: {}", e))),
                }
            }
            None => Ok(None)
        }
    }

    /// Increment counter for metrics tracking
    pub async fn increment_counter(&mut self, key: &str) -> Result<i64, RiskError> {
        let result: RedisResult<i64> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.incr(key, 1)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to increment counter: {}", e)))
    }

    /// Set expiration on a key
    pub async fn expire_key(&mut self, key: &str, ttl_seconds: u64) -> Result<(), RiskError> {
        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.expire(key, ttl_seconds as usize)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to set expiration: {}", e)))?;
        Ok(())
    }

    /// Delete a key from cache
    pub async fn delete_key(&mut self, key: &str) -> Result<(), RiskError> {
        let result: RedisResult<()> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.del(key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        result.map_err(|e| RiskError::CacheError(format!("Failed to delete key: {}", e)))?;
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&mut self) -> Result<CacheStats, RiskError> {
        let info: RedisResult<String> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            redis::cmd("INFO").arg("memory").query_async(&mut self.connection)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let info_str = info.map_err(|e| RiskError::CacheError(format!("Failed to get cache info: {}", e)))?;
        
        // Parse memory usage from INFO output
        let memory_used = info_str
            .lines()
            .find(|line| line.starts_with("used_memory:"))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|val| val.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(CacheStats {
            memory_used_bytes: memory_used,
            connected: true,
        })
    }

    /// Health check for Redis connection
    pub async fn health_check(&mut self) -> Result<bool, RiskError> {
        let result: RedisResult<String> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            redis::cmd("PING").query_async(&mut self.connection)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis ping timeout".to_string()))?;

        match result {
            Ok(response) => Ok(response == "PONG"),
            Err(e) => Err(RiskError::CacheError(format!("Redis health check failed: {}", e)))
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_used_bytes: u64,
    pub connected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::types::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    // Note: These tests require a running Redis instance
    // Run with: docker run -d --name redis -p 6379:6379 redis:alpine

    async fn create_test_cache() -> RiskCache {
        let config = RedisCacheConfig::default();
        RiskCache::new(config).await.expect("Failed to create test cache")
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_risk_metrics_caching() {
        let mut cache = create_test_cache().await;
        let user_id = Uuid::new_v4();
        
        let metrics = RiskMetrics {
            total_exposure_usd: Decimal::from(10000),
            concentration_risk: Decimal::from(25),
            var_95: Decimal::from(5000),
            max_drawdown: Decimal::from(10),
            sharpe_ratio: Decimal::from_str("1.5").unwrap(),
            win_rate: Decimal::from(65),
            avg_trade_size: Decimal::from(2500),
        };

        // Cache metrics
        let result = cache.cache_risk_metrics(user_id, &metrics).await;
        assert!(result.is_ok());

        // Retrieve metrics
        let cached_metrics = cache.get_cached_metrics(user_id).await.unwrap();
        assert!(cached_metrics.is_some());
        
        let retrieved = cached_metrics.unwrap();
        assert_eq!(retrieved.total_exposure_usd, metrics.total_exposure_usd);
        assert_eq!(retrieved.concentration_risk, metrics.concentration_risk);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_position_caching() {
        let mut cache = create_test_cache().await;
        let user_id = Uuid::new_v4();
        
        let mut balances = HashMap::new();
        balances.insert(
            "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5".to_string(),
            TokenBalance {
                amount: Decimal::from(1000),
                avg_cost: Decimal::from(1900),
            }
        );

        let positions = UserPositions {
            balances,
            pnl: Decimal::from(100),
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        };

        // Cache positions
        let result = cache.cache_user_positions(user_id, &positions).await;
        assert!(result.is_ok());

        // Retrieve positions
        let cached_positions = cache.get_cached_positions(user_id).await.unwrap();
        assert!(cached_positions.is_some());
        
        let retrieved = cached_positions.unwrap();
        assert_eq!(retrieved.pnl, positions.pnl);
        assert_eq!(retrieved.balances.len(), 1);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_batch_position_updates() {
        let mut cache = create_test_cache().await;
        
        let updates = vec![
            (Uuid::new_v4(), UserPositions {
                balances: HashMap::new(),
                pnl: Decimal::from(100),
                last_updated: chrono::Utc::now().timestamp_millis() as u64,
            }),
            (Uuid::new_v4(), UserPositions {
                balances: HashMap::new(),
                pnl: Decimal::from(200),
                last_updated: chrono::Utc::now().timestamp_millis() as u64,
            }),
        ];

        let result = cache.batch_update_positions(updates).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_price_caching() {
        let mut cache = create_test_cache().await;
        let token_address = "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5";
        let price = Decimal::from(1900);

        // Cache price
        let result = cache.cache_token_price(token_address, price).await;
        assert!(result.is_ok());

        // Retrieve price
        let cached_price = cache.get_cached_price(token_address).await.unwrap();
        assert!(cached_price.is_some());
        assert_eq!(cached_price.unwrap(), price);
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_health_check() {
        let mut cache = create_test_cache().await;
        let result = cache.health_check().await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    #[ignore] // Requires Redis running
    async fn test_cache_stats() {
        let mut cache = create_test_cache().await;
        let stats = cache.get_cache_stats().await.unwrap();
        assert!(stats.connected);
        assert!(stats.memory_used_bytes > 0);
    }
}
