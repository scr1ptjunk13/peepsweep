use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::prelude::*;
use std::str::FromStr;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

use crate::user_retention::performance_analytics::user_analyzer::{UserPerformanceMetrics, TradingPattern, RiskTolerance};
use crate::analytics::benchmark_integration::BenchmarkDataManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub user_id: Uuid,
    pub user_return: Decimal,
    pub benchmark_return: Decimal,
    pub alpha: f64, // Excess return over benchmark
    pub beta: f64,  // Correlation with benchmark
    pub tracking_error: f64,
    pub information_ratio: f64,
    pub benchmark_name: String,
    pub time_period: String,
    pub outperformed: bool,
    pub calculated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerComparison {
    pub user_id: Uuid,
    pub user_percentile: f64, // 0-100, higher is better
    pub cohort_size: u32,
    pub user_rank: u32,
    pub average_return: Decimal,
    pub median_return: Decimal,
    pub top_quartile_return: Decimal,
    pub user_return: Decimal,
    pub risk_adjusted_rank: u32,
    pub risk_adjusted_percentile: f64,
    pub cohort_criteria: CohortCriteria,
    pub calculated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohortCriteria {
    pub risk_tolerance: Option<RiskTolerance>,
    pub portfolio_size_range: Option<(Decimal, Decimal)>,
    pub trading_frequency_range: Option<(f64, f64)>,
    pub time_period: Duration,
    pub min_trades: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketComparison {
    pub user_id: Uuid,
    pub user_metrics: UserPerformanceMetrics,
    pub market_benchmarks: Vec<BenchmarkComparison>,
    pub peer_comparison: PeerComparison,
    pub dex_performance: DexPerformanceComparison,
    pub overall_score: f64, // 0-100 composite score
    pub performance_category: PerformanceCategory,
    pub calculated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexPerformanceComparison {
    pub user_preferred_dexes: Vec<String>,
    pub dex_performance_scores: HashMap<String, DexScore>,
    pub optimization_opportunities: Vec<DexOptimization>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexScore {
    pub dex_name: String,
    pub user_trades_count: u32,
    pub average_slippage: Decimal,
    pub average_gas_cost: Decimal,
    pub success_rate: f64,
    pub average_execution_time: Duration,
    pub cost_efficiency_score: f64, // 0-100
    pub liquidity_score: f64, // 0-100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexOptimization {
    pub current_dex: String,
    pub recommended_dex: String,
    pub potential_savings_percentage: Decimal,
    pub reason: String,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceCategory {
    TopPerformer,    // Top 10%
    AboveAverage,    // 60-90%
    Average,         // 40-60%
    BelowAverage,    // 10-40%
    Underperformer,  // Bottom 10%
}

pub struct ComparativeAnalytics {
    benchmark_manager: Arc<BenchmarkDataManager>,
    user_metrics_cache: Arc<RwLock<HashMap<Uuid, UserPerformanceMetrics>>>,
    peer_comparisons_cache: Arc<RwLock<HashMap<Uuid, PeerComparison>>>,
    benchmark_comparisons_cache: Arc<RwLock<HashMap<Uuid, Vec<BenchmarkComparison>>>>,
    dex_performance_cache: Arc<RwLock<HashMap<String, DexScore>>>,
}

impl ComparativeAnalytics {
    pub fn new(benchmark_manager: Arc<BenchmarkDataManager>) -> Self {
        Self {
            benchmark_manager,
            user_metrics_cache: Arc::new(RwLock::new(HashMap::new())),
            peer_comparisons_cache: Arc::new(RwLock::new(HashMap::new())),
            benchmark_comparisons_cache: Arc::new(RwLock::new(HashMap::new())),
            dex_performance_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Compare user performance against market benchmarks
    pub async fn compare_with_benchmarks(
        &self,
        user_metrics: &UserPerformanceMetrics,
        benchmarks: Vec<String>,
        time_period: Duration,
    ) -> Result<Vec<BenchmarkComparison>, Box<dyn std::error::Error + Send + Sync>> {
        let mut comparisons = Vec::new();

        for benchmark in benchmarks {
            let user_return_decimal = user_metrics.total_return;
            let dex_performance_decimal = Decimal::from(8); // Placeholder DEX performance
            let benchmark_return = dex_performance_decimal; // Use DEX performance as benchmark return

            let alpha = self.calculate_alpha(user_return_decimal, dex_performance_decimal)?;
            let beta = 1.0; // Placeholder beta calculation
            let tracking_error = 2.0; // Placeholder tracking error
            let information_ratio = if tracking_error != 0.0 {
                alpha / tracking_error
            } else {
                0.0
            };

            let comparison = BenchmarkComparison {
                user_id: user_metrics.user_id,
                user_return: user_metrics.total_return,
                benchmark_return,
                alpha,
                beta,
                tracking_error,
                information_ratio,
                benchmark_name: benchmark.clone(),
                time_period: format!("{} days", time_period.num_days()),
                outperformed: user_metrics.total_return > benchmark_return,
                calculated_at: Utc::now(),
            };

            comparisons.push(comparison);
        }

        // Cache the results
        let mut cache = self.benchmark_comparisons_cache.write().await;
        cache.insert(user_metrics.user_id, comparisons.clone());

        Ok(comparisons)
    }

    /// Compare user performance with peers in similar cohorts
    pub async fn compare_with_peers(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
        cohort_criteria: CohortCriteria,
    ) -> Result<PeerComparison, Box<dyn std::error::Error + Send + Sync>> {
        // Get peer metrics based on cohort criteria
        let peer_metrics = self.get_peer_metrics(&cohort_criteria).await?;

        if peer_metrics.is_empty() {
            return Ok(self.create_empty_peer_comparison(user_metrics.user_id, cohort_criteria));
        }

        // Calculate peer statistics
        let mut peer_returns: Vec<Decimal> = peer_metrics.iter().map(|m| m.total_return).collect();
        peer_returns.sort();

        let cohort_size = peer_returns.len() as u32;
        let average_return = peer_returns.iter().sum::<Decimal>() / Decimal::from(cohort_size);
        let median_return = if cohort_size % 2 == 0 {
            let mid = cohort_size as usize / 2;
            (peer_returns[mid - 1] + peer_returns[mid]) / Decimal::from(2)
        } else {
            peer_returns[cohort_size as usize / 2]
        };

        let top_quartile_index = ((cohort_size as f64 * 0.75) as usize).min(peer_returns.len() - 1);
        let top_quartile_return = peer_returns[top_quartile_index];

        // Calculate user's rank and percentile
        let user_rank = peer_returns.iter()
            .position(|&r| r >= user_metrics.total_return)
            .unwrap_or(peer_returns.len()) as u32 + 1;

        let user_percentile = if cohort_size > 0 {
            ((cohort_size - user_rank + 1) as f64 / cohort_size as f64) * 100.0
        } else {
            0.0
        };

        // Calculate risk-adjusted rankings using Sharpe ratio
        let mut risk_adjusted_metrics: Vec<_> = peer_metrics.iter()
            .map(|m| (m.user_id, m.sharpe_ratio))
            .collect();
        risk_adjusted_metrics.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let risk_adjusted_rank = risk_adjusted_metrics.iter()
            .position(|(id, _)| *id == user_metrics.user_id)
            .unwrap_or(risk_adjusted_metrics.len()) as u32 + 1;

        let risk_adjusted_percentile = if cohort_size > 0 {
            ((cohort_size - risk_adjusted_rank + 1) as f64 / cohort_size as f64) * 100.0
        } else {
            0.0
        };

        let comparison = PeerComparison {
            user_id: user_metrics.user_id,
            user_percentile,
            cohort_size,
            user_rank,
            average_return,
            median_return,
            top_quartile_return,
            user_return: user_metrics.total_return,
            risk_adjusted_rank,
            risk_adjusted_percentile,
            cohort_criteria,
            calculated_at: Utc::now(),
        };

        // Cache the results
        let mut cache = self.peer_comparisons_cache.write().await;
        cache.insert(user_metrics.user_id, comparison.clone());

        Ok(comparison)
    }

    /// Analyze DEX performance for user's trading patterns
    pub async fn analyze_dex_performance(
        &self,
        user_id: Uuid,
        trading_pattern: &TradingPattern,
    ) -> Result<DexPerformanceComparison, Box<dyn std::error::Error + Send + Sync>> {
        let mut dex_scores = HashMap::new();
        let mut optimization_opportunities = Vec::new();

        // Analyze each DEX the user has traded on
        for dex_name in &trading_pattern.preferred_dexes {
            let dex_score = self.calculate_dex_score(user_id, dex_name).await?;
            dex_scores.insert(dex_name.clone(), dex_score);
        }

        // Find optimization opportunities
        for (current_dex, current_score) in &dex_scores {
            let better_alternatives = self.find_better_dex_alternatives(
                current_dex,
                current_score,
                &trading_pattern.preferred_tokens,
                &trading_pattern.preferred_chains,
            ).await?;

            for (recommended_dex, potential_savings, reason, confidence) in better_alternatives {
                optimization_opportunities.push(DexOptimization {
                    current_dex: current_dex.clone(),
                    recommended_dex,
                    potential_savings_percentage: potential_savings,
                    reason,
                    confidence_score: confidence,
                });
            }
        }

        Ok(DexPerformanceComparison {
            user_preferred_dexes: trading_pattern.preferred_dexes.clone(),
            dex_performance_scores: dex_scores,
            optimization_opportunities,
        })
    }

    /// Generate comprehensive market comparison
    pub async fn generate_market_comparison(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
        benchmarks: Vec<String>,
        time_period: Duration,
    ) -> Result<MarketComparison, Box<dyn std::error::Error + Send + Sync>> {
        // Get benchmark comparisons
        let market_benchmarks = self.compare_with_benchmarks(
            user_metrics,
            benchmarks,
            time_period,
        ).await?;

        // Get peer comparison
        let cohort_criteria = CohortCriteria {
            risk_tolerance: Some(trading_pattern.risk_tolerance.clone()),
            portfolio_size_range: Some((
                user_metrics.portfolio_value * Decimal::from_str("0.5").unwrap(),
                user_metrics.portfolio_value * Decimal::from_str("2.0").unwrap(),
            )),
            trading_frequency_range: Some((
                user_metrics.trade_frequency * 0.5,
                user_metrics.trade_frequency * 2.0,
            )),
            time_period,
            min_trades: 10,
        };

        let peer_comparison = self.compare_with_peers(
            user_metrics,
            trading_pattern,
            cohort_criteria,
        ).await?;

        // Get DEX performance analysis
        let dex_performance = self.analyze_dex_performance(
            user_metrics.user_id,
            trading_pattern,
        ).await?;

        // Calculate overall performance score
        let overall_score = self.calculate_overall_score(
            user_metrics,
            &market_benchmarks,
            &peer_comparison,
        );

        // Determine performance category
        let performance_category = self.determine_performance_category(peer_comparison.user_percentile);

        Ok(MarketComparison {
            user_id: user_metrics.user_id,
            user_metrics: user_metrics.clone(),
            market_benchmarks,
            peer_comparison,
            dex_performance,
            overall_score,
            performance_category,
            calculated_at: Utc::now(),
        })
    }

    // Private helper methods
    async fn get_peer_metrics(
        &self,
        _criteria: &CohortCriteria,
    ) -> Result<Vec<UserPerformanceMetrics>, Box<dyn std::error::Error + Send + Sync>> {
        // This would query your database for users matching the cohort criteria
        // For now, return empty vector - implement based on your data storage
        Ok(vec![])
    }

    fn create_empty_peer_comparison(&self, user_id: Uuid, cohort_criteria: CohortCriteria) -> PeerComparison {
        PeerComparison {
            user_id,
            user_percentile: 50.0,
            cohort_size: 0,
            user_rank: 1,
            average_return: Decimal::ZERO,
            median_return: Decimal::ZERO,
            top_quartile_return: Decimal::ZERO,
            user_return: Decimal::ZERO,
            risk_adjusted_rank: 1,
            risk_adjusted_percentile: 50.0,
            cohort_criteria,
            calculated_at: Utc::now(),
        }
    }

    fn calculate_alpha(
        &self,
        user_return: Decimal,
        benchmark_return: Decimal,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        Ok((user_return - benchmark_return).to_f64().unwrap_or(0.0))
    }

    async fn calculate_beta(&self, user_metrics: &UserPerformanceMetrics) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // Implement beta calculation based on correlation with benchmark
        Ok(1.0) // Placeholder
    }

    async fn calculate_tracking_error(
        &self,
        _user_metrics: &UserPerformanceMetrics,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        // Implement tracking error calculation
        Ok(0.05) // Placeholder - 5% tracking error
    }

    async fn calculate_dex_score(
        &self,
        _user_id: Uuid,
        dex_name: &str,
    ) -> Result<DexScore, Box<dyn std::error::Error + Send + Sync>> {
        // This would analyze user's historical trades on this DEX
        // For now, return a placeholder score
        Ok(DexScore {
            dex_name: dex_name.to_string(),
            user_trades_count: 10,
            average_slippage: Decimal::from_str("0.005").unwrap(), // 0.5%
            average_gas_cost: Decimal::from(50), // $50 USD
            success_rate: 95.0,
            average_execution_time: Duration::seconds(30),
            cost_efficiency_score: 75.0,
            liquidity_score: 80.0,
        })
    }

    async fn find_better_dex_alternatives(
        &self,
        _current_dex: &str,
        _current_score: &DexScore,
        _preferred_tokens: &[String],
        _preferred_chains: &[u64],
    ) -> Result<Vec<(String, Decimal, String, f64)>, Box<dyn std::error::Error + Send + Sync>> {
        // This would analyze alternative DEXes and find better options
        // Return format: (dex_name, potential_savings_percentage, reason, confidence_score)
        Ok(vec![
            ("Uniswap V3".to_string(), Decimal::from_str("0.15").unwrap(), "Lower gas costs".to_string(), 0.85),
            ("Curve".to_string(), Decimal::from_str("0.08").unwrap(), "Better rates for stablecoins".to_string(), 0.92),
        ])
    }

    fn calculate_overall_score(
        &self,
        user_metrics: &UserPerformanceMetrics,
        benchmark_comparisons: &[BenchmarkComparison],
        peer_comparison: &PeerComparison,
    ) -> f64 {
        // Weighted composite score
        let return_score = if user_metrics.total_return > Decimal::ZERO {
            user_metrics.total_return.to_f64().unwrap_or(0.0) * 100.0
        } else {
            0.0
        }.min(100.0);

        let sharpe_score = ((user_metrics.sharpe_ratio + 2.0) / 4.0 * 100.0).max(0.0).min(100.0);
        let peer_score = peer_comparison.user_percentile;
        
        let benchmark_score = if !benchmark_comparisons.is_empty() {
            benchmark_comparisons.iter()
                .map(|b| if b.outperformed { 75.0 } else { 25.0 })
                .sum::<f64>() / benchmark_comparisons.len() as f64
        } else {
            50.0
        };

        // Weighted average: 30% returns, 25% risk-adjusted, 25% peer comparison, 20% benchmark
        (return_score * 0.3 + sharpe_score * 0.25 + peer_score * 0.25 + benchmark_score * 0.2)
            .max(0.0)
            .min(100.0)
    }

    fn determine_performance_category(&self, percentile: f64) -> PerformanceCategory {
        match percentile {
            p if p >= 90.0 => PerformanceCategory::TopPerformer,
            p if p >= 60.0 => PerformanceCategory::AboveAverage,
            p if p >= 40.0 => PerformanceCategory::Average,
            p if p >= 10.0 => PerformanceCategory::BelowAverage,
            _ => PerformanceCategory::Underperformer,
        }
    }

    /// Get cached benchmark comparisons
    pub async fn get_cached_benchmark_comparisons(&self, user_id: Uuid) -> Option<Vec<BenchmarkComparison>> {
        let cache = self.benchmark_comparisons_cache.read().await;
        cache.get(&user_id).cloned()
    }

    /// Get cached peer comparison
    pub async fn get_cached_peer_comparison(&self, user_id: Uuid) -> Option<PeerComparison> {
        let cache = self.peer_comparisons_cache.read().await;
        cache.get(&user_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_comparative_analytics_creation() {
        // Mock benchmark manager would be created here
        // This is a placeholder for actual tests
    }

    #[tokio::test]
    async fn test_benchmark_comparison() {
        // Test benchmark comparison logic
    }

    #[tokio::test]
    async fn test_peer_comparison() {
        // Test peer comparison logic
    }

    #[tokio::test]
    async fn test_performance_category_determination() {
        // Test performance category logic
    }
}
