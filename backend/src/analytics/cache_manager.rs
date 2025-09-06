use crate::analytics::data_models::*;
use crate::risk_management::RiskError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Redis interface trait for dependency injection
pub trait RedisInterface: Send + Sync + Clone {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, RiskError>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), RiskError>;
    async fn del(&self, key: &str) -> Result<(), RiskError>;
    async fn exists(&self, key: &str) -> Result<bool, RiskError>;
}

/// Multi-layer cache manager for analytics data
#[derive(Debug)]
pub struct AnalyticsCacheManager<R: RedisInterface> {
    redis_cache: Arc<RedisCache<R>>,
    memory_cache: Arc<MemoryCache>,
    cache_policies: Arc<RwLock<HashMap<CacheKeyType, CachePolicy>>>,
    cache_stats: Arc<RwLock<CacheStats>>,
}

/// Redis cache layer for persistent caching
#[derive(Debug)]
pub struct RedisCache<R: RedisInterface> {
    connection: R,
    key_prefix: String,
}

/// In-memory cache layer for hot data
#[derive(Debug)]
pub struct MemoryCache {
    cache_data: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_size: usize,
    cleanup_interval: Duration,
}

/// Cache entry with TTL and metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub data: Vec<u8>,
    pub created_at: Instant,
    pub ttl: Duration,
    pub access_count: u64,
    pub last_accessed: Instant,
    pub size_bytes: usize,
}

/// Cache policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePolicy {
    pub cache_type: CacheType,
    pub ttl_seconds: u64,
    pub max_size_mb: Option<u64>,
    pub eviction_policy: EvictionPolicy,
    pub compression_enabled: bool,
    pub replication_enabled: bool,
}

/// Cache type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheType {
    Memory,
    Redis,
    Both,
}

/// Eviction policy enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LRU,
    LFU,
    TTL,
    FIFO,
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub memory_hits: u64,
    pub memory_misses: u64,
    pub redis_hits: u64,
    pub redis_misses: u64,
    pub total_requests: u64,
    pub hit_rate: f64,
    pub memory_usage_bytes: u64,
    pub redis_usage_bytes: u64,
    pub evictions: u64,
    pub errors: u64,
    pub average_response_time_ms: f64,
}

impl<R: RedisInterface> AnalyticsCacheManager<R> {
    pub async fn new(redis_connection: R, config: CacheConfig) -> Result<Self, RiskError> {
        let redis_cache = Arc::new(RedisCache::new(redis_connection, "analytics".to_string()).await?);
        let memory_cache = Arc::new(MemoryCache::new(1000, Duration::from_secs(300)).await?);
        
        let mut cache_policies = HashMap::new();
        
        // Set default cache policies
        cache_policies.insert(CacheKeyType::PnLData, CachePolicy {
            cache_type: CacheType::Both,
            ttl: Duration::from_secs(300), // 5 minutes
            max_size: Some(10000),
            compression: false,
            replication: true,
        });
        
        cache_policies.insert(CacheKeyType::PerformanceMetrics, CachePolicy {
            cache_type: CacheType::Both,
            ttl: Duration::from_secs(600), // 10 minutes
            max_size: Some(5000),
            compression: true,
            replication: true,
        });
        
        cache_policies.insert(CacheKeyType::GasUsageData, CachePolicy {
            cache_type: CacheType::Redis,
            ttl: Duration::from_secs(3600), // 1 hour
            max_size: Some(50000),
            compression: true,
            replication: false,
        });
        
        Ok(Self {
            redis_cache,
            memory_cache,
            cache_policies: Arc::new(RwLock::new(cache_policies)),
            cache_stats: Arc::new(RwLock::new(CacheStats::default())),
        })
    }

    /// Get data from cache with fallback strategy
    pub async fn get<T>(&self, key: &CacheKey) -> Result<Option<T>, RiskError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let start_time = Instant::now();
        let key_str = key.to_string();
        
        // Try memory cache first
        if let Some(policy) = self.get_cache_policy(&key.key_type) {
            if policy.cache_type == CacheType::Memory || policy.cache_type == CacheType::Both {
                if let Some(data) = self.memory_cache.get(&key_str).await? {
                    self.update_stats_hit("memory", start_time).await;
                    let deserialized: T = serde_json::from_slice(&data)
                        .map_err(|e| RiskError::SerializationError(e.to_string()))?;
                    return Ok(Some(deserialized));
                }
            }
        }

        // Try Redis cache
        if let Some(policy) = self.get_cache_policy(&key.key_type) {
            if policy.cache_type == CacheType::Redis || policy.cache_type == CacheType::Both {
                if let Some(data) = self.redis_cache.get(&key_str).await? {
                    // Store in memory cache for faster future access
                    if policy.cache_type == CacheType::Both {
                        let _ = self.memory_cache.set(&key_str, &data, Duration::from_secs(policy.ttl_seconds)).await;
                    }
                    
                    self.update_stats_hit("redis", start_time).await;
                    let deserialized: T = serde_json::from_slice(&data)
                        .map_err(|e| RiskError::SerializationError(e.to_string()))?;
                    return Ok(Some(deserialized));
                }
            }
        }

        self.update_stats_miss(start_time).await;
        Ok(None)
    }

    /// Set data in cache according to policy
    pub async fn set<T>(&self, key: &CacheKey, value: &T) -> Result<(), RiskError>
    where
        T: Serialize,
    {
        let key_str = key.to_string();
        let serialized = serde_json::to_vec(value)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;

        if let Some(policy) = self.get_cache_policy(&key.key_type) {
            let ttl = Duration::from_secs(policy.ttl_seconds);

            // Set in memory cache
            if policy.cache_type == CacheType::Memory || policy.cache_type == CacheType::Both {
                self.memory_cache.set(&key_str, &serialized, ttl).await?;
            }

            // Set in Redis cache
            if policy.cache_type == CacheType::Redis || policy.cache_type == CacheType::Both {
                self.redis_cache.set(&key_str, &serialized, ttl).await?;
            }
        }

        Ok(())
    }

    /// Delete data from cache
    pub async fn delete(&self, key: &CacheKey) -> Result<(), RiskError> {
        let key_str = key.to_string();

        // Delete from memory cache
        self.memory_cache.delete(&key_str).await?;

        // Delete from Redis cache
        self.redis_cache.delete(&key_str).await?;

        Ok(())
    }

    /// Clear all cache data
    pub async fn clear_all(&self) -> Result<(), RiskError> {
        self.memory_cache.clear().await?;
        self.redis_cache.clear().await?;
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.cache_stats.read().await.clone()
    }

    /// Get cache policy for key type
    fn get_cache_policy(&self, key_type: &CacheKeyType) -> Option<&CachePolicy> {
        let key = format!("{:?}", key_type);
        self.cache_policies.get(&key)
    }

    /// Default cache policies for different data types
    fn default_cache_policies() -> HashMap<String, CachePolicy> {
        let mut policies = HashMap::new();

        // P&L data - hot data, cache in both layers
        policies.insert("PnLData".to_string(), CachePolicy {
            cache_type: CacheType::Both,
            ttl_seconds: 300, // 5 minutes
            max_size_mb: Some(100),
            eviction_policy: EvictionPolicy::LRU,
            compression_enabled: false,
            replication_enabled: true,
        });

        // Performance metrics - less frequent updates
        policies.insert("PerformanceMetrics".to_string(), CachePolicy {
            cache_type: CacheType::Redis,
            ttl_seconds: 3600, // 1 hour
            max_size_mb: Some(50),
            eviction_policy: EvictionPolicy::TTL,
            compression_enabled: true,
            replication_enabled: false,
        });

        // Gas optimization reports - daily updates
        policies.insert("GasOptimizationReport".to_string(), CachePolicy {
            cache_type: CacheType::Redis,
            ttl_seconds: 86400, // 24 hours
            max_size_mb: Some(25),
            eviction_policy: EvictionPolicy::TTL,
            compression_enabled: true,
            replication_enabled: false,
        });

        // Trade history - frequently accessed
        policies.insert("TradeHistory".to_string(), CachePolicy {
            cache_type: CacheType::Both,
            ttl_seconds: 1800, // 30 minutes
            max_size_mb: Some(200),
            eviction_policy: EvictionPolicy::LRU,
            compression_enabled: false,
            replication_enabled: true,
        });

        // Price data - very hot data
        policies.insert("PriceData".to_string(), CachePolicy {
            cache_type: CacheType::Memory,
            ttl_seconds: 60, // 1 minute
            max_size_mb: Some(10),
            eviction_policy: EvictionPolicy::TTL,
            compression_enabled: false,
            replication_enabled: false,
        });

        // Benchmark data - infrequent updates
        policies.insert("BenchmarkData".to_string(), CachePolicy {
            cache_type: CacheType::Redis,
            ttl_seconds: 7200, // 2 hours
            max_size_mb: Some(10),
            eviction_policy: EvictionPolicy::TTL,
            compression_enabled: true,
            replication_enabled: false,
        });

        policies
    }

    /// Update cache hit statistics
    async fn update_stats_hit(&self, cache_type: &str, start_time: Instant) {
        let mut stats = self.cache_stats.write().await;
        stats.total_requests += 1;
        
        match cache_type {
            "memory" => stats.memory_hits += 1,
            "redis" => stats.redis_hits += 1,
            _ => {}
        }
        
        let response_time = start_time.elapsed().as_millis() as f64;
        let total_time = stats.average_response_time_ms * (stats.total_requests - 1) as f64;
        stats.average_response_time_ms = (total_time + response_time) / stats.total_requests as f64;
        
        stats.hit_rate = ((stats.memory_hits + stats.redis_hits) as f64) / (stats.total_requests as f64) * 100.0;
    }

    /// Update cache miss statistics
    async fn update_stats_miss(&self, start_time: Instant) {
        let mut stats = self.cache_stats.write().await;
        stats.total_requests += 1;
        stats.memory_misses += 1;
        stats.redis_misses += 1;
        
        let response_time = start_time.elapsed().as_millis() as f64;
        let total_time = stats.average_response_time_ms * (stats.total_requests - 1) as f64;
        stats.average_response_time_ms = (total_time + response_time) / stats.total_requests as f64;
        
        stats.hit_rate = ((stats.memory_hits + stats.redis_hits) as f64) / (stats.total_requests as f64) * 100.0;
    }

    /// Start background cleanup task
    async fn start_cleanup_task(&self) {
        let memory_cache = self.memory_cache.clone();
        let cache_stats = self.cache_stats.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                // Cleanup expired entries
                if let Err(e) = memory_cache.cleanup_expired().await {
                    error!("Failed to cleanup expired cache entries: {}", e);
                }
                
                // Update memory usage stats
                let memory_usage = memory_cache.get_memory_usage().await;
                let mut stats = cache_stats.write().await;
                stats.memory_usage_bytes = memory_usage;
            }
        });
    }
}

impl<R: RedisInterface> RedisCache<R> {
    pub async fn new(connection: R, key_prefix: String) -> Result<Self, RiskError> {
        Ok(Self {
            connection,
            key_prefix,
        })
    }

    /// Get data from Redis
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, RiskError> {
        let full_key = format!("{}:{}", self.key_prefix, key);
        let mut conn = self.connection.as_ref().clone();
        
        let result: Option<Vec<u8>> = redis::cmd("GET")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| RiskError::CacheError(e.to_string()))?;
        
        if result.is_some() {
            debug!("Redis cache hit: {}", key);
        }
        
        Ok(result)
    }

    /// Set data in Redis
    pub async fn set(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), RiskError> {
        let full_key = format!("{}:{}", self.key_prefix, key);
        let mut conn = self.connection.as_ref().clone();
        
        redis::cmd("SETEX")
            .arg(&full_key)
            .arg(ttl.as_secs())
            .arg(value)
            .query_async(&mut conn)
            .await
            .map_err(|e| RiskError::CacheError(e.to_string()))?;
        
        debug!("Redis cache set: {} (TTL: {}s)", key, ttl.as_secs());
        Ok(())
    }

    /// Delete data from Redis
    pub async fn delete(&self, key: &str) -> Result<(), RiskError> {
        let full_key = format!("{}:{}", self.key_prefix, key);
        let mut conn = self.connection.as_ref().clone();
        
        redis::cmd("DEL")
            .arg(&full_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| RiskError::CacheError(e.to_string()))?;
        
        debug!("Redis cache delete: {}", key);
        Ok(())
    }

    /// Clear all data with prefix
    pub async fn clear(&self) -> Result<(), RiskError> {
        let pattern = format!("{}:*", self.key_prefix);
        let mut conn = self.connection.as_ref().clone();
        
        // Get all keys matching pattern
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| RiskError::CacheError(e.to_string()))?;
        
        if !keys.is_empty() {
            // Delete all matching keys
            redis::cmd("DEL")
                .arg(&keys)
                .query_async(&mut conn)
                .await
                .map_err(|e| RiskError::CacheError(e.to_string()))?;
        }
        
        info!("Redis cache cleared: {} keys deleted", keys.len());
        Ok(())
    }
}

impl MemoryCache {
    pub async fn new(max_size: usize, cleanup_interval: Duration) -> Result<Self, RiskError> {
        let cache_data = Arc::new(RwLock::new(HashMap::new()));
        
        let cache = Self {
            cache_data,
            max_size,
            cleanup_interval,
        };

        Ok(cache)
    }

    /// Get data from memory cache
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, RiskError> {
        let mut cache = self.cache_data.write().await;
        
        if let Some(entry) = cache.get_mut(key) {
            // Check if entry is expired
            if entry.created_at.elapsed() > entry.ttl {
                cache.remove(key);
                return Ok(None);
            }
            
            // Update access statistics
            entry.access_count += 1;
            entry.last_accessed = Instant::now();
            
            debug!("Memory cache hit: {}", key);
            return Ok(Some(entry.data.clone()));
        }
        
        Ok(None)
    }

    /// Set data in memory cache
    pub async fn set(&self, key: &str, value: &[u8], ttl: Duration) -> Result<(), RiskError> {
        let mut cache = self.cache_data.write().await;
        
        // Check if we need to evict entries
        if cache.len() >= self.max_size {
            self.evict_lru(&mut cache).await;
        }
        
        let entry = CacheEntry {
            data: value.to_vec(),
            created_at: Instant::now(),
            ttl,
            access_count: 1,
            last_accessed: Instant::now(),
            size_bytes: value.len(),
        };
        
        cache.insert(key.to_string(), entry);
        debug!("Memory cache set: {} ({} bytes, TTL: {}s)", key, value.len(), ttl.as_secs());
        
        Ok(())
    }

    /// Delete data from memory cache
    pub async fn delete(&self, key: &str) -> Result<(), RiskError> {
        let mut cache = self.cache_data.write().await;
        cache.remove(key);
        debug!("Memory cache delete: {}", key);
        Ok(())
    }

    /// Clear all data from memory cache
    pub async fn clear(&self) -> Result<(), RiskError> {
        let mut cache = self.cache_data.write().await;
        let count = cache.len();
        cache.clear();
        info!("Memory cache cleared: {} entries removed", count);
        Ok(())
    }

    /// Cleanup expired entries
    pub async fn cleanup_expired(&self) -> Result<(), RiskError> {
        let mut cache = self.cache_data.write().await;
        let now = Instant::now();
        
        let expired_keys: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.created_at) > entry.ttl)
            .map(|(key, _)| key.clone())
            .collect();
        
        for key in &expired_keys {
            cache.remove(key);
        }
        
        if !expired_keys.is_empty() {
            debug!("Memory cache cleanup: {} expired entries removed", expired_keys.len());
        }
        
        Ok(())
    }

    /// Get current memory usage
    pub async fn get_memory_usage(&self) -> u64 {
        let cache = self.cache_data.read().await;
        cache.values().map(|entry| entry.size_bytes as u64).sum()
    }

    /// Evict least recently used entry
    async fn evict_lru(&self, cache: &mut HashMap<String, CacheEntry>) {
        if let Some((lru_key, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(key, entry)| (key.clone(), entry.clone()))
        {
            cache.remove(&lru_key);
            debug!("Memory cache evicted LRU entry: {}", lru_key);
        }
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            memory_hits: 0,
            memory_misses: 0,
            redis_hits: 0,
            redis_misses: 0,
            total_requests: 0,
            hit_rate: 0.0,
            memory_usage_bytes: 0,
            redis_usage_bytes: 0,
            evictions: 0,
            errors: 0,
            average_response_time_ms: 0.0,
        }
    }
}
