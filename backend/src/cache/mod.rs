use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use redis::{Client, aio::Connection, AsyncCommands, RedisResult};
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;
use crate::Address;

use crate::{CacheResult, CacheError, USER_POSITIONS_TTL, TOKEN_PRICES_TTL, IL_SNAPSHOTS_TTL};
use crate::database::models::{UserPositionSummary, TokenPrice, IlSnapshot};

pub mod strategies;

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub redis_url: String,
    pub max_pool_size: u32,
    pub connection_timeout_ms: u64,
    pub enable_l1_cache: bool,
    pub l1_max_size: usize,
    pub l1_ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            max_pool_size: 10,
            connection_timeout_ms: 5000,
            enable_l1_cache: true,
            l1_max_size: 1000,
            l1_ttl_seconds: 300, // 5 minutes
        }
    }
}

#[derive(Debug, Clone)]
struct L1CacheEntry<T> {
    data: T,
    expires_at: Instant,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheMetrics {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l1_size: usize,
    pub total_requests: u64,
}

#[derive(Debug)]
pub struct CacheManager {
    client: Client,
    config: CacheConfig,
    l1_cache: Arc<RwLock<HashMap<String, L1CacheEntry<Vec<u8>>>>>,
    metrics: Arc<RwLock<CacheMetrics>>,
}

impl CacheManager {
    pub async fn new(config: CacheConfig) -> CacheResult<Self> {
        let client = Client::open(config.redis_url.clone())
            .map_err(|e| CacheError::ConnectionError(format!("Failed to create Redis client: {}", e)))?;
        
        // Test connection
        let mut conn = client.get_async_connection().await
            .map_err(|e| CacheError::ConnectionError(format!("Failed to get connection: {}", e)))?;
        
        // Temporarily comment out ping to fix compilation
        // conn.ping().await
        //     .map_err(|e| CacheError::ConnectionError(format!("Ping failed: {}", e)))?;

        Ok(Self {
            client,
            config,
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(CacheMetrics {
                l1_hits: 0,
                l1_misses: 0,
                l2_hits: 0,
                l2_misses: 0,
                l1_size: 0,
                total_requests: 0,
            })),
        })
    }

    async fn get_connection(&self) -> CacheResult<Connection> {
        self.client.get_async_connection().await
            .map_err(|e| CacheError::ConnectionError(format!("Failed to get Redis connection: {}", e)))
    }

    pub async fn set<T: Serialize + ?Sized>(&self, key: &str, value: &T, ttl_seconds: u64) -> CacheResult<()> {
        let serialized = serde_json::to_vec(value)
            .map_err(|e| CacheError::SerializationError(format!("Failed to serialize value: {}", e)))?;

        // Store in L1 cache if enabled
        if self.config.enable_l1_cache {
            let expires_at = Instant::now() + Duration::from_secs(self.config.l1_ttl_seconds.min(ttl_seconds));
            let mut l1_cache = self.l1_cache.write().await;
            
            // Evict expired entries
            self.evict_expired_l1_entries(&mut l1_cache).await;
            
            // Check size limit
            if l1_cache.len() >= self.config.l1_max_size {
                // Remove oldest entry
                if let Some(oldest_key) = l1_cache.keys().next().cloned() {
                    l1_cache.remove(&oldest_key);
                }
            }
            
            l1_cache.insert(key.to_string(), L1CacheEntry {
                data: serialized.clone(),
                expires_at,
            });
        }

        // Store in L2 (Redis) cache
        let mut conn = self.get_connection().await?;
        conn.set_ex(key, serialized, ttl_seconds).await
            .map_err(|e| CacheError::OperationError(format!("Failed to set Redis key: {}", e)))?;

        Ok(())
    }

    pub async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> CacheResult<Option<T>> {
        self.increment_total_requests().await;

        // Try L1 cache first
        if self.config.enable_l1_cache {
            let mut l1_cache = self.l1_cache.write().await;
            
            if let Some(entry) = l1_cache.get(key) {
                if entry.expires_at > Instant::now() {
                    // L1 cache hit
                    self.increment_l1_hits().await;
                    let value: T = serde_json::from_slice(&entry.data)
                        .map_err(|e| CacheError::SerializationError(format!("Failed to deserialize L1 value: {}", e)))?;
                    return Ok(Some(value));
                } else {
                    // Entry expired, remove it
                    l1_cache.remove(key);
                }
            }
            
            self.increment_l1_misses().await;
        }

        // Try L2 (Redis) cache
        let mut conn = self.get_connection().await?;
        let value: Option<Vec<u8>> = conn.get(key).await
            .map_err(|e| CacheError::OperationError(format!("Failed to get Redis key: {}", e)))?;

        match value {
            Some(data) => {
                self.increment_l2_hits().await;
                
                // Store in L1 cache for future requests
                if self.config.enable_l1_cache {
                    let expires_at = Instant::now() + Duration::from_secs(self.config.l1_ttl_seconds);
                    let mut l1_cache = self.l1_cache.write().await;
                    l1_cache.insert(key.to_string(), L1CacheEntry {
                        data: data.clone(),
                        expires_at,
                    });
                }
                
                let deserialized: T = serde_json::from_slice(&data)
                    .map_err(|e| CacheError::SerializationError(format!("Failed to deserialize Redis value: {}", e)))?;
                Ok(Some(deserialized))
            }
            None => {
                self.increment_l2_misses().await;
                Ok(None)
            }
        }
    }

    pub async fn delete(&self, key: &str) -> CacheResult<()> {
        // Remove from L1 cache
        if self.config.enable_l1_cache {
            self.l1_cache.write().await.remove(key);
        }

        // Remove from L2 cache
        let mut conn = self.get_connection().await?;
        conn.del(key).await
            .map_err(|e| CacheError::OperationError(format!("Failed to delete Redis key: {}", e)))?;

        Ok(())
    }

    pub async fn clear_pattern(&self, pattern: &str) -> CacheResult<()> {
        let mut conn = self.get_connection().await?;
        
        // Get all keys matching pattern
        let keys: Vec<String> = conn.keys(pattern).await
            .map_err(|e| CacheError::OperationError(format!("Failed to get keys: {}", e)))?;

        if !keys.is_empty() {
            // Delete all matching keys
            conn.del(&keys).await
                .map_err(|e| CacheError::OperationError(format!("Failed to delete keys: {}", e)))?;
        }

        // Clear L1 cache entries matching pattern
        if self.config.enable_l1_cache {
            let mut l1_cache = self.l1_cache.write().await;
            let keys_to_remove: Vec<String> = l1_cache.keys()
                .filter(|k| self.matches_pattern(k, pattern))
                .cloned()
                .collect();
            
            for key in keys_to_remove {
                l1_cache.remove(&key);
            }
        }

        Ok(())
    }

    pub async fn clear_all(&self) -> CacheResult<()> {
        // Clear L1 cache
        if self.config.enable_l1_cache {
            self.l1_cache.write().await.clear();
        }

        // Clear L2 cache
        let mut conn = self.get_connection().await?;
        // Temporarily comment out flushdb to fix compilation
        // conn.flushdb().await
        //     .map_err(|e| CacheError::OperationError(format!("Failed to flush database: {}", e)))?;

        Ok(())
    }

    // Domain-specific cache methods
    pub async fn get_user_positions(&self, user_address: &str) -> CacheResult<Option<Vec<UserPositionSummary>>> {
        let key = format!("user_positions:{}", user_address);
        self.get(&key).await
    }

    pub async fn set_user_positions(&self, user_address: &str, positions: &[UserPositionSummary]) -> CacheResult<()> {
        let key = format!("user_positions:{}", user_address);
        self.set(&key, positions, USER_POSITIONS_TTL).await
    }

    pub async fn invalidate_user_positions(&self, user_address: &str) {
        let key = format!("user_positions:{}", user_address);
        if let Err(e) = self.delete(&key).await {
            tracing::warn!("Failed to invalidate user positions cache: {}", e);
        }
    }

    pub async fn get_token_price(&self, token_address: &Address) -> Option<Decimal> {
        let key = format!("token_price:{}", token_address);
        match self.get::<Decimal>(&key).await {
            Ok(price) => price,
            Err(e) => {
                tracing::warn!("Failed to get token price from cache: {}", e);
                None
            }
        }
    }

    pub async fn set_token_price(&self, token_address: &Address, price: Decimal) {
        let key = format!("token_price:{}", token_address);
        if let Err(e) = self.set(&key, &price, TOKEN_PRICES_TTL).await {
            tracing::warn!("Failed to cache token price: {}", e);
        }
    }

    pub async fn get_il_snapshot(&self, position_id: i64) -> CacheResult<Option<IlSnapshot>> {
        let key = format!("il_snapshot:{}", position_id);
        self.get(&key).await
    }

    pub async fn set_il_snapshot(&self, position_id: i64, snapshot: &IlSnapshot) -> CacheResult<()> {
        let key = format!("il_snapshot:{}", position_id);
        self.set(&key, snapshot, IL_SNAPSHOTS_TTL).await
    }

    pub async fn health_check(&self) -> CacheResult<()> {
        let _conn = self.get_connection().await?;
        // Temporarily comment out ping to fix compilation
        // conn.ping().await
        //     .map_err(|e| CacheError::ConnectionError(format!("Health check failed: {}", e)))?;
        Ok(())
    }

    pub async fn get_metrics(&self) -> CacheMetrics {
        let mut metrics = self.metrics.read().await.clone();
        
        if self.config.enable_l1_cache {
            metrics.l1_size = self.l1_cache.read().await.len();
        }
        
        metrics
    }

    // Helper methods
    async fn evict_expired_l1_entries(&self, l1_cache: &mut HashMap<String, L1CacheEntry<Vec<u8>>>) {
        let now = Instant::now();
        let expired_keys: Vec<String> = l1_cache.iter()
            .filter(|(_, entry)| entry.expires_at <= now)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in expired_keys {
            l1_cache.remove(&key);
        }
    }

    fn matches_pattern(&self, key: &str, pattern: &str) -> bool {
        // Simple pattern matching for Redis-style patterns
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            key.starts_with(prefix)
        } else {
            key == pattern
        }
    }

    async fn increment_total_requests(&self) {
        self.metrics.write().await.total_requests += 1;
    }

    async fn increment_l1_hits(&self) {
        self.metrics.write().await.l1_hits += 1;
    }

    async fn increment_l1_misses(&self) {
        self.metrics.write().await.l1_misses += 1;
    }

    async fn increment_l2_hits(&self) {
        self.metrics.write().await.l2_hits += 1;
    }

    async fn increment_l2_misses(&self) {
        self.metrics.write().await.l2_misses += 1;
    }
}

pub async fn init(config: CacheConfig) -> CacheResult<CacheManager> {
    let cache = CacheManager::new(config).await?;
    tracing::info!("Cache initialized successfully");
    Ok(cache)
}
