use crate::analytics::performance_metrics::{PerformanceMetrics, PerformanceComparison, BenchmarkComparison, TimePeriod};
use crate::analytics::benchmark_integration::BenchmarkDataManager;
use crate::risk_management::types::{RiskError, UserId};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Performance comparison engine for multi-user analysis
#[derive(Debug)]
pub struct PerformanceComparator {
    user_metrics: Arc<RwLock<HashMap<Uuid, PerformanceMetrics>>>,
    benchmark_manager: Arc<BenchmarkDataManager>,
    anonymization_engine: Arc<AnonymizationEngine>,
    comparison_cache: Arc<RwLock<HashMap<String, CachedComparison>>>,
    risk_free_rate: Decimal,
}

/// Anonymized performance data for leaderboards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymizedPerformance {
    pub anonymous_id: String,
    pub performance_metrics: PerformanceMetrics,
    pub percentile_rank: Decimal,
    pub category: PerformanceCategory,
    pub risk_adjusted_score: Decimal,
}

/// Performance categories for grouping users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceCategory {
    Conservative,  // Low risk, steady returns
    Moderate,      // Balanced risk/return
    Aggressive,    // High risk, high potential returns
    HighFrequency, // Many trades, short-term focus
    LongTerm,      // Few trades, long-term focus
}

/// Leaderboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardConfig {
    pub time_period: TimePeriod,
    pub metric: LeaderboardMetric,
    pub category_filter: Option<PerformanceCategory>,
    pub min_trades: Option<u64>,
    pub min_portfolio_value: Option<Decimal>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LeaderboardMetric {
    TotalReturn,
    SharpeRatio,
    SortinoRatio,
    MaxDrawdown,
    WinRate,
    ProfitFactor,
    RiskAdjustedReturn,
}

/// Performance cohort analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohortAnalysis {
    pub cohort_name: String,
    pub user_count: u64,
    pub average_metrics: PerformanceMetrics,
    pub median_metrics: PerformanceMetrics,
    pub top_quartile_metrics: PerformanceMetrics,
    pub bottom_quartile_metrics: PerformanceMetrics,
    pub distribution_stats: DistributionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionStats {
    pub mean: Decimal,
    pub median: Decimal,
    pub std_deviation: Decimal,
    pub skewness: Decimal,
    pub kurtosis: Decimal,
    pub percentiles: HashMap<u8, Decimal>, // 10th, 25th, 50th, 75th, 90th percentiles
}

/// Cached comparison result
#[derive(Debug, Clone)]
struct CachedComparison {
    result: PerformanceComparison,
    cached_at: DateTime<Utc>,
    ttl_minutes: u64,
}

/// Anonymization engine for privacy-preserving comparisons
#[derive(Debug)]
pub struct AnonymizationEngine {
    user_mappings: Arc<RwLock<HashMap<Uuid, String>>>,
    reverse_mappings: Arc<RwLock<HashMap<String, Uuid>>>,
}

impl AnonymizationEngine {
    pub fn new() -> Self {
        Self {
            user_mappings: Arc::new(RwLock::new(HashMap::new())),
            reverse_mappings: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create anonymous ID for a user
    pub async fn get_anonymous_id(&self, user_id: &Uuid) -> String {
        let mut mappings = self.user_mappings.write().await;
        
        if let Some(anonymous_id) = mappings.get(user_id) {
            return anonymous_id.clone();
        }

        // Generate new anonymous ID
        let anonymous_id = format!("user_{}", uuid::Uuid::new_v4().to_string()[..8].to_uppercase());
        
        mappings.insert(*user_id, anonymous_id.clone());
        
        let mut reverse_mappings = self.reverse_mappings.write().await;
        reverse_mappings.insert(anonymous_id.clone(), *user_id);
        
        anonymous_id
    }

    /// Check if user has opted into public comparisons
    pub async fn is_user_public(&self, _user_id: &Uuid) -> bool {
        // In a real implementation, this would check user privacy settings
        // For now, assume all users are public for testing
        true
    }
}

impl PerformanceComparator {
    pub fn new(
        benchmark_manager: Arc<BenchmarkDataManager>,
        risk_free_rate: Decimal,
    ) -> Self {
        Self {
            user_metrics: Arc::new(RwLock::new(HashMap::new())),
            benchmark_manager,
            anonymization_engine: Arc::new(AnonymizationEngine::new()),
            comparison_cache: Arc::new(RwLock::new(HashMap::new())),
            risk_free_rate,
        }
    }

    /// Add or update user performance metrics
    pub async fn update_user_metrics(&self, user_id: Uuid, metrics: PerformanceMetrics) {
        let mut user_metrics = self.user_metrics.write().await;
        user_metrics.insert(user_id, metrics);
        debug!("Updated performance metrics for user {}", user_id);
    }

    /// Compare user performance against benchmarks
    pub async fn compare_against_benchmarks(
        &self,
        user_id: &Uuid,
        benchmark_symbols: &[String],
        time_period: TimePeriod,
    ) -> Result<PerformanceComparison, RiskError> {
        let cache_key = format!("{}_{:?}_{:?}", user_id, benchmark_symbols, time_period);
        
        // Check cache first
        if let Some(cached) = self.get_cached_comparison(&cache_key).await {
            return Ok(cached.result);
        }

        // Get user metrics
        let user_metrics = {
            let metrics = self.user_metrics.read().await;
            metrics.get(user_id).cloned()
                .ok_or_else(|| RiskError::UserNotFound(*user_id))?
        };

        // Get benchmark data
        let benchmark_data = self.benchmark_manager
            .get_multiple_benchmarks(benchmark_symbols, 365)
            .await?;

        let mut benchmark_comparisons = Vec::new();

        for (symbol, benchmark) in benchmark_data {
            // Calculate benchmark return for the same time period
            let benchmark_return = self.calculate_benchmark_return_for_period(&benchmark, &time_period);
            
            // Create dummy user returns for comparison (in real implementation, get from user data)
            let user_returns = vec![user_metrics.daily_return_average; 30]; // Simplified
            let benchmark_returns: Vec<Decimal> = benchmark.returns
                .iter()
                .take(30)
                .map(|r| r.return_percentage / Decimal::from(100))
                .collect();

            // Calculate comparison metrics
            let correlation = self.benchmark_manager
                .calculate_correlation(&user_returns, &benchmark_returns)
                .unwrap_or(Decimal::ZERO);

            let beta = self.benchmark_manager
                .calculate_beta(&user_returns, &benchmark_returns)
                .unwrap_or(Decimal::ONE);

            let alpha = self.benchmark_manager
                .calculate_alpha(
                    user_metrics.total_return_percentage / Decimal::from(100),
                    benchmark_return,
                    beta,
                    self.risk_free_rate,
                );

            let tracking_error = self.benchmark_manager
                .calculate_tracking_error(&user_returns, &benchmark_returns)
                .unwrap_or(Decimal::ZERO);

            benchmark_comparisons.push(BenchmarkComparison {
                benchmark_name: benchmark.name,
                user_return: user_metrics.total_return_percentage / Decimal::from(100),
                benchmark_return,
                alpha,
                beta,
                correlation,
                tracking_error,
            });
        }

        // Calculate relative performance score
        let relative_performance_score = self.calculate_relative_performance_score(&benchmark_comparisons);

        // Calculate percentile rank
        let percentile_rank = self.calculate_percentile_rank(user_id, &time_period).await?;

        let comparison = PerformanceComparison {
            user_metrics,
            benchmark_comparisons,
            relative_performance_score,
            percentile_rank: Some(percentile_rank),
        };

        // Cache the result
        self.cache_comparison(cache_key, comparison.clone(), 60).await; // 1 hour TTL

        Ok(comparison)
    }

    /// Generate anonymized performance leaderboard
    pub async fn generate_leaderboard(
        &self,
        config: LeaderboardConfig,
    ) -> Result<Vec<AnonymizedPerformance>, RiskError> {
        let user_metrics = self.user_metrics.read().await;
        let mut eligible_users = Vec::new();

        // Filter users based on criteria
        for (user_id, metrics) in user_metrics.iter() {
            // Check if user meets minimum requirements
            if let Some(min_trades) = config.min_trades {
                if metrics.total_trades < min_trades {
                    continue;
                }
            }

            if let Some(min_portfolio) = config.min_portfolio_value {
                if metrics.current_portfolio_value < min_portfolio {
                    continue;
                }
            }

            // Check if user has opted into public comparisons
            if !self.anonymization_engine.is_user_public(user_id).await {
                continue;
            }

            // Categorize user
            let category = self.categorize_user_performance(metrics);

            // Filter by category if specified
            if let Some(ref filter_category) = config.category_filter {
                if !self.matches_category(&category, filter_category) {
                    continue;
                }
            }

            eligible_users.push((*user_id, metrics.clone(), category));
        }

        // Sort by the specified metric
        eligible_users.sort_by(|a, b| {
            let metric_a = self.extract_metric_value(&a.1, &config.metric);
            let metric_b = self.extract_metric_value(&b.1, &config.metric);
            
            // Sort in descending order (higher is better for most metrics)
            match config.metric {
                LeaderboardMetric::MaxDrawdown => metric_a.cmp(&metric_b), // Lower is better for drawdown
                _ => metric_b.cmp(&metric_a),
            }
        });

        // Take top N users
        let top_users = eligible_users.into_iter().take(config.limit);

        // Create anonymized results
        let mut leaderboard = Vec::new();
        let total_users = user_metrics.len() as f64;

        for (rank, (user_id, metrics, category)) in top_users.enumerate() {
            let anonymous_id = self.anonymization_engine.get_anonymous_id(&user_id).await;
            let percentile_rank = Decimal::from(100) - (Decimal::from(rank) / Decimal::try_from(total_users).unwrap_or(Decimal::ONE)) * Decimal::from(100);
            let risk_adjusted_score = self.calculate_risk_adjusted_score(&metrics);

            leaderboard.push(AnonymizedPerformance {
                anonymous_id,
                performance_metrics: metrics,
                percentile_rank,
                category,
                risk_adjusted_score,
            });
        }

        Ok(leaderboard)
    }

    /// Calculate percentile rank for a user
    pub async fn calculate_percentile_rank(
        &self,
        user_id: &Uuid,
        _time_period: &TimePeriod,
    ) -> Result<Decimal, RiskError> {
        let user_metrics = self.user_metrics.read().await;
        
        let user_metric = user_metrics.get(user_id)
            .ok_or_else(|| RiskError::UserNotFound(*user_id))?;

        let user_return = user_metric.total_return_percentage;
        
        // Count users with lower returns
        let mut lower_count = 0;
        let mut total_count = 0;

        for metrics in user_metrics.values() {
            if metrics.total_return_percentage < user_return {
                lower_count += 1;
            }
            total_count += 1;
        }

        if total_count == 0 {
            return Ok(Decimal::ZERO);
        }

        let percentile = (Decimal::from(lower_count) / Decimal::from(total_count)) * Decimal::from(100);
        Ok(percentile)
    }

    // Private helper methods
    fn calculate_benchmark_return_for_period(&self, benchmark: &crate::analytics::performance_metrics::BenchmarkData, _time_period: &TimePeriod) -> Decimal {
        if benchmark.returns.is_empty() {
            return Decimal::ZERO;
        }

        // For simplicity, calculate total return over the period
        let first_price = benchmark.returns.first().unwrap().price;
        let last_price = benchmark.returns.last().unwrap().price;

        if first_price == Decimal::ZERO {
            return Decimal::ZERO;
        }

        ((last_price - first_price) / first_price) * Decimal::from(100)
    }

    fn calculate_relative_performance_score(&self, comparisons: &[BenchmarkComparison]) -> Decimal {
        if comparisons.is_empty() {
            return Decimal::ZERO;
        }

        // Calculate weighted average of alpha values
        let total_alpha: Decimal = comparisons.iter().map(|c| c.alpha).sum();
        total_alpha / Decimal::from(comparisons.len())
    }

    fn categorize_user_performance(&self, metrics: &PerformanceMetrics) -> PerformanceCategory {
        // Categorize based on risk and trading patterns
        let volatility = metrics.daily_return_volatility;
        let trade_frequency = metrics.total_trades as f64 / 365.0; // trades per day

        if trade_frequency > 1.0 {
            PerformanceCategory::HighFrequency
        } else if trade_frequency < 0.1 {
            PerformanceCategory::LongTerm
        } else if volatility < Decimal::from_str("0.01").unwrap() {
            PerformanceCategory::Conservative
        } else if volatility > Decimal::from_str("0.05").unwrap() {
            PerformanceCategory::Aggressive
        } else {
            PerformanceCategory::Moderate
        }
    }

    fn matches_category(&self, user_category: &PerformanceCategory, filter_category: &PerformanceCategory) -> bool {
        std::mem::discriminant(user_category) == std::mem::discriminant(filter_category)
    }

    fn extract_metric_value(&self, metrics: &PerformanceMetrics, metric: &LeaderboardMetric) -> Decimal {
        match metric {
            LeaderboardMetric::TotalReturn => metrics.total_return_percentage,
            LeaderboardMetric::SharpeRatio => metrics.sharpe_ratio,
            LeaderboardMetric::SortinoRatio => metrics.sortino_ratio,
            LeaderboardMetric::MaxDrawdown => metrics.max_drawdown,
            LeaderboardMetric::WinRate => metrics.win_rate,
            LeaderboardMetric::ProfitFactor => metrics.profit_factor,
            LeaderboardMetric::RiskAdjustedReturn => self.calculate_risk_adjusted_score(metrics),
        }
    }

    fn calculate_risk_adjusted_score(&self, metrics: &PerformanceMetrics) -> Decimal {
        // Combine multiple risk-adjusted metrics into a single score
        let sharpe_weight = Decimal::from_str("0.4").unwrap();
        let sortino_weight = Decimal::from_str("0.3").unwrap();
        let return_weight = Decimal::from_str("0.2").unwrap();
        let drawdown_weight = Decimal::from_str("0.1").unwrap();

        let normalized_sharpe = (metrics.sharpe_ratio + Decimal::from(2)) / Decimal::from(4); // Normalize to 0-1
        let normalized_sortino = (metrics.sortino_ratio + Decimal::from(2)) / Decimal::from(4);
        let normalized_return = metrics.total_return_percentage / Decimal::from(100);
        let normalized_drawdown = (Decimal::from(50) - metrics.max_drawdown) / Decimal::from(50);

        sharpe_weight * normalized_sharpe.max(Decimal::ZERO).min(Decimal::ONE) +
        sortino_weight * normalized_sortino.max(Decimal::ZERO).min(Decimal::ONE) +
        return_weight * normalized_return.max(Decimal::ZERO).min(Decimal::ONE) +
        drawdown_weight * normalized_drawdown.max(Decimal::ZERO).min(Decimal::ONE)
    }

    async fn get_cached_comparison(&self, cache_key: &str) -> Option<CachedComparison> {
        let cache = self.comparison_cache.read().await;
        if let Some(cached) = cache.get(cache_key) {
            let age = Utc::now().signed_duration_since(cached.cached_at);
            if age.num_minutes() < cached.ttl_minutes as i64 {
                return Some(cached.clone());
            }
        }
        None
    }

    async fn cache_comparison(&self, cache_key: String, result: PerformanceComparison, ttl_minutes: u64) {
        let mut cache = self.comparison_cache.write().await;
        cache.insert(cache_key, CachedComparison {
            result,
            cached_at: Utc::now(),
            ttl_minutes,
        });
    }
}

/// Cohort criteria for grouping users
#[derive(Debug, Clone)]
pub enum CohortCriteria {
    ByPortfolioSize,
    ByTradingFrequency,
    ByRiskProfile,
}
