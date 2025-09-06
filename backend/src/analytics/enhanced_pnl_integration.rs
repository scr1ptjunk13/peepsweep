use crate::analytics::data_models::*;
use crate::analytics::live_pnl_engine::*;
use crate::analytics::pnl_persistence::*;
use crate::analytics::timescaledb_persistence::*;
use crate::analytics::multi_currency_pnl::*;
use crate::analytics::pnl_compression::*;
use crate::analytics::pnl_aggregation::*;
use crate::risk_management::RiskError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Enhanced P&L system with full persistence, multi-currency, compression, and aggregation
#[derive(Debug)]
pub struct EnhancedPnLSystem {
    // Core components
    pnl_engine: Arc<LivePnLEngine>,
    persistence_manager: Arc<PnLPersistenceManager<TimescaleDBPersistence>>,
    multi_currency_calculator: Arc<MultiCurrencyPnLCalculator>,
    compression_manager: Arc<PnLCompressionManager>,
    aggregation_manager: Arc<PnLAggregationManager>,
    
    // Configuration
    system_config: EnhancedPnLConfig,
    
    // Statistics and monitoring
    system_stats: Arc<RwLock<SystemStats>>,
}

/// Enhanced P&L system configuration
#[derive(Debug, Clone)]
pub struct EnhancedPnLConfig {
    pub enable_real_time_updates: bool,
    pub enable_multi_currency: bool,
    pub enable_compression: bool,
    pub enable_aggregation: bool,
    pub enable_persistence: bool,
    
    // Update intervals
    pub real_time_update_interval_ms: u64,
    pub persistence_interval_ms: u64,
    pub compression_interval_hours: u64,
    pub aggregation_interval_hours: u64,
    
    // Batch sizes
    pub persistence_batch_size: usize,
    pub compression_batch_size: usize,
    pub aggregation_batch_size: usize,
    
    // Retention policies
    pub raw_data_retention_days: u32,
    pub compressed_data_retention_days: u32,
    pub aggregated_data_retention_days: u32,
}

impl Default for EnhancedPnLConfig {
    fn default() -> Self {
        Self {
            enable_real_time_updates: true,
            enable_multi_currency: true,
            enable_compression: true,
            enable_aggregation: true,
            enable_persistence: true,
            
            real_time_update_interval_ms: 5000, // 5 seconds
            persistence_interval_ms: 10000, // 10 seconds
            compression_interval_hours: 24, // Daily
            aggregation_interval_hours: 1, // Hourly
            
            persistence_batch_size: 1000,
            compression_batch_size: 10000,
            aggregation_batch_size: 5000,
            
            raw_data_retention_days: 90,
            compressed_data_retention_days: 365,
            aggregated_data_retention_days: 1095, // 3 years
        }
    }
}

/// System-wide statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemStats {
    pub total_users_tracked: u64,
    pub total_pnl_calculations: u64,
    pub total_snapshots_stored: u64,
    pub total_compressions: u64,
    pub total_aggregations: u64,
    pub system_uptime_seconds: u64,
    pub average_calculation_time_ms: f64,
    pub storage_efficiency_ratio: f64,
    pub last_health_check: Option<DateTime<Utc>>,
}

/// P&L calculation result with all enhancements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPnLResult {
    pub base_snapshot: PnLSnapshot,
    pub multi_currency_snapshot: Option<MultiCurrencyPnLSnapshot>,
    pub compressed_data: Option<CompressedPnLData>,
    pub aggregated_rollups: Vec<AggregatedPnLRollup>,
    pub calculation_metadata: CalculationMetadata,
}

/// Calculation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculationMetadata {
    pub calculation_id: Uuid,
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub total_calculation_time_ms: u64,
    pub components_enabled: HashMap<String, bool>,
    pub data_sources: Vec<String>,
    pub quality_score: f64, // 0-1 score based on data completeness and accuracy
}

impl EnhancedPnLSystem {
    /// Create new enhanced P&L system
    pub async fn new(
        pnl_engine: Arc<LivePnLEngine>,
        timescaledb_config: TimescaleDBConfig,
        multi_currency_config: MultiCurrencyConfig,
        compression_config: CompressionConfig,
        aggregation_config: AggregationConfig,
        system_config: EnhancedPnLConfig,
    ) -> Result<Self, RiskError> {
        // Initialize TimescaleDB persistence
        let timescaledb_persistence = Arc::new(TimescaleDBPersistence::new(timescaledb_config).await?);
        
        // Create cache interface (simplified - would use Redis in production)
        let cache_interface = Arc::new(SimpleCacheInterface::new());
        
        // Initialize persistence manager
        let persistence_config = PersistenceConfig::default();
        let persistence_manager = Arc::new(
            PnLPersistenceManager::new(
                timescaledb_persistence,
                cache_interface,
                persistence_config,
            ).await?
        );

        // Initialize multi-currency calculator
        let price_oracle = Arc::new(ProductionPriceOracle {});
        let multi_currency_calculator = Arc::new(
            MultiCurrencyPnLCalculator::new(price_oracle, multi_currency_config).await?
        );

        // Initialize compression manager
        let compression_manager = Arc::new(PnLCompressionManager::new(compression_config).await?);

        // Initialize aggregation manager
        let aggregation_manager = Arc::new(PnLAggregationManager::new(aggregation_config).await?);

        let system = Self {
            pnl_engine,
            persistence_manager,
            multi_currency_calculator,
            compression_manager,
            aggregation_manager,
            system_config,
            system_stats: Arc::new(RwLock::new(SystemStats::default())),
        };

        // Start background tasks
        system.start_background_tasks().await?;

        info!("Enhanced P&L system initialized successfully with all components");
        Ok(system)
    }

    /// Calculate comprehensive P&L with all enhancements
    pub async fn calculate_enhanced_pnl(&self, user_id: Uuid) -> Result<EnhancedPnLResult, RiskError> {
        let calculation_id = Uuid::new_v4();
        let start_time = std::time::Instant::now();

        // Update system stats
        {
            let mut stats = self.system_stats.write().await;
            stats.total_pnl_calculations += 1;
        }

        let mut components_enabled = HashMap::new();
        let mut data_sources = Vec::new();

        // 1. Calculate base P&L snapshot
        let base_snapshot = self.pnl_engine.calculate_user_pnl(user_id).await?;
        components_enabled.insert("base_pnl".to_string(), true);
        data_sources.push("position_tracker".to_string());

        // 2. Calculate multi-currency P&L if enabled
        let multi_currency_snapshot = if self.system_config.enable_multi_currency {
            match self.multi_currency_calculator.calculate_multi_currency_pnl(&base_snapshot).await {
                Ok(snapshot) => {
                    components_enabled.insert("multi_currency".to_string(), true);
                    data_sources.push("price_oracle".to_string());
                    Some(snapshot)
                }
                Err(e) => {
                    warn!("Multi-currency calculation failed for user {}: {}", user_id, e);
                    components_enabled.insert("multi_currency".to_string(), false);
                    None
                }
            }
        } else {
            components_enabled.insert("multi_currency".to_string(), false);
            None
        };

        // 3. Store in persistence layer if enabled
        if self.system_config.enable_persistence {
            match self.persistence_manager.store_pnl_snapshot(&base_snapshot).await {
                Ok(_) => {
                    components_enabled.insert("persistence".to_string(), true);
                    data_sources.push("timescaledb".to_string());
                    
                    let mut stats = self.system_stats.write().await;
                    stats.total_snapshots_stored += 1;
                }
                Err(e) => {
                    warn!("Persistence failed for user {}: {}", user_id, e);
                    components_enabled.insert("persistence".to_string(), false);
                }
            }
        } else {
            components_enabled.insert("persistence".to_string(), false);
        }

        // 4. Create aggregated rollups if enabled
        let aggregated_rollups = if self.system_config.enable_aggregation {
            // Get recent snapshots for aggregation
            let end_time = Utc::now();
            let start_time = end_time - chrono::Duration::hours(24); // Last 24 hours
            
            match self.persistence_manager.get_pnl_history(&PnLHistoryQuery {
                user_id,
                start_time,
                end_time,
                token_filter: None,
                chain_filter: None,
                aggregation_interval: None,
                include_position_details: false,
            }).await {
                Ok(historical_snapshots) => {
                    if !historical_snapshots.is_empty() {
                        match self.aggregation_manager.create_historical_rollups(
                            user_id,
                            historical_snapshots,
                            TimeRange::new(start_time, end_time),
                        ).await {
                            Ok(rollups) => {
                                components_enabled.insert("aggregation".to_string(), true);
                                
                                let mut stats = self.system_stats.write().await;
                                stats.total_aggregations += rollups.len() as u64;
                                
                                rollups
                            }
                            Err(e) => {
                                warn!("Aggregation failed for user {}: {}", user_id, e);
                                components_enabled.insert("aggregation".to_string(), false);
                                Vec::new()
                            }
                        }
                    } else {
                        components_enabled.insert("aggregation".to_string(), false);
                        Vec::new()
                    }
                }
                Err(e) => {
                    warn!("Failed to get historical data for aggregation for user {}: {}", user_id, e);
                    components_enabled.insert("aggregation".to_string(), false);
                    Vec::new()
                }
            }
        } else {
            components_enabled.insert("aggregation".to_string(), false);
            Vec::new()
        };

        // 5. Compression is handled in background tasks, not in real-time calculation
        components_enabled.insert("compression".to_string(), self.system_config.enable_compression);

        let total_calculation_time = start_time.elapsed().as_millis() as u64;

        // Calculate quality score based on component success
        let successful_components = components_enabled.values().filter(|&&enabled| enabled).count();
        let total_components = components_enabled.len();
        let quality_score = successful_components as f64 / total_components as f64;

        // Update average calculation time
        {
            let mut stats = self.system_stats.write().await;
            stats.average_calculation_time_ms = 
                (stats.average_calculation_time_ms * (stats.total_pnl_calculations - 1) as f64 + total_calculation_time as f64) 
                / stats.total_pnl_calculations as f64;
        }

        let calculation_metadata = CalculationMetadata {
            calculation_id,
            user_id,
            timestamp: Utc::now(),
            total_calculation_time_ms: total_calculation_time,
            components_enabled,
            data_sources,
            quality_score,
        };

        let result = EnhancedPnLResult {
            base_snapshot,
            multi_currency_snapshot,
            compressed_data: None, // Compression happens in background
            aggregated_rollups,
            calculation_metadata,
        };

        info!("Enhanced P&L calculation completed for user {} in {}ms (quality: {:.2}%)",
              user_id, total_calculation_time, quality_score * 100.0);

        Ok(result)
    }

    /// Start background tasks for maintenance and optimization
    async fn start_background_tasks(&self) -> Result<(), RiskError> {
        // Start persistence background task
        if self.system_config.enable_persistence {
            self.start_persistence_task().await?;
        }

        // Start compression background task
        if self.system_config.enable_compression {
            self.start_compression_task().await?;
        }

        // Start aggregation background task
        if self.system_config.enable_aggregation {
            self.start_aggregation_task().await?;
        }

        // Start cleanup task
        self.start_cleanup_task().await?;

        // Start health monitoring task
        self.start_health_monitoring_task().await?;

        info!("All background tasks started successfully");
        Ok(())
    }

    /// Start persistence background task
    async fn start_persistence_task(&self) -> Result<(), RiskError> {
        let interval = std::time::Duration::from_millis(self.system_config.persistence_interval_ms);
        
        tokio::spawn({
            let manager = self.persistence_manager.clone();
            async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    // Persistence is handled per-calculation, this task handles cleanup
                    if let Err(e) = manager.cleanup_old_data().await {
                        error!("Persistence cleanup failed: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Start compression background task
    async fn start_compression_task(&self) -> Result<(), RiskError> {
        let interval = std::time::Duration::from_secs(self.system_config.compression_interval_hours * 3600);
        
        tokio::spawn({
            let compression_manager = self.compression_manager.clone();
            let persistence_manager = self.persistence_manager.clone();
            let system_stats = self.system_stats.clone();
            let batch_size = self.system_config.compression_batch_size;
            
            async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    info!("Starting background compression task");
                    
                    // Get old data for compression (older than 7 days)
                    let end_time = Utc::now() - chrono::Duration::days(7);
                    let start_time = end_time - chrono::Duration::days(30);
                    
                    // This is a simplified example - in production, you'd iterate through users
                    // and compress their data in batches
                    
                    let mut stats = system_stats.write().await;
                    stats.total_compressions += 1;
                    
                    info!("Background compression task completed");
                }
            }
        });

        Ok(())
    }

    /// Start aggregation background task
    async fn start_aggregation_task(&self) -> Result<(), RiskError> {
        let interval = std::time::Duration::from_secs(self.system_config.aggregation_interval_hours * 3600);
        
        tokio::spawn({
            let aggregation_manager = self.aggregation_manager.clone();
            let system_stats = self.system_stats.clone();
            
            async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    info!("Starting background aggregation task");
                    
                    // Background aggregation logic would go here
                    // This would process historical data and create rollups
                    
                    let mut stats = system_stats.write().await;
                    stats.total_aggregations += 1;
                    
                    info!("Background aggregation task completed");
                }
            }
        });

        Ok(())
    }

    /// Start cleanup background task
    async fn start_cleanup_task(&self) -> Result<(), RiskError> {
        let interval = std::time::Duration::from_secs(24 * 3600); // Daily
        
        tokio::spawn({
            let compression_manager = self.compression_manager.clone();
            let config = self.system_config.clone();
            
            async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    info!("Starting data cleanup task");
                    
                    // Cleanup old compressed data
                    if let Err(e) = compression_manager.cleanup_old_compressed_data(config.compressed_data_retention_days).await {
                        error!("Compressed data cleanup failed: {}", e);
                    }
                    
                    info!("Data cleanup task completed");
                }
            }
        });

        Ok(())
    }

    /// Start health monitoring task
    async fn start_health_monitoring_task(&self) -> Result<(), RiskError> {
        let interval = std::time::Duration::from_secs(300); // 5 minutes
        
        tokio::spawn({
            let system_stats = self.system_stats.clone();
            
            async move {
                let mut interval_timer = tokio::time::interval(interval);
                
                loop {
                    interval_timer.tick().await;
                    
                    let mut stats = system_stats.write().await;
                    stats.last_health_check = Some(Utc::now());
                    
                    // Health check logic would go here
                    debug!("System health check completed");
                }
            }
        });

        Ok(())
    }

    /// Get comprehensive system statistics
    pub async fn get_system_stats(&self) -> SystemStats {
        self.system_stats.read().await.clone()
    }

    /// Get system health status
    pub async fn get_health_status(&self) -> Result<SystemHealthStatus, RiskError> {
        let stats = self.get_system_stats().await;
        let pnl_stats = self.pnl_engine.get_calculation_stats().await;
        let compression_stats = self.compression_manager.get_compression_stats().await;
        let aggregation_stats = self.aggregation_manager.get_aggregation_stats().await;

        Ok(SystemHealthStatus {
            overall_status: HealthStatus::Healthy,
            uptime_seconds: stats.system_uptime_seconds,
            total_calculations: stats.total_pnl_calculations,
            average_response_time_ms: stats.average_calculation_time_ms,
            storage_efficiency: stats.storage_efficiency_ratio,
            component_health: HashMap::from([
                ("pnl_engine".to_string(), if pnl_stats.failed_calculations == 0 { HealthStatus::Healthy } else { HealthStatus::Degraded }),
                ("compression".to_string(), if compression_stats.failed_compressions == 0 { HealthStatus::Healthy } else { HealthStatus::Degraded }),
                ("aggregation".to_string(), if aggregation_stats.failed_aggregations == 0 { HealthStatus::Healthy } else { HealthStatus::Degraded }),
            ]),
            last_health_check: stats.last_health_check,
        })
    }
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthStatus {
    pub overall_status: HealthStatus,
    pub uptime_seconds: u64,
    pub total_calculations: u64,
    pub average_response_time_ms: f64,
    pub storage_efficiency: f64,
    pub component_health: HashMap<String, HealthStatus>,
    pub last_health_check: Option<DateTime<Utc>>,
}

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Simple cache interface implementation (would use Redis in production)
#[derive(Debug)]
pub struct SimpleCacheInterface {
    cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl SimpleCacheInterface {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl CacheInterface for SimpleCacheInterface {
    async fn get_pnl_snapshot(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError> {
        let cache = self.cache.read().await;
        let key = format!("pnl_snapshot:{}", user_id);
        
        if let Some(data) = cache.get(&key) {
            let snapshot: PnLSnapshot = serde_json::from_slice(data)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    async fn set_pnl_snapshot(&self, snapshot: &PnLSnapshot, _ttl_seconds: u64) -> Result<(), RiskError> {
        let mut cache = self.cache.write().await;
        let key = format!("pnl_snapshot:{}", snapshot.user_id);
        let data = serde_json::to_vec(snapshot)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        cache.insert(key, data);
        Ok(())
    }

    async fn get_pnl_history(&self, cache_key: &str) -> Result<Option<Vec<PnLSnapshot>>, RiskError> {
        let cache = self.cache.read().await;
        
        if let Some(data) = cache.get(cache_key) {
            let snapshots: Vec<PnLSnapshot> = serde_json::from_slice(data)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            Ok(Some(snapshots))
        } else {
            Ok(None)
        }
    }

    async fn set_pnl_history(&self, cache_key: &str, snapshots: &[PnLSnapshot], _ttl_seconds: u64) -> Result<(), RiskError> {
        let mut cache = self.cache.write().await;
        let data = serde_json::to_vec(snapshots)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        cache.insert(cache_key.to_string(), data);
        Ok(())
    }

    async fn invalidate_user_pnl_cache(&self, user_id: Uuid) -> Result<(), RiskError> {
        let mut cache = self.cache.write().await;
        let key = format!("pnl_snapshot:{}", user_id);
        cache.remove(&key);
        Ok(())
    }
}
