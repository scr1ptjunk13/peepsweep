use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{debug, error, info, warn};

use crate::analytics::data_aggregation_engine::{AggregationResult, PerformanceAggregation};
use crate::risk_management::RiskError;
use uuid::Uuid as UserId;

/// Competitive benchmark engine for comparing user performance against market standards
#[derive(Debug)]
pub struct CompetitiveBenchmarkEngine {
    benchmark_cache: Arc<RwLock<HashMap<String, BenchmarkData>>>,
    benchmark_stats: Arc<RwLock<BenchmarkStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkData {
    pub benchmark_id: String,
    pub name: String,
    pub category: BenchmarkCategory,
    pub performance_metrics: BenchmarkMetrics,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BenchmarkCategory {
    MarketIndex,
    DeFiProtocol,
    TradingStrategy,
    PeerGroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub total_return: Decimal,
    pub volatility: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub user_id: UserId,
    pub comparison_id: String,
    pub user_performance: UserPerformanceSnapshot,
    pub benchmark_comparisons: Vec<IndividualBenchmarkComparison>,
    pub overall_ranking: OverallRanking,
    pub insights: Vec<PerformanceInsight>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPerformanceSnapshot {
    pub total_return: Decimal,
    pub volatility: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndividualBenchmarkComparison {
    pub benchmark_id: String,
    pub benchmark_name: String,
    pub return_difference: Decimal,
    pub relative_performance: RelativePerformance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelativePerformance {
    SignificantlyOutperforming,
    Outperforming,
    InLine,
    Underperforming,
    SignificantlyUnderperforming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallRanking {
    pub percentile: Decimal,
    pub rank: u32,
    pub total_participants: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceInsight {
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub impact_level: ImpactLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    StrengthIdentification,
    WeaknessIdentification,
    RiskWarning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactLevel {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub total_comparisons: u64,
    pub active_benchmarks: u64,
    pub average_comparison_time_ms: f64,
}

impl Default for BenchmarkStats {
    fn default() -> Self {
        Self {
            total_comparisons: 0,
            active_benchmarks: 0,
            average_comparison_time_ms: 0.0,
        }
    }
}

impl CompetitiveBenchmarkEngine {
    pub fn new() -> Self {
        Self {
            benchmark_cache: Arc::new(RwLock::new(HashMap::new())),
            benchmark_stats: Arc::new(RwLock::new(BenchmarkStats::default())),
        }
    }

    pub async fn initialize_default_benchmarks(&self) -> Result<(), RiskError> {
        info!("Initializing default benchmark data");

        let benchmarks = vec![
            BenchmarkData {
                benchmark_id: "crypto_market_index".to_string(),
                name: "Crypto Market Index".to_string(),
                category: BenchmarkCategory::MarketIndex,
                performance_metrics: BenchmarkMetrics {
                    total_return: Decimal::new(125, 1),
                    volatility: Decimal::new(182, 1),
                    sharpe_ratio: Decimal::new(85, 2),
                    max_drawdown: Decimal::new(-153, 1),
                },
                last_updated: Utc::now(),
            },
            BenchmarkData {
                benchmark_id: "defi_protocol_index".to_string(),
                name: "DeFi Protocol Index".to_string(),
                category: BenchmarkCategory::DeFiProtocol,
                performance_metrics: BenchmarkMetrics {
                    total_return: Decimal::new(258, 1),
                    volatility: Decimal::new(357, 1),
                    sharpe_ratio: Decimal::new(72, 2),
                    max_drawdown: Decimal::new(-284, 1),
                },
                last_updated: Utc::now(),
            },
        ];

        let mut cache = self.benchmark_cache.write().await;
        for benchmark in benchmarks {
            cache.insert(benchmark.benchmark_id.clone(), benchmark);
        }

        let mut stats = self.benchmark_stats.write().await;
        stats.active_benchmarks = cache.len() as u64;

        info!("Initialized {} default benchmarks", cache.len());
        Ok(())
    }

    pub async fn compare_performance(
        &self,
        user_id: UserId,
        user_performance: &PerformanceAggregation,
    ) -> Result<BenchmarkComparison, RiskError> {
        let start_time = std::time::Instant::now();
        info!("Starting benchmark comparison for user {}", user_id);

        let user_snapshot = UserPerformanceSnapshot {
            total_return: user_performance.total_return,
            volatility: user_performance.volatility,
            sharpe_ratio: user_performance.sharpe_ratio,
            max_drawdown: user_performance.max_drawdown,
        };

        let benchmarks = self.get_all_benchmarks().await;
        let mut benchmark_comparisons = Vec::new();

        for benchmark in &benchmarks {
            let return_difference = user_snapshot.total_return - benchmark.performance_metrics.total_return;
            
            let relative_performance = if return_difference > Decimal::from(5) {
                RelativePerformance::SignificantlyOutperforming
            } else if return_difference > Decimal::from(0) {
                RelativePerformance::Outperforming
            } else if return_difference > Decimal::from(-5) {
                RelativePerformance::InLine
            } else if return_difference > Decimal::from(-10) {
                RelativePerformance::Underperforming
            } else {
                RelativePerformance::SignificantlyUnderperforming
            };

            benchmark_comparisons.push(IndividualBenchmarkComparison {
                benchmark_id: benchmark.benchmark_id.clone(),
                benchmark_name: benchmark.name.clone(),
                return_difference,
                relative_performance,
            });
        }

        let overall_ranking = self.calculate_overall_ranking(&benchmark_comparisons).await;
        let insights = self.generate_insights(&user_snapshot, &benchmark_comparisons).await;

        let comparison_time = start_time.elapsed().as_millis() as u64;
        self.update_stats(comparison_time).await;

        Ok(BenchmarkComparison {
            user_id,
            comparison_id: Uuid::new_v4().to_string(),
            user_performance: user_snapshot,
            benchmark_comparisons,
            overall_ranking,
            insights,
            created_at: Utc::now(),
        })
    }

    pub async fn get_benchmark_stats(&self) -> BenchmarkStats {
        (*self.benchmark_stats.read().await).clone()
    }

    async fn get_all_benchmarks(&self) -> Vec<BenchmarkData> {
        let cache = self.benchmark_cache.read().await;
        cache.values().cloned().collect()
    }

    async fn calculate_overall_ranking(&self, comparisons: &[IndividualBenchmarkComparison]) -> OverallRanking {
        let outperforming_count = comparisons.iter()
            .filter(|c| matches!(c.relative_performance, RelativePerformance::Outperforming | RelativePerformance::SignificantlyOutperforming))
            .count();
        
        let percentile = (outperforming_count as f64 / comparisons.len() as f64) * 100.0;

        OverallRanking {
            percentile: Decimal::from_f64_retain(percentile).unwrap_or_default(),
            rank: 25,
            total_participants: 1000,
        }
    }

    async fn generate_insights(&self, user_performance: &UserPerformanceSnapshot, _comparisons: &[IndividualBenchmarkComparison]) -> Vec<PerformanceInsight> {
        let mut insights = Vec::new();

        if user_performance.sharpe_ratio > Decimal::new(15, 1) {
            insights.push(PerformanceInsight {
                insight_type: InsightType::StrengthIdentification,
                title: "Excellent Risk-Adjusted Returns".to_string(),
                description: "Your Sharpe ratio indicates superior risk-adjusted performance.".to_string(),
                impact_level: ImpactLevel::High,
            });
        }

        if user_performance.volatility > Decimal::from(20) {
            insights.push(PerformanceInsight {
                insight_type: InsightType::RiskWarning,
                title: "High Portfolio Volatility".to_string(),
                description: "Your portfolio shows elevated risk levels.".to_string(),
                impact_level: ImpactLevel::Medium,
            });
        }

        insights
    }

    async fn update_stats(&self, comparison_time_ms: u64) {
        let mut stats = self.benchmark_stats.write().await;
        stats.total_comparisons += 1;
        
        let total_time = stats.average_comparison_time_ms * (stats.total_comparisons - 1) as f64;
        stats.average_comparison_time_ms = (total_time + comparison_time_ms as f64) / stats.total_comparisons as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_benchmark_initialization() {
        let engine = CompetitiveBenchmarkEngine::new();
        let result = engine.initialize_default_benchmarks().await;
        assert!(result.is_ok());
        
        let stats = engine.get_benchmark_stats().await;
        assert!(stats.active_benchmarks > 0);
    }
}
