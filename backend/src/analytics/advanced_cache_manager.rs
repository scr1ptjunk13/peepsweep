use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{debug, error, info, warn};

use crate::analytics::pnl_calculator::PnLResult;
use crate::analytics::data_models::PerformanceMetrics;
use crate::analytics::data_aggregation_engine::AggregationResult;
use crate::risk_management::RiskError;
use uuid::Uuid as UserId;

/// Advanced multi-tier cache manager for analytics data
#[derive(Debug)]
pub struct AdvancedCacheManager {
    /// L1 Cache: In-memory hot data
    l1_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// L2 Cache: Compressed warm data
    l2_cache: Arc<RwLock<HashMap<String, CompressedCacheEntry>>>,
    /// Cache configuration
    config: CacheConfig,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Cache eviction policy
    eviction_policy: EvictionPolicy,
}

/// Cache configuration parameters
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub l1_max_entries: usize,
    pub l2_max_entries: usize,
    pub l1_ttl_seconds: u64,
    pub l2_ttl_seconds: u64,
    pub compression_threshold_bytes: usize,
    pub eviction_batch_size: usize,
    pub background_cleanup_interval_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_max_entries: 10000,
            l2_max_entries: 50000,
            l1_ttl_seconds: 300,      // 5 minutes
            l2_ttl_seconds: 3600,     // 1 hour
            compression_threshold_bytes: 1024,
            eviction_batch_size: 100,
            background_cleanup_interval_seconds: 60,
        }
    }
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub key: String,
    pub data: CacheData,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
    pub size_bytes: usize,
    pub priority: CachePriority,
}

/// Compressed cache entry for L2 storage
#[derive(Debug, Clone)]
pub struct CompressedCacheEntry {
    pub key: String,
    pub compressed_data: Vec<u8>,
    pub original_size: usize,
    pub compression_ratio: f64,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
}

/// Cache data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheData {
    PnLResult(PnLResult),
    AggregationResult(AggregationResult),
    PerformanceMetrics(PerformanceMetrics),
    UserAnalytics(UserAnalyticsData),
    BenchmarkData(BenchmarkCacheData),
    TimeSeriesData(TimeSeriesCache),
}

/// User analytics data for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnalyticsData {
    pub user_id: UserId,
    pub portfolio_summary: PortfolioSummary,
    pub trading_metrics: TradingMetrics,
    pub risk_metrics: RiskMetrics,
    pub performance_history: Vec<PerformanceSnapshot>,
}

/// Portfolio summary for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub total_value: Decimal,
    pub total_pnl: Decimal,
    pub asset_allocation: HashMap<String, Decimal>,
    pub position_count: u32,
}

/// Trading metrics for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMetrics {
    pub total_trades: u64,
    pub win_rate: Decimal,
    pub average_trade_size: Decimal,
    pub total_volume: Decimal,
    pub fees_paid: Decimal,
}

/// Risk metrics for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub volatility: Decimal,
    pub var_95: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: Decimal,
    pub beta: Decimal,
}

/// Performance snapshot for time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub portfolio_value: Decimal,
    pub pnl: Decimal,
    pub return_pct: Decimal,
}

/// Benchmark data for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCacheData {
    pub benchmark_id: String,
    pub values: Vec<BenchmarkValue>,
    pub metadata: BenchmarkMetadata,
}

/// Benchmark value point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkValue {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub volume: Option<Decimal>,
}

/// Benchmark metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetadata {
    pub name: String,
    pub category: String,
    pub update_frequency: String,
}

/// Time series cache data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesCache {
    pub series_id: String,
    pub data_points: Vec<TimeSeriesPoint>,
    pub aggregation_level: AggregationLevel,
}

/// Time series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: Decimal,
    pub metadata: Option<HashMap<String, String>>,
}

/// Aggregation level for time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationLevel {
    Raw,
    Minute,
    Hour,
    Day,
    Week,
    Month,
}

/// Cache priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CachePriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Cache eviction policies
#[derive(Debug, Clone)]
pub enum EvictionPolicy {
    LRU,          // Least Recently Used
    LFU,          // Least Frequently Used
    TTL,          // Time To Live
    Adaptive,     // Adaptive based on access patterns
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub evictions: u64,
    pub compressions: u64,
    pub total_requests: u64,
    pub average_response_time_ms: f64,
    pub memory_usage_bytes: usize,
    pub compression_ratio: f64,
    pub last_cleanup: DateTime<Utc>,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            l1_hits: 0,
            l1_misses: 0,
            l2_hits: 0,
            l2_misses: 0,
            evictions: 0,
            compressions: 0,
            total_requests: 0,
            average_response_time_ms: 0.0,
            memory_usage_bytes: 0,
            compression_ratio: 0.0,
            last_cleanup: Utc::now(),
        }
    }
}

/// Cache operation result
#[derive(Debug)]
pub enum CacheResult<T> {
    L1Hit(T),
    L2Hit(T),
    Miss,
}

impl AdvancedCacheManager {
    /// Create a new advanced cache manager
    pub fn new(config: CacheConfig, eviction_policy: EvictionPolicy) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            eviction_policy,
        }
    }

    /// Get data from cache with multi-tier lookup
    pub async fn get<T>(&self, key: &str) -> CacheResult<T> 
    where 
        T: Clone + for<'de> Deserialize<'de>,
        CacheData: TryInto<T>,
    {
        let start_time = std::time::Instant::now();
        
        // Try L1 cache first
        if let Some(entry) = self.get_from_l1(key).await {
            self.update_access_stats(&entry.key, true, false, start_time.elapsed().as_millis() as f64).await;
            if let Ok(data) = entry.data.try_into() {
                return CacheResult::L1Hit(data);
            }
        }

        // Try L2 cache
        if let Some(entry) = self.get_from_l2(key).await {
            // Decompress and promote to L1 if frequently accessed
            if let Ok(decompressed) = self.decompress_entry(&entry).await {
                if entry.access_count > 5 {
                    self.promote_to_l1(key.to_string(), decompressed.clone()).await;
                }
                self.update_access_stats(key, false, true, start_time.elapsed().as_millis() as f64).await;
                if let Ok(data) = decompressed.try_into() {
                    return CacheResult::L2Hit(data);
                }
            }
        }

        // Cache miss
        self.update_access_stats(key, false, false, start_time.elapsed().as_millis() as f64).await;
        CacheResult::Miss
    }

    /// Store data in cache with intelligent tier placement
    pub async fn put(&self, key: String, data: CacheData, priority: CachePriority) -> Result<(), RiskError> {
        let size_bytes = self.estimate_size(&data);
        
        // Determine cache tier based on size and priority
        if size_bytes < self.config.compression_threshold_bytes || priority >= CachePriority::High {
            self.put_l1(key, data, priority, size_bytes).await
        } else {
            self.put_l2(key, data, size_bytes).await
        }
    }

    /// Invalidate cache entry
    pub async fn invalidate(&self, key: &str) -> bool {
        let l1_removed = {
            let mut l1 = self.l1_cache.write().await;
            l1.remove(key).is_some()
        };

        let l2_removed = {
            let mut l2 = self.l2_cache.write().await;
            l2.remove(key).is_some()
        };

        l1_removed || l2_removed
    }

    /// Clear all cache data
    pub async fn clear(&self) {
        {
            let mut l1 = self.l1_cache.write().await;
            l1.clear();
        }
        {
            let mut l2 = self.l2_cache.write().await;
            l2.clear();
        }
        
        let mut stats = self.stats.write().await;
        *stats = CacheStats::default();
        
        info!("Cache cleared");
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        (*self.stats.read().await).clone()
    }

    /// Perform background cleanup and optimization
    pub async fn cleanup(&self) -> Result<(), RiskError> {
        info!("Starting cache cleanup");
        
        let now = Utc::now();
        let mut evicted_count = 0;

        // Clean L1 cache
        {
            let mut l1 = self.l1_cache.write().await;
            let keys_to_remove: Vec<String> = l1.iter()
                .filter(|(_, entry)| {
                    let age = (now - entry.created_at).num_seconds() as u64;
                    age > self.config.l1_ttl_seconds
                })
                .map(|(key, _)| key.clone())
                .collect();

            for key in keys_to_remove {
                l1.remove(&key);
                evicted_count += 1;
            }
        }

        // Clean L2 cache
        {
            let mut l2 = self.l2_cache.write().await;
            let keys_to_remove: Vec<String> = l2.iter()
                .filter(|(_, entry)| {
                    let age = (now - entry.created_at).num_seconds() as u64;
                    age > self.config.l2_ttl_seconds
                })
                .map(|(key, _)| key.clone())
                .collect();

            for key in keys_to_remove {
                l2.remove(&key);
                evicted_count += 1;
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.evictions += evicted_count;
            stats.last_cleanup = now;
        }

        info!("Cache cleanup completed, evicted {} entries", evicted_count);
        Ok(())
    }

    /// Optimize cache based on access patterns
    pub async fn optimize(&self) -> Result<(), RiskError> {
        info!("Starting cache optimization");

        // Analyze access patterns and promote frequently accessed L2 entries
        let candidates_for_promotion = {
            let l2 = self.l2_cache.read().await;
            l2.iter()
                .filter(|(_, entry)| entry.access_count > 10)
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect::<Vec<_>>()
        };

        for (key, l2_entry) in candidates_for_promotion {
            if let Ok(decompressed) = self.decompress_entry(&l2_entry).await {
                self.promote_to_l1(key, decompressed).await;
            }
        }

        // Compress large L1 entries that are infrequently accessed
        let candidates_for_compression = {
            let l1 = self.l1_cache.read().await;
            l1.iter()
                .filter(|(_, entry)| {
                    entry.size_bytes > self.config.compression_threshold_bytes && 
                    entry.access_count < 5 &&
                    entry.priority < CachePriority::High
                })
                .map(|(key, entry)| (key.clone(), entry.clone()))
                .collect::<Vec<_>>()
        };

        for (key, l1_entry) in candidates_for_compression {
            self.demote_to_l2(key, l1_entry).await;
        }

        info!("Cache optimization completed");
        Ok(())
    }

    /// Private helper methods

    async fn get_from_l1(&self, key: &str) -> Option<CacheEntry> {
        let mut l1 = self.l1_cache.write().await;
        if let Some(entry) = l1.get_mut(key) {
            entry.last_accessed = Utc::now();
            entry.access_count += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    async fn get_from_l2(&self, key: &str) -> Option<CompressedCacheEntry> {
        let mut l2 = self.l2_cache.write().await;
        if let Some(entry) = l2.get_mut(key) {
            entry.last_accessed = Utc::now();
            entry.access_count += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    async fn put_l1(&self, key: String, data: CacheData, priority: CachePriority, size_bytes: usize) -> Result<(), RiskError> {
        let mut l1 = self.l1_cache.write().await;
        
        // Check if eviction is needed
        if l1.len() >= self.config.l1_max_entries {
            self.evict_l1_entries(&mut l1).await;
        }

        let entry = CacheEntry {
            key: key.clone(),
            data,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 1,
            size_bytes,
            priority,
        };

        l1.insert(key, entry);
        Ok(())
    }

    async fn put_l2(&self, key: String, data: CacheData, size_bytes: usize) -> Result<(), RiskError> {
        let compressed_data = self.compress_data(&data).await?;
        let compression_ratio = compressed_data.len() as f64 / size_bytes as f64;

        let mut l2 = self.l2_cache.write().await;
        
        // Check if eviction is needed
        if l2.len() >= self.config.l2_max_entries {
            self.evict_l2_entries(&mut l2).await;
        }

        let entry = CompressedCacheEntry {
            key: key.clone(),
            compressed_data,
            original_size: size_bytes,
            compression_ratio,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 1,
        };

        l2.insert(key, entry);

        // Update compression stats
        {
            let mut stats = self.stats.write().await;
            stats.compressions += 1;
            stats.compression_ratio = (stats.compression_ratio + compression_ratio) / 2.0;
        }

        Ok(())
    }

    async fn promote_to_l1(&self, key: String, data: CacheData) {
        let size_bytes = self.estimate_size(&data);
        let _ = self.put_l1(key.clone(), data, CachePriority::Medium, size_bytes).await;
        
        // Remove from L2
        let mut l2 = self.l2_cache.write().await;
        l2.remove(&key);
    }

    async fn demote_to_l2(&self, key: String, entry: CacheEntry) {
        let _ = self.put_l2(key.clone(), entry.data, entry.size_bytes).await;
        
        // Remove from L1
        let mut l1 = self.l1_cache.write().await;
        l1.remove(&key);
    }

    async fn evict_l1_entries(&self, l1_cache: &mut HashMap<String, CacheEntry>) {
        let keys_to_remove: Vec<String> = match self.eviction_policy {
            EvictionPolicy::LRU => {
                let mut entries: Vec<_> = l1_cache.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.last_accessed);
                entries.iter().take(self.config.eviction_batch_size).map(|(key, _)| key.to_string()).collect()
            }
            EvictionPolicy::LFU => {
                let mut entries: Vec<_> = l1_cache.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.access_count);
                entries.iter().take(self.config.eviction_batch_size).map(|(key, _)| key.to_string()).collect()
            }
            _ => {
                // Default to LRU
                let mut entries: Vec<_> = l1_cache.iter().collect();
                entries.sort_by_key(|(_, entry)| entry.last_accessed);
                entries.iter().take(self.config.eviction_batch_size).map(|(key, _)| key.to_string()).collect()
            }
        };
        
        for key in keys_to_remove {
            l1_cache.remove(&key);
        }
    }

    async fn evict_l2_entries(&self, l2_cache: &mut HashMap<String, CompressedCacheEntry>) {
        let mut entries: Vec<_> = l2_cache.iter().collect();
        entries.sort_by_key(|(_, entry)| entry.last_accessed);
        
        let keys_to_remove: Vec<String> = entries.iter().take(self.config.eviction_batch_size).map(|(key, _)| key.to_string()).collect();
        
        for key in keys_to_remove {
            l2_cache.remove(&key);
        }
    }

    async fn compress_data(&self, data: &CacheData) -> Result<Vec<u8>, RiskError> {
        // Simulate compression (in real implementation, use actual compression library)
        let serialized = serde_json::to_vec(data)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        
        // Mock compression - reduce size by 30%
        let compressed_size = (serialized.len() as f64 * 0.7) as usize;
        Ok(vec![0u8; compressed_size])
    }

    async fn decompress_entry(&self, entry: &CompressedCacheEntry) -> Result<CacheData, RiskError> {
        // Simulate decompression (mock implementation)
        // In real implementation, decompress the actual data
        Ok(CacheData::UserAnalytics(UserAnalyticsData {
            user_id: Uuid::new_v4(),
            portfolio_summary: PortfolioSummary {
                total_value: Decimal::from(100000),
                total_pnl: Decimal::from(5000),
                asset_allocation: HashMap::new(),
                position_count: 10,
            },
            trading_metrics: TradingMetrics {
                total_trades: 100,
                win_rate: Decimal::new(655, 1),
                average_trade_size: Decimal::from(1000),
                total_volume: Decimal::from(100000),
                fees_paid: Decimal::from(500),
            },
            risk_metrics: RiskMetrics {
                volatility: Decimal::new(152, 1),
                var_95: Decimal::from(-2500),
                max_drawdown: Decimal::new(-85, 1),
                sharpe_ratio: Decimal::new(145, 2),
                beta: Decimal::new(115, 2),
            },
            performance_history: Vec::new(),
        }))
    }

    fn estimate_size(&self, data: &CacheData) -> usize {
        // Estimate serialized size (simplified)
        match data {
            CacheData::PnLResult(_) => 1024,
            CacheData::AggregationResult(_) => 4096,
            CacheData::PerformanceMetrics(_) => 512,
            CacheData::UserAnalytics(_) => 2048,
            CacheData::BenchmarkData(_) => 8192,
            CacheData::TimeSeriesData(_) => 16384,
        }
    }

    async fn update_access_stats(&self, _key: &str, l1_hit: bool, l2_hit: bool, response_time_ms: f64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        if l1_hit {
            stats.l1_hits += 1;
        } else if l2_hit {
            stats.l2_hits += 1;
            stats.l1_misses += 1;
        } else {
            stats.l1_misses += 1;
            stats.l2_misses += 1;
        }

        // Update rolling average response time
        let total_time = stats.average_response_time_ms * (stats.total_requests - 1) as f64;
        stats.average_response_time_ms = (total_time + response_time_ms) / stats.total_requests as f64;
    }
}

// Implement TryInto for CacheData conversions
impl TryInto<PnLResult> for CacheData {
    type Error = ();
    
    fn try_into(self) -> Result<PnLResult, Self::Error> {
        match self {
            CacheData::PnLResult(data) => Ok(data),
            _ => Err(()),
        }
    }
}

impl TryInto<AggregationResult> for CacheData {
    type Error = ();
    
    fn try_into(self) -> Result<AggregationResult, Self::Error> {
        match self {
            CacheData::AggregationResult(data) => Ok(data),
            _ => Err(()),
        }
    }
}

impl TryInto<UserAnalyticsData> for CacheData {
    type Error = ();
    
    fn try_into(self) -> Result<UserAnalyticsData, Self::Error> {
        match self {
            CacheData::UserAnalytics(data) => Ok(data),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_put_get() {
        let cache = AdvancedCacheManager::new(CacheConfig::default(), EvictionPolicy::LRU);
        
        let data = CacheData::UserAnalytics(UserAnalyticsData {
            user_id: Uuid::new_v4(),
            portfolio_summary: PortfolioSummary {
                total_value: Decimal::from(100000),
                total_pnl: Decimal::from(5000),
                asset_allocation: HashMap::new(),
                position_count: 10,
            },
            trading_metrics: TradingMetrics {
                total_trades: 100,
                win_rate: Decimal::new(655, 1),
                average_trade_size: Decimal::from(1000),
                total_volume: Decimal::from(100000),
                fees_paid: Decimal::from(500),
            },
            risk_metrics: RiskMetrics {
                volatility: Decimal::new(152, 1),
                var_95: Decimal::from(-2500),
                max_drawdown: Decimal::new(-85, 1),
                sharpe_ratio: Decimal::new(145, 2),
                beta: Decimal::new(115, 2),
            },
            performance_history: Vec::new(),
        });

        let key = "test_user_analytics".to_string();
        cache.put(key.clone(), data, CachePriority::High).await.unwrap();

        let result: CacheResult<UserAnalyticsData> = cache.get(&key).await;
        assert!(matches!(result, CacheResult::L1Hit(_)));
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let mut config = CacheConfig::default();
        config.l1_max_entries = 2;
        
        let cache = AdvancedCacheManager::new(config, EvictionPolicy::LRU);
        
        // Fill cache beyond capacity
        for i in 0..5 {
            let data = CacheData::UserAnalytics(UserAnalyticsData {
                user_id: Uuid::new_v4(),
                portfolio_summary: PortfolioSummary {
                    total_value: Decimal::from(100000),
                    total_pnl: Decimal::from(5000),
                    asset_allocation: HashMap::new(),
                    position_count: 10,
                },
                trading_metrics: TradingMetrics {
                    total_trades: 100,
                    win_rate: Decimal::new(655, 1),
                    average_trade_size: Decimal::from(1000),
                    total_volume: Decimal::from(100000),
                    fees_paid: Decimal::from(500),
                },
                risk_metrics: RiskMetrics {
                    volatility: Decimal::new(152, 1),
                    var_95: Decimal::from(-2500),
                    max_drawdown: Decimal::new(-85, 1),
                    sharpe_ratio: Decimal::new(145, 2),
                    beta: Decimal::new(115, 2),
                },
                performance_history: Vec::new(),
            });
            
            cache.put(format!("key_{}", i), data, CachePriority::Medium).await.unwrap();
        }

        let stats = cache.get_stats().await;
        assert!(stats.evictions > 0);
    }
}
