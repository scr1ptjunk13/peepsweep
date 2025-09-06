use crate::risk_management::types::*;
use redis::{AsyncCommands, Client, RedisResult};
use redis::aio::MultiplexedConnection;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use rust_decimal::Decimal;

/// Configuration for Redis cache
#[derive(Debug, Clone)]
pub struct RedisCacheConfig {
    pub redis_url: String,
    pub default_ttl_seconds: u64,
    pub command_timeout_ms: u64,
    pub max_batch_size: usize,
    pub enable_compression: bool,
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            default_ttl_seconds: 300, // 5 minutes
            command_timeout_ms: 1000, // 1 second
            max_batch_size: 100,
            enable_compression: false,
        }
    }
}

/// Cached risk metrics with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRiskMetrics {
    metrics: RiskMetrics,
    cached_at: u64,
    ttl_seconds: u64,
}

impl TryInto<RiskMetrics> for CachedRiskMetrics {
    type Error = RiskError;

    fn try_into(self) -> Result<RiskMetrics, Self::Error> {
        let now = chrono::Utc::now().timestamp() as u64;
        if now > self.cached_at + self.ttl_seconds {
            return Err(RiskError::CacheError("Cached data expired".to_string()));
        }
        Ok(self.metrics)
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub errors: u64,
    pub total_requests: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.hits as f64 / self.total_requests as f64
        }
    }
}

/// High-performance Redis cache for risk management data
pub struct RiskCache {
    connection: MultiplexedConnection,
    config: RedisCacheConfig,
}

impl RiskCache {
    /// Create new Redis cache instance with default config
    pub async fn new() -> Result<Self, RiskError> {
        Self::with_config(RedisCacheConfig::default()).await
    }

    /// Create new Redis cache instance
    pub async fn with_config(config: RedisCacheConfig) -> Result<Self, RiskError> {
        let client = redis::Client::open(config.redis_url.as_str())
            .map_err(|e| RiskError::CacheError(format!("Failed to create Redis client: {}", e)))?;

        let connection = client.get_multiplexed_async_connection().await
            .map_err(|e| RiskError::CacheError(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self { connection, config })
    }

    /// Cache risk metrics with TTL
    pub async fn cache_metrics(&mut self, user_id: UserId, metrics: &RiskMetrics) -> Result<(), RiskError> {
        let key = format!("risk:{}:metrics", user_id);
        
        let cached_metrics = CachedRiskMetrics {
            metrics: metrics.clone(),
            cached_at: chrono::Utc::now().timestamp() as u64,
            ttl_seconds: self.config.default_ttl_seconds,
        };

        let serialized_data = serde_json::to_string(&cached_metrics)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize metrics: {}", e)))?;

        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(&key, &serialized_data, self.config.default_ttl_seconds as usize)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Failed to cache metrics: {}", e)))?;

        Ok(())
    }

    /// Get cached risk metrics for a user
    pub async fn get_cached_metrics(&mut self, user_id: UserId) -> Result<Option<RiskMetrics>, RiskError> {
        let start = Instant::now();
        let key = format!("risk:{}:metrics", user_id);

        let cached_result: RedisResult<Option<Vec<u8>>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(&key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached_result
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached metrics: {}", e)))?;

        let result = match cached_data {
            Some(bytes) => {
                let data = String::from_utf8(bytes)
                    .map_err(|e| RiskError::SerializationError(format!("Invalid UTF-8: {}", e)))?;
                let cached_metrics: CachedRiskMetrics = serde_json::from_str(&data)
                    .map_err(|e| RiskError::SerializationError(format!("Failed to deserialize metrics: {}", e)))?;
                Some(cached_metrics.try_into()?)
            }
            None => None,
        };

        // Target: <0.5ms for cache retrieval
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(500) {
            log::warn!("Slow risk metrics retrieval: {:?}", elapsed);
        }

        Ok(result)
    }

    /// Cache user positions
    pub async fn cache_positions(&mut self, user_id: UserId, positions: &UserPositions) -> Result<(), RiskError> {
        let key = format!("position:{}:current", user_id);
        
        let serialized = serde_json::to_string(positions)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize positions: {}", e)))?;

        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(&key, &serialized, self.config.default_ttl_seconds as usize)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Failed to cache positions: {}", e)))?;

        Ok(())
    }

    /// Get cached user positions
    pub async fn get_cached_positions(&mut self, user_id: UserId) -> Result<Option<UserPositions>, RiskError> {
        let key = format!("position:{}:current", user_id);

        let cached_result: RedisResult<Option<Vec<u8>>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(&key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached_result
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached positions: {}", e)))?;

        match cached_data {
            Some(bytes) => {
                let data = String::from_utf8(bytes)
                    .map_err(|e| RiskError::SerializationError(format!("Invalid UTF-8: {}", e)))?;
                match serde_json::from_str::<UserPositions>(&data) {
                    Ok(positions) => Ok(Some(positions)),
                    Err(_) => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    /// Batch update multiple user positions for efficiency
    pub async fn batch_update_positions(&mut self, updates: Vec<(UserId, UserPositions)>) -> Result<(), RiskError> {
        if updates.is_empty() {
            return Ok(());
        }

        // Split into batches to avoid overwhelming Redis
        let batch_size = self.config.max_batch_size.min(updates.len());
        
        for chunk in updates.chunks(batch_size) {
            let mut pipe = redis::pipe();
            
            for (user_id, positions) in chunk {
                let key = format!("position:{}:current", user_id);
                let serialized = serde_json::to_string(positions)
                    .map_err(|e| RiskError::SerializationError(format!("Failed to serialize positions: {}", e)))?;
                
                pipe.set_ex(&key, &serialized, self.config.default_ttl_seconds as usize);
            }

            let _: () = timeout(
                Duration::from_millis(self.config.command_timeout_ms * 2), // Longer timeout for batch
                pipe.query_async(&mut self.connection)
            )
            .await
            .map_err(|_| RiskError::CacheError("Redis batch command timeout".to_string()))?
            .map_err(|e| RiskError::CacheError(format!("Failed to batch update positions: {}", e)))?;
        }

        Ok(())
    }

    /// Cache token price with timestamp
    pub async fn cache_price(&mut self, token: &TokenAddress, price: Decimal, timestamp: u64) -> Result<(), RiskError> {
        let key = format!("price:{}:latest", token);
        
        let price_data = serde_json::json!({
            "price": price.to_string(),
            "timestamp": timestamp,
            "token": token
        });

        let serialized = serde_json::to_string(&price_data)
            .map_err(|e| RiskError::SerializationError(format!("Failed to serialize price: {}", e)))?;

        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(&key, &serialized, (self.config.default_ttl_seconds / 2) as usize) // Shorter TTL for prices
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Failed to cache price: {}", e)))?;

        Ok(())
    }

    /// Get cached token price
    pub async fn get_cached_price(&mut self, token: &TokenAddress) -> Result<Option<Decimal>, RiskError> {
        let key = format!("price:{}:latest", token);

        let cached_result: RedisResult<Option<Vec<u8>>> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(&key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        let cached_data = cached_result
            .map_err(|e| RiskError::CacheError(format!("Failed to get cached price: {}", e)))?;

        match cached_data {
            Some(bytes) => {
                let data = String::from_utf8(bytes)
                    .map_err(|e| RiskError::SerializationError(format!("Invalid UTF-8: {}", e)))?;
                let price_data: serde_json::Value = serde_json::from_str(&data)
                    .map_err(|e| RiskError::SerializationError(format!("Failed to deserialize price: {}", e)))?;
                
                let price_str = price_data["price"].as_str()
                    .ok_or_else(|| RiskError::SerializationError("Invalid price format".to_string()))?;
                
                match price_str.parse::<Decimal>() {
                    Ok(price) => Ok(Some(price)),
                    Err(e) => Err(RiskError::SerializationError(format!("Failed to parse price: {}", e))),
                }
            }
            None => Ok(None),
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

    /// Simple get method for string values
    pub async fn get(&mut self, key: &str) -> Result<Option<String>, RiskError> {
        let result: RedisResult<String> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?;

        match result {
            Ok(value) => Ok(Some(value)),
            Err(e) => {
                // Check if it's a nil response (key doesn't exist)
                if e.kind() == redis::ErrorKind::TypeError {
                    Ok(None)
                } else {
                    Err(RiskError::CacheError(format!("Failed to get value: {}", e)))
                }
            }
        }
    }

    /// Simple set method for string values with optional TTL
    pub async fn set(&mut self, key: &str, value: &str, ttl_seconds: Option<u64>) -> Result<(), RiskError> {
        let result = if let Some(ttl) = ttl_seconds {
            timeout(
                Duration::from_millis(self.config.command_timeout_ms),
                self.connection.set_ex(key, value, ttl as usize)
            )
            .await
            .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?
        } else {
            timeout(
                Duration::from_millis(self.config.command_timeout_ms),
                self.connection.set(key, value)
            )
            .await
            .map_err(|_| RiskError::CacheError("Redis command timeout".to_string()))?
        };

        result.map_err(|e| RiskError::CacheError(format!("Failed to set value: {}", e)))
    }

    /// Get cache statistics
    pub async fn get_stats(&mut self) -> Result<CacheStats, RiskError> {
        let hits: i64 = self.connection.get("cache:stats:hits").await.unwrap_or(0);
        let misses: i64 = self.connection.get("cache:stats:misses").await.unwrap_or(0);
        let errors: i64 = self.connection.get("cache:stats:errors").await.unwrap_or(0);

        Ok(CacheStats {
            hits: hits as u64,
            misses: misses as u64,
            errors: errors as u64,
            total_requests: (hits + misses) as u64,
        })
    }

    /// Health check for Redis connection
    pub async fn health_check(&mut self) -> Result<bool, RiskError> {
        let test_key = "health:check";
        let test_value = "ok";

        // Test write
        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.set_ex(test_key, test_value, 10)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis health check timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Redis health check failed: {}", e)))?;

        // Test read
        let result: RedisResult<String> = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.get(test_key)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis health check timeout".to_string()))?;

        match result {
            Ok(value) => Ok(value == test_value),
            Err(e) => Err(RiskError::CacheError(format!("Redis health check failed: {}", e))),
        }
    }

    /// Delete a specific key
    pub async fn delete(&mut self, key: &str) -> Result<(), RiskError> {
        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms),
            self.connection.del(key)
        ).await
        .map_err(|_| RiskError::CacheError("Redis delete timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Redis delete error: {}", e)))?;
        
        Ok(())
    }

    /// Clear all cached data (use with caution)
    pub async fn clear_all(&mut self) -> Result<(), RiskError> {
        let _: () = timeout(
            Duration::from_millis(self.config.command_timeout_ms * 5), // Longer timeout for FLUSHDB
            redis::cmd("FLUSHDB").query_async(&mut self.connection)
        )
        .await
        .map_err(|_| RiskError::CacheError("Redis clear timeout".to_string()))?
        .map_err(|e| RiskError::CacheError(format!("Failed to clear cache: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_cache_metrics() {
        let config = RedisCacheConfig::default();
        let mut cache = RiskCache::with_config(config).await.unwrap();

        let user_id = uuid::Uuid::new_v4();
        let metrics = RiskMetrics {
            total_exposure_usd: Decimal::from(20000),
            concentration_risk: Decimal::from(25),
            var_95: Decimal::from(800),
            max_drawdown: Decimal::from(600),
            sharpe_ratio: Decimal::from_str("1.2").unwrap(),
            win_rate: Decimal::from(65),
            avg_trade_size: Decimal::from(500),
        };

        // Cache metrics
        let user_uuid = uuid::Uuid::new_v4();
        cache.cache_metrics(user_uuid, &metrics).await.unwrap();

        // Retrieve metrics
        let cached = cache.get_cached_metrics(user_uuid).await.unwrap();
        assert!(cached.is_some());
        let cached_metrics = cached.unwrap();
        assert_eq!(cached_metrics.total_exposure_usd, metrics.total_exposure_usd);
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_cache_positions() {
        let config = RedisCacheConfig::default();
        let mut cache = RiskCache::with_config(config).await.unwrap();

        let user_id = uuid::Uuid::new_v4();
        let positions = UserPositions {
            balances: HashMap::new(),
            pnl: Decimal::from(100),
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        };

        // Cache positions
        cache.cache_positions(user_id, &positions).await.unwrap();

        // Test retrieval
        let cached = cache.get_cached_positions(user_id).await.unwrap();
        assert!(cached.is_some());
        let cached_positions = cached.unwrap();
        assert_eq!(cached_positions.pnl, positions.pnl);
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_health_check() {
        let config = RedisCacheConfig::default();
        let mut cache = RiskCache::with_config(config).await.unwrap();

        let health = cache.health_check().await.unwrap();
        assert!(health);
    }
}
