use crate::analytics::live_pnl_engine::{PnLSnapshot, PositionPnL};
use crate::risk_management::types::RiskError;
use chrono::{DateTime, Utc, Datelike, Timelike};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// P&L persistence layer for historical tracking
pub struct PnLPersistenceManager<P: PostgresInterface> {
    postgres_client: Arc<P>,
    cache_manager: Arc<dyn CacheInterface>,
    persistence_config: PersistenceConfig,
    persistence_stats: Arc<RwLock<PersistenceStats>>,
}

/// PostgreSQL interface for P&L persistence
#[async_trait::async_trait]
pub trait PostgresInterface: Send + Sync {
    async fn insert_pnl_snapshot(&self, snapshot: &PnLSnapshot) -> Result<(), RiskError>;
    async fn get_pnl_snapshots(&self, user_id: Uuid, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Vec<PnLSnapshot>, RiskError>;
    async fn get_latest_pnl_snapshot(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError>;
    async fn insert_position_pnl_history(&self, user_id: Uuid, position_pnl: &PositionPnL) -> Result<(), RiskError>;
    async fn get_position_pnl_history(&self, user_id: Uuid, token_address: &str, chain_id: u64, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Vec<PositionPnL>, RiskError>;
    async fn cleanup_old_snapshots(&self, retention_days: u32) -> Result<u64, RiskError>;
}

/// Cache interface for P&L data caching
#[async_trait::async_trait]
pub trait CacheInterface: Send + Sync {
    async fn get_pnl_snapshot(&self, user_id: Uuid, timestamp: DateTime<Utc>) -> Result<Option<PnLSnapshot>, RiskError>;
    async fn set_pnl_snapshot(&self, snapshot: &PnLSnapshot) -> Result<(), RiskError>;
    async fn get_pnl_history(&self, cache_key: &str) -> Result<Option<Vec<PnLSnapshot>>, RiskError>;
    async fn set_pnl_history(&self, cache_key: &str, snapshots: &[PnLSnapshot]) -> Result<(), RiskError>;
    async fn invalidate_user_pnl_cache(&self, user_id: Uuid) -> Result<(), RiskError>;
}

/// P&L historical data query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLHistoryQuery {
    pub user_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub token_filter: Option<Vec<String>>, // Filter by specific token addresses
    pub chain_filter: Option<Vec<u64>>, // Filter by specific chain IDs
    pub aggregation_interval: Option<AggregationInterval>, // Aggregate data by interval
    pub include_position_details: bool,
    pub limit: Option<u32>, // Limit number of results
}

/// Data aggregation intervals
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AggregationInterval {
    Minute,
    FiveMinutes,
    FifteenMinutes,
    Hour,
    Day,
    Week,
    Month,
}

/// Aggregated P&L data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPnLData {
    pub timestamp: DateTime<Utc>,
    pub interval: AggregationInterval,
    pub user_id: Uuid,
    pub avg_total_pnl_usd: Decimal,
    pub max_total_pnl_usd: Decimal,
    pub min_total_pnl_usd: Decimal,
    pub avg_portfolio_value_usd: Decimal,
    pub max_portfolio_value_usd: Decimal,
    pub min_portfolio_value_usd: Decimal,
    pub sample_count: u64,
    pub volatility: Decimal,
}

/// P&L persistence configuration
#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    pub snapshot_retention_days: u32,
    pub position_history_retention_days: u32,
    pub cache_ttl_seconds: u64,
    pub batch_insert_size: usize,
    pub cleanup_interval_hours: u64,
    pub enable_compression: bool,
    pub enable_partitioning: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            snapshot_retention_days: 90, // 3 months
            position_history_retention_days: 365, // 1 year
            cache_ttl_seconds: 300, // 5 minutes
            batch_insert_size: 1000,
            cleanup_interval_hours: 24, // Daily cleanup
            enable_compression: true,
            enable_partitioning: true,
        }
    }
}

/// Persistence statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistenceStats {
    pub total_snapshots_stored: u64,
    pub total_position_history_stored: u64,
    pub successful_inserts: u64,
    pub failed_inserts: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cleanup_operations: u64,
    pub records_cleaned: u64,
    pub average_insert_time_ms: f64,
    pub last_cleanup_time: Option<DateTime<Utc>>,
}

impl<P: PostgresInterface> PnLPersistenceManager<P> {
    /// Create new P&L persistence manager
    pub async fn new(
        postgres_client: Arc<P>,
        cache_manager: Arc<dyn CacheInterface>,
        config: PersistenceConfig,
    ) -> Result<Self, RiskError> {
        Ok(Self {
            postgres_client,
            cache_manager,
            persistence_config: config,
            persistence_stats: Arc::new(RwLock::new(PersistenceStats::default())),
        })
    }

    /// Store P&L snapshot with historical tracking
    pub async fn store_pnl_snapshot(&self, snapshot: &PnLSnapshot) -> Result<(), RiskError> {
        let start_time = std::time::Instant::now();

        // Store in database
        match self.postgres_client.insert_pnl_snapshot(snapshot).await {
            Ok(_) => {
                // Update cache
                if let Err(e) = self.cache_manager.set_pnl_snapshot(snapshot).await {
                    warn!("Failed to cache P&L snapshot for user {}: {}", snapshot.user_id, e);
                }

                // Store individual position history
                for position in &snapshot.positions {
                    if let Err(e) = self.postgres_client.insert_position_pnl_history(snapshot.user_id, position).await {
                        warn!("Failed to store position P&L history for {}: {}", position.token_address, e);
                    }
                }

                // Update stats
                let insert_time = start_time.elapsed().as_millis() as f64;
                let mut stats = self.persistence_stats.write().await;
                stats.total_snapshots_stored += 1;
                stats.total_position_history_stored += snapshot.positions.len() as u64;
                stats.successful_inserts += 1;
                stats.average_insert_time_ms = 
                    (stats.average_insert_time_ms * (stats.successful_inserts - 1) as f64 + insert_time) 
                    / stats.successful_inserts as f64;

                info!("Stored P&L snapshot for user {} with {} positions in {:.2}ms", 
                      snapshot.user_id, snapshot.positions.len(), insert_time);

                Ok(())
            }
            Err(e) => {
                let mut stats = self.persistence_stats.write().await;
                stats.failed_inserts += 1;
                error!("Failed to store P&L snapshot for user {}: {}", snapshot.user_id, e);
                Err(e)
            }
        }
    }

    /// Get P&L history for a user
    pub async fn get_pnl_history(&self, query: &PnLHistoryQuery) -> Result<Vec<PnLSnapshot>, RiskError> {
        // Generate cache key
        let cache_key = format!("pnl_history:{}:{}:{}:{}", 
                               query.user_id, 
                               query.start_time.timestamp(), 
                               query.end_time.timestamp(),
                               query.aggregation_interval.as_ref().map(|i| format!("{:?}", i)).unwrap_or_else(|| "none".to_string()));

        // Check cache first
        if let Ok(Some(cached_history)) = self.cache_manager.get_pnl_history(&cache_key).await {
            let mut stats = self.persistence_stats.write().await;
            stats.cache_hits += 1;
            return Ok(cached_history);
        }

        // Cache miss - fetch from database
        let mut stats = self.persistence_stats.write().await;
        stats.cache_misses += 1;
        drop(stats);

        let mut snapshots = self.postgres_client.get_pnl_snapshots(
            query.user_id, 
            query.start_time, 
            query.end_time
        ).await?;

        // Apply filters
        if let Some(ref token_filter) = query.token_filter {
            snapshots = snapshots.into_iter().map(|mut snapshot| {
                snapshot.positions.retain(|pos| token_filter.contains(&pos.token_address));
                // Recalculate totals after filtering
                self.recalculate_snapshot_totals(&mut snapshot);
                snapshot
            }).collect();
        }

        if let Some(ref chain_filter) = query.chain_filter {
            snapshots = snapshots.into_iter().map(|mut snapshot| {
                snapshot.positions.retain(|pos| chain_filter.contains(&pos.chain_id));
                // Recalculate totals after filtering
                self.recalculate_snapshot_totals(&mut snapshot);
                snapshot
            }).collect();
        }

        // Apply aggregation if requested
        if let Some(ref interval) = query.aggregation_interval {
            snapshots = self.aggregate_snapshots(snapshots, interval.clone()).await?;
        }

        // Cache the result
        if let Err(e) = self.cache_manager.set_pnl_history(&cache_key, &snapshots).await {
            warn!("Failed to cache P&L history: {}", e);
        }

        Ok(snapshots)
    }

    /// Get latest P&L snapshot for a user
    pub async fn get_latest_pnl_snapshot(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError> {
        // Check cache first
        if let Ok(Some(cached_snapshot)) = self.cache_manager.get_pnl_snapshot(user_id, chrono::Utc::now()).await {
            let mut stats = self.persistence_stats.write().await;
            stats.cache_hits += 1;
            return Ok(Some(cached_snapshot));
        }

        // Cache miss - fetch from database
        let mut stats = self.persistence_stats.write().await;
        stats.cache_misses += 1;
        drop(stats);

        let snapshot = self.postgres_client.get_latest_pnl_snapshot(user_id).await?;

        // Cache the result if found
        if let Some(ref snapshot) = snapshot {
            if let Err(e) = self.cache_manager.set_pnl_snapshot(snapshot).await {
                warn!("Failed to cache latest P&L snapshot: {}", e);
            }
        }

        Ok(snapshot)
    }

    /// Get position P&L history for specific token
    pub async fn get_position_history(
        &self, 
        user_id: Uuid, 
        token_address: &str, 
        chain_id: u64, 
        start_time: DateTime<Utc>, 
        end_time: DateTime<Utc>
    ) -> Result<Vec<PositionPnL>, RiskError> {
        self.postgres_client.get_position_pnl_history(user_id, token_address, chain_id, start_time, end_time).await
    }

    /// Get aggregated P&L data
    pub async fn get_aggregated_pnl_data(&self, query: &PnLHistoryQuery) -> Result<Vec<AggregatedPnLData>, RiskError> {
        let snapshots = self.get_pnl_history(query).await?;
        
        if snapshots.is_empty() {
            return Ok(Vec::new());
        }

        let interval = query.aggregation_interval.clone().unwrap_or(AggregationInterval::Hour);
        let aggregated_data = self.calculate_aggregated_data(snapshots, interval).await?;

        Ok(aggregated_data)
    }

    /// Calculate aggregated P&L statistics
    async fn calculate_aggregated_data(&self, snapshots: Vec<PnLSnapshot>, interval: AggregationInterval) -> Result<Vec<AggregatedPnLData>, RiskError> {
        let mut aggregated_data = Vec::new();
        
        if snapshots.is_empty() {
            return Ok(aggregated_data);
        }

        // Group snapshots by time interval
        let mut grouped_snapshots: HashMap<DateTime<Utc>, Vec<PnLSnapshot>> = HashMap::new();
        
        for snapshot in snapshots {
            let interval_timestamp = self.round_to_interval(snapshot.timestamp, &interval);
            grouped_snapshots.entry(interval_timestamp).or_insert_with(Vec::new).push(snapshot);
        }

        // Calculate aggregated statistics for each interval
        for (timestamp, interval_snapshots) in grouped_snapshots {
            if interval_snapshots.is_empty() {
                continue;
            }

            let user_id = interval_snapshots[0].user_id;
            let sample_count = interval_snapshots.len() as u64;

            let total_pnls: Vec<Decimal> = interval_snapshots.iter().map(|s| s.total_pnl_usd).collect();
            let portfolio_values: Vec<Decimal> = interval_snapshots.iter().map(|s| s.total_portfolio_value_usd).collect();

            let avg_total_pnl = total_pnls.iter().sum::<Decimal>() / Decimal::from(sample_count);
            let max_total_pnl = total_pnls.iter().max().cloned().unwrap_or(Decimal::ZERO);
            let min_total_pnl = total_pnls.iter().min().cloned().unwrap_or(Decimal::ZERO);

            let avg_portfolio_value = portfolio_values.iter().sum::<Decimal>() / Decimal::from(sample_count);
            let max_portfolio_value = portfolio_values.iter().max().cloned().unwrap_or(Decimal::ZERO);
            let min_portfolio_value = portfolio_values.iter().min().cloned().unwrap_or(Decimal::ZERO);

            // Calculate volatility (standard deviation of P&L)
            let variance = total_pnls.iter()
                .map(|pnl| (*pnl - avg_total_pnl) * (*pnl - avg_total_pnl))
                .sum::<Decimal>() / Decimal::from(sample_count);
            let volatility = if variance > Decimal::ZERO {
                // Convert to f64, calculate sqrt, then back to Decimal
                let variance_f64 = variance.to_string().parse::<f64>().unwrap_or(0.0);
                let sqrt_f64 = variance_f64.sqrt();
                Decimal::try_from(sqrt_f64).unwrap_or(Decimal::ZERO)
            } else {
                Decimal::ZERO
            };

            aggregated_data.push(AggregatedPnLData {
                timestamp,
                interval: interval.clone(),
                user_id,
                avg_total_pnl_usd: avg_total_pnl,
                max_total_pnl_usd: max_total_pnl,
                min_total_pnl_usd: min_total_pnl,
                avg_portfolio_value_usd: avg_portfolio_value,
                max_portfolio_value_usd: max_portfolio_value,
                min_portfolio_value_usd: min_portfolio_value,
                sample_count,
                volatility,
            });
        }

        // Sort by timestamp
        aggregated_data.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(aggregated_data)
    }

    /// Round timestamp to aggregation interval
    fn round_to_interval(&self, timestamp: DateTime<Utc>, interval: &AggregationInterval) -> DateTime<Utc> {
        match interval {
            AggregationInterval::Minute => timestamp.with_second(0).unwrap().with_nanosecond(0).unwrap(),
            AggregationInterval::FiveMinutes => {
                let minute = (timestamp.minute() / 5) * 5;
                timestamp.with_minute(minute).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap()
            }
            AggregationInterval::FifteenMinutes => {
                let minute = (timestamp.minute() / 15) * 15;
                timestamp.with_minute(minute).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap()
            }
            AggregationInterval::Hour => timestamp.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap(),
            AggregationInterval::Day => timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc(),
            AggregationInterval::Week => {
                let days_since_monday = timestamp.weekday().num_days_from_monday();
                let week_start = timestamp.date_naive() - chrono::Duration::days(days_since_monday as i64);
                week_start.and_hms_opt(0, 0, 0).unwrap().and_utc()
            }
            AggregationInterval::Month => {
                timestamp.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
            }
        }
    }

    /// Aggregate snapshots by time interval
    async fn aggregate_snapshots(&self, snapshots: Vec<PnLSnapshot>, interval: AggregationInterval) -> Result<Vec<PnLSnapshot>, RiskError> {
        // For now, return original snapshots - full aggregation would be more complex
        // In production, this would group snapshots by interval and create averaged snapshots
        Ok(snapshots)
    }

    /// Recalculate snapshot totals after filtering positions
    fn recalculate_snapshot_totals(&self, snapshot: &mut PnLSnapshot) {
        snapshot.total_unrealized_pnl_usd = snapshot.positions.iter().map(|p| p.unrealized_pnl_usd).sum();
        snapshot.total_realized_pnl_usd = snapshot.positions.iter().map(|p| p.realized_pnl_usd).sum();
        snapshot.total_pnl_usd = snapshot.total_unrealized_pnl_usd + snapshot.total_realized_pnl_usd;
        snapshot.total_portfolio_value_usd = snapshot.positions.iter().map(|p| p.position_value_usd).sum();
    }

    /// Start periodic cleanup of old data
    pub async fn start_cleanup_task(self: Arc<Self>) -> Result<(), RiskError> 
    where 
        P: 'static,
    {
        let cleanup_interval = self.persistence_config.cleanup_interval_hours;
        
        info!("Starting P&L data cleanup task with {}h interval", cleanup_interval);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(cleanup_interval * 3600));
            loop {
                interval.tick().await;
                
                // Cleanup old snapshots
                if let Err(e) = self.postgres_client.cleanup_old_snapshots(self.persistence_config.snapshot_retention_days).await {
                    tracing::error!("Failed to cleanup old snapshots: {}", e);
                }
                
                // Update stats
                let mut stats = self.persistence_stats.write().await;
                stats.cleanup_operations += 1;
            }
        });

        Ok(())
    }

    /// Clean up old P&L data based on retention policy
    async fn cleanup_old_data(&self) -> Result<(), RiskError> {
        info!("Starting P&L data cleanup...");

        let records_cleaned = self.postgres_client.cleanup_old_snapshots(self.persistence_config.snapshot_retention_days).await?;

        let mut stats = self.persistence_stats.write().await;
        stats.cleanup_operations += 1;
        stats.records_cleaned += records_cleaned;
        stats.last_cleanup_time = Some(Utc::now());

        info!("P&L data cleanup completed: {} records cleaned", records_cleaned);

        Ok(())
    }

    /// Invalidate user P&L cache
    pub async fn invalidate_user_cache(&self, user_id: Uuid) -> Result<(), RiskError> {
        self.cache_manager.invalidate_user_pnl_cache(user_id).await
    }

    /// Get persistence statistics
    pub async fn get_persistence_stats(&self) -> PersistenceStats {
        self.persistence_stats.read().await.clone()
    }
}

// Clone implementation for PnLPersistenceManager
impl<P: PostgresInterface> Clone for PnLPersistenceManager<P> {
    fn clone(&self) -> Self {
        Self {
            postgres_client: Arc::clone(&self.postgres_client),
            cache_manager: Arc::clone(&self.cache_manager),
            persistence_config: self.persistence_config.clone(),
            persistence_stats: Arc::clone(&self.persistence_stats),
        }
    }
}
