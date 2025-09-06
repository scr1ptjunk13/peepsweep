use crate::analytics::data_models::*;
use crate::analytics::live_pnl_engine::*;
use crate::analytics::pnl_persistence::*;
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use rust_decimal_macros::dec;
use chrono::{DateTime, Utc, Duration, Datelike, Timelike};
use uuid::Uuid;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::risk_management::types::RiskError;
use tracing::{debug, error, info, warn};
use tokio::sync::RwLock;
use std::sync::Arc;
use num_traits::FromPrimitive;

/// Historical P&L aggregation manager for rollups and analytics
#[derive(Debug)]
pub struct PnLAggregationManager {
    aggregation_config: AggregationConfig,
    aggregation_stats: Arc<RwLock<AggregationStats>>,
    rollup_cache: Arc<RwLock<HashMap<String, PnLRollup>>>,
}

/// Aggregation configuration
#[derive(Debug, Clone)]
pub struct AggregationConfig {
    pub enable_rollups: bool,
    pub rollup_intervals: Vec<AggregationInterval>,
    pub retention_policy: HashMap<AggregationInterval, u32>, // Days to retain
    pub batch_size: usize,
    pub parallel_processing: bool,
    pub max_concurrent_aggregations: usize,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        let mut retention_policy = HashMap::new();
        retention_policy.insert(AggregationInterval::Minute, 7); // 7 days
        retention_policy.insert(AggregationInterval::FiveMinutes, 30); // 30 days
        retention_policy.insert(AggregationInterval::FifteenMinutes, 90); // 90 days
        retention_policy.insert(AggregationInterval::Hour, 365); // 1 year
        retention_policy.insert(AggregationInterval::Day, 1095); // 3 years
        retention_policy.insert(AggregationInterval::Week, 2190); // 6 years
        retention_policy.insert(AggregationInterval::Month, 3650); // 10 years

        Self {
            enable_rollups: true,
            rollup_intervals: vec![
                AggregationInterval::FiveMinutes,
                AggregationInterval::FifteenMinutes,
                AggregationInterval::Hour,
                AggregationInterval::Day,
                AggregationInterval::Week,
                AggregationInterval::Month,
            ],
            retention_policy,
            batch_size: 10000,
            parallel_processing: true,
            max_concurrent_aggregations: 4,
        }
    }
}

/// P&L rollup data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLRollup {
    pub user_id: Uuid,
    pub interval: AggregationInterval,
    pub timestamp_start: DateTime<Utc>,
    pub timestamp_end: DateTime<Utc>,
    pub total_pnl_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub portfolio_value_usd: Decimal,
    pub daily_change_usd: Decimal,
    pub daily_change_percent: Decimal,
    pub position_count: u32,
    pub data_points: u32,
}

/// Aggregated P&L rollup data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPnLRollup {
    pub user_id: Uuid,
    pub interval: AggregationInterval,
    pub timestamp: DateTime<Utc>,
    pub time_bucket_start: DateTime<Utc>,
    pub time_bucket_end: DateTime<Utc>,
    
    // Aggregated P&L metrics
    pub total_pnl_usd: AggregatedMetric,
    pub unrealized_pnl_usd: AggregatedMetric,
    pub realized_pnl_usd: AggregatedMetric,
    pub portfolio_value_usd: AggregatedMetric,
    pub daily_change_usd: AggregatedMetric,
    pub daily_change_percent: AggregatedMetric,
    
    // Multi-currency aggregations
    pub total_pnl_eth: AggregatedMetric,
    pub total_pnl_btc: AggregatedMetric,
    
    // Statistical metrics
    pub volatility: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown: Decimal,
    pub win_rate: Decimal,
    
    // Metadata
    pub sample_count: u64,
    pub data_points: u64,
    pub calculation_duration_ms: u64,
    pub created_at: DateTime<Utc>,
}

/// Aggregated metric with statistical measures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetric {
    pub min: Decimal,
    pub max: Decimal,
    pub avg: Decimal,
    pub sum: Decimal,
    pub first: Decimal,
    pub last: Decimal,
    pub std_dev: Decimal,
    pub median: Decimal,
    pub percentile_95: Decimal,
    pub percentile_99: Decimal,
}

/// Aggregation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregationStats {
    pub total_aggregations: u64,
    pub successful_aggregations: u64,
    pub failed_aggregations: u64,
    pub total_data_points_processed: u64,
    pub average_aggregation_time_ms: f64,
    pub rollups_created: HashMap<AggregationInterval, u64>,
    pub last_aggregation_time: Option<DateTime<Utc>>,
}

/// Aggregation job for parallel processing
#[derive(Debug, Clone)]
pub struct AggregationJob {
    pub user_id: Uuid,
    pub interval: AggregationInterval,
    pub time_range: TimeRange,
    pub snapshots: Vec<PnLSnapshot>,
}

impl PnLAggregationManager {
    /// Create new P&L aggregation manager
    pub async fn new(config: AggregationConfig) -> Result<Self, RiskError> {
        Ok(Self {
            aggregation_config: config,
            aggregation_stats: Arc::new(RwLock::new(AggregationStats::default())),
            rollup_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create historical P&L rollups for all intervals
    pub async fn create_historical_rollups(
        &self,
        user_id: Uuid,
        snapshots: Vec<PnLSnapshot>,
        time_range: TimeRange,
    ) -> Result<Vec<PnLRollup>, RiskError> {
        if snapshots.is_empty() {
            return Ok(Vec::new());
        }

        let start_time = std::time::Instant::now();
        let mut rollups = Vec::new();

        // Update aggregation stats
        {
            let mut stats = self.aggregation_stats.write().await;
            stats.total_aggregations += 1;
            stats.total_data_points_processed += snapshots.len() as u64;
            stats.last_aggregation_time = Some(Utc::now());
        }

        if self.aggregation_config.parallel_processing {
            // Process intervals in parallel
            let mut tasks = Vec::new();
            
            for interval in &self.aggregation_config.rollup_intervals {
                let job = AggregationJob {
                    user_id,
                    interval: interval.clone(),
                    time_range: time_range.clone(),
                    snapshots: snapshots.clone(),
                };
                
                let manager = self.clone();
                let task = tokio::spawn(async move {
                    manager.process_aggregation_job(job).await
                });
                tasks.push(task);
            }

            // Collect results
            for task in tasks {
                match task.await {
                    Ok(Ok(rollup)) => rollups.push(rollup),
                    Ok(Err(e)) => {
                        error!("Aggregation task failed: {}", e);
                        let mut stats = self.aggregation_stats.write().await;
                        stats.failed_aggregations += 1;
                    }
                    Err(e) => {
                        error!("Aggregation task panicked: {}", e);
                        let mut stats = self.aggregation_stats.write().await;
                        stats.failed_aggregations += 1;
                    }
                }
            }
        } else {
            // Process intervals sequentially
            for interval in &self.aggregation_config.rollup_intervals {
                match self.create_pnl_rollup(user_id, time_range.start, snapshots.clone(), interval).await {
                    Ok(rollup) => rollups.push(rollup),
                    Err(e) => {
                        error!("Failed to create rollup for interval {:?}: {}", interval, e);
                        let mut stats = self.aggregation_stats.write().await;
                        stats.failed_aggregations += 1;
                    }
                }
            }
        }

        let aggregation_duration = start_time.elapsed().as_millis() as u64;

        // Update successful aggregation stats
        {
            let mut stats = self.aggregation_stats.write().await;
            stats.successful_aggregations += 1;
            stats.average_aggregation_time_ms = 
                (stats.average_aggregation_time_ms * (stats.successful_aggregations - 1) as f64 + aggregation_duration as f64) 
                / stats.successful_aggregations as f64;

            for rollup in &rollups {
                *stats.rollups_created.entry(rollup.interval.clone()).or_insert(0) += 1;
            }
        }

        info!("Created {} P&L rollups for user {} across {} intervals in {}ms",
              rollups.len(), user_id, self.aggregation_config.rollup_intervals.len(), aggregation_duration);

        Ok(rollups)
    }

    /// Process single aggregation job
    async fn process_aggregation_job(&self, job: AggregationJob) -> Result<PnLRollup, RiskError> {
        self.create_pnl_rollup(job.user_id, job.time_range.start, job.snapshots, &job.interval).await
    }

    /// Create rollup for specific interval
    async fn create_pnl_rollup(
        &self,
        user_id: Uuid,
        bucket_start: DateTime<Utc>,
        bucket_snapshots: Vec<PnLSnapshot>,
        interval: &AggregationInterval,
    ) -> Result<PnLRollup, RiskError> {
        let start_time = std::time::Instant::now();
        
        if bucket_snapshots.is_empty() {
            return Err(RiskError::ValidationError("No data points for aggregation".to_string()));
        }
        let bucket_end = self.calculate_bucket_end(bucket_start, interval);

        // Calculate aggregated metrics
        let total_pnl_usd = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_pnl_usd)?.sum;
        let unrealized_pnl_usd = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_unrealized_pnl_usd)?.sum;
        let realized_pnl_usd = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_realized_pnl_usd)?.sum;
        let portfolio_value_usd = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_portfolio_value_usd)?.avg;
        let daily_change_usd = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.daily_change_usd)?.avg;
        let daily_change_percent = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.daily_change_percent)?.avg;

        // Calculate multi-currency metrics (simplified - would need actual conversion rates)
        let total_pnl_eth = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_pnl_usd / Decimal::new(3200, 0))?;
        let total_pnl_btc = self.calculate_aggregated_metric(&bucket_snapshots, |s| s.total_pnl_usd / Decimal::new(65000, 0))?;

        // Calculate statistical metrics
        let volatility = self.calculate_volatility(&bucket_snapshots)?;
        let sharpe_ratio = self.calculate_sharpe_ratio(&bucket_snapshots)?;
        let max_drawdown = self.calculate_max_drawdown(&bucket_snapshots)?;
        let win_rate = self.calculate_win_rate(&bucket_snapshots)?;

        let calculation_duration = start_time.elapsed().as_millis() as u64;

        let rollup = PnLRollup {
            user_id,
            interval: interval.clone(),
            timestamp_start: bucket_start,
            timestamp_end: bucket_end,
            total_pnl_usd,
            unrealized_pnl_usd,
            realized_pnl_usd,
            portfolio_value_usd,
            daily_change_usd,
            daily_change_percent,
            position_count: bucket_snapshots.len() as u32,
            data_points: bucket_snapshots.len() as u32,
        };

        // Cache the rollup - create references before move
        let interval_str = format!("{:?}", &interval);
        let interval_ref = &interval;
        let cache_key = format!("{}:{}:{}", user_id, interval_str, bucket_start.timestamp());
        let mut cache = self.rollup_cache.write().await;
        cache.insert(cache_key, rollup.clone());

        debug!("Created P&L rollup for user {} interval {:?} in {}ms", 
               user_id, interval_ref, calculation_duration);

        Ok(rollup)
    }

    /// Group snapshots by time bucket based on aggregation interval
    fn group_snapshots_by_time_bucket(
        &self,
        snapshots: &[PnLSnapshot],
        interval: &AggregationInterval,
    ) -> HashMap<DateTime<Utc>, Vec<PnLSnapshot>> {
        let mut buckets = HashMap::new();

        for snapshot in snapshots {
            let bucket_start = self.round_to_interval(snapshot.timestamp, interval);
            buckets.entry(bucket_start).or_insert_with(Vec::new).push(snapshot.clone());
        }

        buckets
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

    /// Calculate bucket end time
    fn calculate_bucket_end(&self, bucket_start: DateTime<Utc>, interval: &AggregationInterval) -> DateTime<Utc> {
        match interval {
            AggregationInterval::Minute => bucket_start + chrono::Duration::minutes(1),
            AggregationInterval::FiveMinutes => bucket_start + chrono::Duration::minutes(5),
            AggregationInterval::FifteenMinutes => bucket_start + chrono::Duration::minutes(15),
            AggregationInterval::Hour => bucket_start + chrono::Duration::hours(1),
            AggregationInterval::Day => bucket_start + chrono::Duration::days(1),
            AggregationInterval::Week => bucket_start + chrono::Duration::weeks(1),
            AggregationInterval::Month => {
                // Add one month (approximate)
                bucket_start + chrono::Duration::days(30)
            }
        }
    }

    /// Calculate aggregated metric from snapshots
    fn calculate_aggregated_metric<F>(&self, snapshots: &[PnLSnapshot], extractor: F) -> Result<AggregatedMetric, RiskError>
    where
        F: Fn(&PnLSnapshot) -> Decimal,
    {
        if snapshots.is_empty() {
            return Ok(AggregatedMetric {
                min: Decimal::ZERO,
                max: Decimal::ZERO,
                avg: Decimal::ZERO,
                sum: Decimal::ZERO,
                first: Decimal::ZERO,
                last: Decimal::ZERO,
                std_dev: Decimal::ZERO,
                median: Decimal::ZERO,
                percentile_95: Decimal::ZERO,
                percentile_99: Decimal::ZERO,
            });
        }

        let values: Vec<Decimal> = snapshots.iter().map(|s| extractor(s)).collect();
        
        let min = values.iter().min().cloned().unwrap_or(Decimal::ZERO);
        let max = values.iter().max().cloned().unwrap_or(Decimal::ZERO);
        let sum: Decimal = values.iter().sum();
        let avg = sum / Decimal::from(values.len());
        let first = values.first().cloned().unwrap_or(Decimal::ZERO);
        let last = values.last().cloned().unwrap_or(Decimal::ZERO);

        // Calculate standard deviation
        let variance: Decimal = values.iter()
            .map(|v| (*v - avg).powi(2))
            .sum::<Decimal>() / Decimal::from(values.len());
        let std_dev = variance.sqrt().unwrap_or(Decimal::ZERO);

        // Calculate percentiles
        let mut sorted_values = values.clone();
        sorted_values.sort();
        
        let median = self.calculate_percentile(&sorted_values, 50.0);
        let percentile_95 = self.calculate_percentile(&sorted_values, 95.0);
        let percentile_99 = self.calculate_percentile(&sorted_values, 99.0);

        Ok(AggregatedMetric {
            min,
            max,
            avg,
            sum,
            first,
            last,
            std_dev,
            median,
            percentile_95,
            percentile_99,
        })
    }

    /// Calculate percentile from sorted values
    fn calculate_percentile(&self, sorted_values: &[Decimal], percentile: f64) -> Decimal {
        if sorted_values.is_empty() {
            return Decimal::ZERO;
        }

        let index = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
        let lower_index = index.floor() as usize;
        let upper_index = index.ceil() as usize;

        if lower_index == upper_index {
            sorted_values[lower_index]
        } else {
            let weight = Decimal::from_f64(index - lower_index as f64).unwrap_or(Decimal::ZERO);
            let lower_value = sorted_values[lower_index];
            let upper_value = sorted_values[upper_index];
            lower_value + (upper_value - lower_value) * weight
        }
    }

    /// Calculate volatility (standard deviation of returns)
    fn calculate_volatility(&self, snapshots: &[PnLSnapshot]) -> Result<Decimal, RiskError> {
        if snapshots.len() < 2 {
            return Ok(Decimal::ZERO);
        }

        let returns: Vec<Decimal> = snapshots.windows(2)
            .map(|window| {
                let current = window[1].total_portfolio_value_usd;
                let previous = window[0].total_portfolio_value_usd;
                if previous > Decimal::ZERO {
                    (current - previous) / previous
                } else {
                    Decimal::ZERO
                }
            })
            .collect();

        let avg_return: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        let variance: Decimal = returns.iter()
            .map(|r| (*r - avg_return).powi(2))
            .sum::<Decimal>() / Decimal::from(returns.len());

        Ok(variance.sqrt().unwrap_or(Decimal::ZERO))
    }

    /// Calculate Sharpe ratio (simplified)
    fn calculate_sharpe_ratio(&self, snapshots: &[PnLSnapshot]) -> Result<Decimal, RiskError> {
        let volatility = self.calculate_volatility(snapshots)?;
        
        if snapshots.is_empty() || volatility == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        let total_return = if snapshots.len() >= 2 {
            let initial_value = snapshots.first().unwrap().total_portfolio_value_usd;
            let final_value = snapshots.last().unwrap().total_portfolio_value_usd;
            
            if initial_value > Decimal::ZERO {
                (final_value - initial_value) / initial_value
            } else {
                Decimal::ZERO
            }
        } else {
            Decimal::ZERO
        };

        // Assuming risk-free rate of 2% annually (simplified)
        let risk_free_rate = Decimal::new(2, 2); // 0.02
        
        Ok((total_return - risk_free_rate) / volatility)
    }

    /// Calculate maximum drawdown
    fn calculate_max_drawdown(&self, snapshots: &[PnLSnapshot]) -> Result<Decimal, RiskError> {
        if snapshots.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let mut max_value = snapshots[0].total_portfolio_value_usd;
        let mut max_drawdown = Decimal::ZERO;

        for snapshot in snapshots {
            let current_value = snapshot.total_portfolio_value_usd;
            
            if current_value > max_value {
                max_value = current_value;
            }
            
            if max_value > Decimal::ZERO {
                let drawdown = (max_value - current_value) / max_value;
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
            }
        }

        Ok(max_drawdown * Decimal::new(100, 0)) // Convert to percentage
    }

    /// Calculate win rate (percentage of positive P&L periods)
    fn calculate_win_rate(&self, snapshots: &[PnLSnapshot]) -> Result<Decimal, RiskError> {
        if snapshots.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let positive_periods = snapshots.iter()
            .filter(|s| s.total_pnl_usd > Decimal::ZERO)
            .count();

        let win_rate = Decimal::from(positive_periods) / Decimal::from(snapshots.len());
        Ok(win_rate * Decimal::new(100, 0)) // Convert to percentage
    }

    /// Get aggregation statistics
    pub async fn get_aggregation_stats(&self) -> AggregationStats {
        self.aggregation_stats.read().await.clone()
    }

    /// Get cached rollup
    pub async fn get_cached_rollup(&self, cache_key: &str) -> Option<PnLRollup> {
        let cache = self.rollup_cache.read().await;
        cache.get(cache_key).cloned()
    }

    /// Clear rollup cache
    pub async fn clear_cache(&self) -> Result<(), RiskError> {
        let mut cache = self.rollup_cache.write().await;
        cache.clear();
        info!("P&L aggregation cache cleared");
        Ok(())
    }
}

impl Clone for PnLAggregationManager {
    fn clone(&self) -> Self {
        Self {
            aggregation_config: self.aggregation_config.clone(),
            aggregation_stats: Arc::clone(&self.aggregation_stats),
            rollup_cache: Arc::clone(&self.rollup_cache),
        }
    }
}

impl AggregationInterval {
    fn to_string(&self) -> String {
        match self {
            AggregationInterval::Minute => "1m".to_string(),
            AggregationInterval::FiveMinutes => "5m".to_string(),
            AggregationInterval::FifteenMinutes => "15m".to_string(),
            AggregationInterval::Hour => "1h".to_string(),
            AggregationInterval::Day => "1d".to_string(),
            AggregationInterval::Week => "1w".to_string(),
            AggregationInterval::Month => "1M".to_string(),
        }
    }
}
