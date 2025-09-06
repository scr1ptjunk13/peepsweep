use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::{Decimal, prelude::FromStr};
use std::str::FromStr as StdFromStr;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

use crate::user_retention::performance_analytics::user_analyzer::{UserPerformanceMetrics, TradingPattern, RiskTolerance, StrategyType};
use crate::user_retention::performance_analytics::comparative_analytics::{MarketComparison, DexOptimization, PerformanceCategory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingInsight {
    pub insight_id: Uuid,
    pub user_id: Uuid,
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub impact_score: f64, // 0-100, higher = more impactful
    pub confidence_score: f64, // 0-100, higher = more confident
    pub priority: InsightPriority,
    pub actionable_steps: Vec<String>,
    pub expected_improvement: Option<Decimal>,
    pub risk_level: RiskLevel,
    pub generated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    PerformanceImprovement,
    RiskReduction,
    CostOptimization,
    TimingOptimization,
    DiversificationSuggestion,
    StrategyAdjustment,
    MarketOpportunity,
    BehavioralPattern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecommendation {
    pub user_id: Uuid,
    pub current_performance: UserPerformanceMetrics,
    pub target_improvements: Vec<ImprovementTarget>,
    pub strategy_adjustments: Vec<StrategyAdjustment>,
    pub risk_warnings: Vec<RiskWarning>,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementTarget {
    pub metric_name: String,
    pub current_value: Decimal,
    pub target_value: Decimal,
    pub improvement_percentage: Decimal,
    pub estimated_timeframe: Duration,
    pub difficulty: Difficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyAdjustment {
    pub adjustment_type: AdjustmentType,
    pub description: String,
    pub rationale: String,
    pub expected_impact: String,
    pub implementation_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskWarning {
    pub warning_type: WarningType,
    pub severity: Severity,
    pub description: String,
    pub potential_impact: String,
    pub mitigation_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationOpportunity {
    pub opportunity_type: OpportunityType,
    pub description: String,
    pub potential_benefit: Decimal,
    pub implementation_effort: Effort,
    pub success_probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdjustmentType {
    IncreaseFrequency,
    DecreaseFrequency,
    ChangeDexPreference,
    AdjustPositionSizing,
    ImproveRiskManagement,
    OptimizeTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningType {
    HighRisk,
    PoorPerformance,
    OverConcentration,
    ExcessiveTrading,
    InsufficientDiversification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    CostReduction,
    PerformanceEnhancement,
    RiskOptimization,
    EfficiencyImprovement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effort {
    Low,
    Medium,
    High,
}

pub struct InsightsGenerator {
    insights_cache: Arc<RwLock<HashMap<Uuid, Vec<TradingInsight>>>>,
    recommendations_cache: Arc<RwLock<HashMap<Uuid, PerformanceRecommendation>>>,
}

impl InsightsGenerator {
    pub fn new() -> Self {
        Self {
            insights_cache: Arc::new(RwLock::new(HashMap::new())),
            recommendations_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate comprehensive trading insights for a user
    pub async fn generate_insights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
        market_comparison: &MarketComparison,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Generate performance-based insights
        insights.extend(self.generate_performance_insights(user_metrics, market_comparison).await?);

        // Generate risk-based insights
        insights.extend(self.generate_risk_insights(user_metrics, trading_pattern).await?);

        // Generate cost optimization insights
        insights.extend(self.generate_cost_optimization_insights(trading_pattern, &market_comparison.dex_performance).await?);

        // Generate timing insights
        insights.extend(self.generate_timing_insights(user_metrics, trading_pattern).await?);

        // Generate diversification insights
        insights.extend(self.generate_diversification_insights(trading_pattern).await?);

        // Generate behavioral insights
        insights.extend(self.generate_behavioral_insights(user_metrics, trading_pattern).await?);

        // Sort by impact score and priority
        insights.sort_by(|a, b| {
            b.impact_score.partial_cmp(&a.impact_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| match (&a.priority, &b.priority) {
                    (InsightPriority::Critical, _) => std::cmp::Ordering::Less,
                    (_, InsightPriority::Critical) => std::cmp::Ordering::Greater,
                    (InsightPriority::High, _) => std::cmp::Ordering::Less,
                    (_, InsightPriority::High) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                })
        });

        // Cache the results
        let mut cache = self.insights_cache.write().await;
        cache.insert(user_metrics.user_id, insights.clone());

        Ok(insights)
    }

    /// Generate performance improvement recommendations
    pub async fn generate_recommendations(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
        market_comparison: &MarketComparison,
    ) -> Result<PerformanceRecommendation, Box<dyn std::error::Error + Send + Sync>> {
        // Generate improvement targets
        let target_improvements = self.generate_improvement_targets(user_metrics, market_comparison).await?;

        // Generate strategy adjustments
        let strategy_adjustments = self.generate_strategy_adjustments(user_metrics, trading_pattern, market_comparison).await?;

        // Generate risk warnings
        let risk_warnings = self.generate_risk_warnings(user_metrics, trading_pattern).await?;

        // Generate optimization opportunities
        let optimization_opportunities = self.generate_optimization_opportunities(market_comparison).await?;

        let recommendation = PerformanceRecommendation {
            user_id: user_metrics.user_id,
            current_performance: user_metrics.clone(),
            target_improvements,
            strategy_adjustments,
            risk_warnings,
            optimization_opportunities,
            generated_at: Utc::now(),
        };

        // Cache the results
        let mut cache = self.recommendations_cache.write().await;
        cache.insert(user_metrics.user_id, recommendation.clone());

        Ok(recommendation)
    }

    // Private helper methods for generating specific types of insights

    async fn generate_performance_insights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        market_comparison: &MarketComparison,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Low win rate insight
        if user_metrics.win_rate < 50.0 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::PerformanceImprovement,
                title: "Low Win Rate Detected".to_string(),
                description: format!("Your current win rate is {:.1}%, which is below the typical 50-60% range for successful traders.", user_metrics.win_rate),
                recommendation: "Consider improving your entry and exit strategies, or reducing position sizes to minimize losses.".to_string(),
                impact_score: 85.0,
                confidence_score: 90.0,
                priority: InsightPriority::High,
                actionable_steps: vec![
                    "Review your entry criteria and tighten stop-loss levels".to_string(),
                    "Consider paper trading new strategies before implementing".to_string(),
                    "Analyze your losing trades to identify common patterns".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.15").unwrap()), // 15% improvement
                risk_level: RiskLevel::Medium,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(30)),
            });
        }

        // Underperformance vs peers
        if market_comparison.peer_comparison.user_percentile < 25.0 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::PerformanceImprovement,
                title: "Underperforming Peer Group".to_string(),
                description: format!("You're in the bottom 25% of similar traders ({}th percentile).", market_comparison.peer_comparison.user_percentile as u32),
                recommendation: "Consider adopting strategies used by top performers in your peer group.".to_string(),
                impact_score: 90.0,
                confidence_score: 85.0,
                priority: InsightPriority::Critical,
                actionable_steps: vec![
                    "Study top performer strategies in your risk category".to_string(),
                    "Consider reducing trading frequency to focus on quality".to_string(),
                    "Implement stricter risk management rules".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.25").unwrap()),
                risk_level: RiskLevel::Low,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(60)),
            });
        }

        Ok(insights)
    }

    async fn generate_risk_insights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // High volatility warning
        if user_metrics.volatility > Decimal::from_str("0.30").unwrap() {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::RiskReduction,
                title: "High Portfolio Volatility".to_string(),
                description: format!("Your portfolio volatility is {:.1}%, which is quite high and may indicate excessive risk.", user_metrics.volatility * Decimal::from(100)),
                recommendation: "Consider diversifying across more assets or reducing position sizes.".to_string(),
                impact_score: 80.0,
                confidence_score: 88.0,
                priority: InsightPriority::High,
                actionable_steps: vec![
                    "Diversify across more uncorrelated assets".to_string(),
                    "Reduce individual position sizes".to_string(),
                    "Consider adding stablecoin positions as a buffer".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.10").unwrap()),
                risk_level: RiskLevel::High,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(14)),
            });
        }

        // Poor Sharpe ratio
        if user_metrics.sharpe_ratio < 0.5 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::RiskReduction,
                title: "Poor Risk-Adjusted Returns".to_string(),
                description: format!("Your Sharpe ratio of {:.2} indicates poor risk-adjusted performance.", user_metrics.sharpe_ratio),
                recommendation: "Focus on improving returns while managing risk, or reduce risk exposure.".to_string(),
                impact_score: 75.0,
                confidence_score: 92.0,
                priority: InsightPriority::Medium,
                actionable_steps: vec![
                    "Implement stricter stop-loss rules".to_string(),
                    "Focus on higher probability trades".to_string(),
                    "Consider reducing leverage or position sizes".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.20").unwrap()),
                risk_level: RiskLevel::Medium,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(45)),
            });
        }

        Ok(insights)
    }

    async fn generate_cost_optimization_insights(
        &self,
        trading_pattern: &TradingPattern,
        dex_performance: &crate::user_retention::performance_analytics::comparative_analytics::DexPerformanceComparison,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // DEX optimization opportunities
        for optimization in &dex_performance.optimization_opportunities {
            if optimization.potential_savings_percentage > Decimal::from_str("0.05").unwrap() {
                insights.push(TradingInsight {
                    insight_id: Uuid::new_v4(),
                    user_id: Uuid::new_v4(), // This should be passed in
                    insight_type: InsightType::CostOptimization,
                    title: format!("Switch from {} to {}", optimization.current_dex, optimization.recommended_dex),
                    description: format!("You could save {:.1}% on trading costs by switching DEXes.", optimization.potential_savings_percentage * Decimal::from(100)),
                    recommendation: format!("Consider using {} instead of {} for better rates. {}", optimization.recommended_dex, optimization.current_dex, optimization.reason),
                    impact_score: ((optimization.potential_savings_percentage * Decimal::from(1000)).to_string().parse::<f64>().unwrap_or(0.0)).min(100.0),
                    confidence_score: optimization.confidence_score * 100.0,
                    priority: if optimization.potential_savings_percentage > Decimal::from_str("0.10").unwrap() {
                        InsightPriority::High
                    } else {
                        InsightPriority::Medium
                    },
                    actionable_steps: vec![
                        format!("Test small trades on {}", optimization.recommended_dex),
                        "Compare actual execution costs".to_string(),
                        "Gradually migrate larger trades if results are positive".to_string(),
                    ],
                    expected_improvement: Some(optimization.potential_savings_percentage),
                    risk_level: RiskLevel::Low,
                    generated_at: Utc::now(),
                    expires_at: Some(Utc::now() + Duration::days(21)),
                });
            }
        }

        Ok(insights)
    }

    async fn generate_timing_insights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Overtrading detection
        if user_metrics.trade_frequency > 10.0 { // More than 10 trades per day
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::TimingOptimization,
                title: "Potential Overtrading Detected".to_string(),
                description: format!("You're averaging {:.1} trades per day, which may be excessive.", user_metrics.trade_frequency),
                recommendation: "Consider reducing trading frequency to focus on higher-quality opportunities.".to_string(),
                impact_score: 70.0,
                confidence_score: 80.0,
                priority: InsightPriority::Medium,
                actionable_steps: vec![
                    "Set a daily trade limit".to_string(),
                    "Wait for higher-conviction setups".to_string(),
                    "Implement a cooling-off period between trades".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.15").unwrap()),
                risk_level: RiskLevel::Low,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(30)),
            });
        }

        Ok(insights)
    }

    async fn generate_diversification_insights(
        &self,
        trading_pattern: &TradingPattern,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Limited token diversity
        if trading_pattern.preferred_tokens.len() < 5 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: Uuid::new_v4(), // This should be passed in
                insight_type: InsightType::DiversificationSuggestion,
                title: "Limited Token Diversification".to_string(),
                description: format!("You're only trading {} different tokens, which may limit opportunities.", trading_pattern.preferred_tokens.len()),
                recommendation: "Consider expanding to more token pairs to capture diverse market opportunities.".to_string(),
                impact_score: 60.0,
                confidence_score: 75.0,
                priority: InsightPriority::Medium,
                actionable_steps: vec![
                    "Research tokens in different sectors (DeFi, Layer 1, etc.)".to_string(),
                    "Start with small positions in new tokens".to_string(),
                    "Monitor correlation between your holdings".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.10").unwrap()),
                risk_level: RiskLevel::Medium,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(60)),
            });
        }

        // Single chain concentration
        if trading_pattern.preferred_chains.len() == 1 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: Uuid::new_v4(), // This should be passed in
                insight_type: InsightType::DiversificationSuggestion,
                title: "Single Chain Concentration".to_string(),
                description: "You're only trading on one blockchain, missing cross-chain arbitrage opportunities.".to_string(),
                recommendation: "Consider expanding to other chains like Polygon, Arbitrum, or BSC for better opportunities.".to_string(),
                impact_score: 65.0,
                confidence_score: 70.0,
                priority: InsightPriority::Medium,
                actionable_steps: vec![
                    "Research gas costs and liquidity on other chains".to_string(),
                    "Start with small cross-chain transactions".to_string(),
                    "Look for arbitrage opportunities between chains".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.12").unwrap()),
                risk_level: RiskLevel::Medium,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(45)),
            });
        }

        Ok(insights)
    }

    async fn generate_behavioral_insights(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
    ) -> Result<Vec<TradingInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Negative streak warning
        if user_metrics.current_streak < -3 {
            insights.push(TradingInsight {
                insight_id: Uuid::new_v4(),
                user_id: user_metrics.user_id,
                insight_type: InsightType::BehavioralPattern,
                title: "Negative Trading Streak".to_string(),
                description: format!("You're on a {}-trade losing streak, which may affect decision-making.", user_metrics.current_streak.abs()),
                recommendation: "Consider taking a break or reducing position sizes to avoid emotional trading.".to_string(),
                impact_score: 85.0,
                confidence_score: 95.0,
                priority: InsightPriority::High,
                actionable_steps: vec![
                    "Take a 24-48 hour break from trading".to_string(),
                    "Review and analyze recent losing trades".to_string(),
                    "Reduce position sizes for the next few trades".to_string(),
                    "Consider paper trading to rebuild confidence".to_string(),
                ],
                expected_improvement: Some(Decimal::from_str("0.20").unwrap()),
                risk_level: RiskLevel::High,
                generated_at: Utc::now(),
                expires_at: Some(Utc::now() + Duration::days(7)),
            });
        }

        Ok(insights)
    }

    // Helper methods for generating recommendations

    async fn generate_improvement_targets(
        &self,
        user_metrics: &UserPerformanceMetrics,
        market_comparison: &MarketComparison,
    ) -> Result<Vec<ImprovementTarget>, Box<dyn std::error::Error + Send + Sync>> {
        let mut targets = Vec::new();

        // Win rate improvement
        if user_metrics.win_rate < 60.0 {
            targets.push(ImprovementTarget {
                metric_name: "Win Rate".to_string(),
                current_value: Decimal::try_from(user_metrics.win_rate).unwrap_or(Decimal::ZERO),
                target_value: Decimal::from(60),
                improvement_percentage: Decimal::from(60) - Decimal::try_from(user_metrics.win_rate).unwrap_or(Decimal::ZERO),
                estimated_timeframe: Duration::days(30),
                difficulty: Difficulty::Medium,
            });
        }

        // Sharpe ratio improvement
        if user_metrics.sharpe_ratio < 1.0 {
            let current_sharpe = Decimal::try_from(user_metrics.sharpe_ratio).unwrap_or(Decimal::ZERO);
            let target_sharpe = Decimal::from_str("1.0").unwrap();
            let improvement_percentage = if current_sharpe.is_zero() {
                Decimal::from(100) // 100% improvement needed from zero
            } else {
                (target_sharpe - current_sharpe) / current_sharpe * Decimal::from(100)
            };
            
            targets.push(ImprovementTarget {
                metric_name: "Sharpe Ratio".to_string(),
                current_value: current_sharpe,
                target_value: target_sharpe,
                improvement_percentage,
                estimated_timeframe: Duration::days(60),
                difficulty: Difficulty::Hard,
            });
        }

        Ok(targets)
    }

    async fn generate_strategy_adjustments(
        &self,
        user_metrics: &UserPerformanceMetrics,
        trading_pattern: &TradingPattern,
        _market_comparison: &MarketComparison,
    ) -> Result<Vec<StrategyAdjustment>, Box<dyn std::error::Error + Send + Sync>> {
        let mut adjustments = Vec::new();

        // Overtrading adjustment
        if user_metrics.trade_frequency > 5.0 {
            adjustments.push(StrategyAdjustment {
                adjustment_type: AdjustmentType::DecreaseFrequency,
                description: "Reduce trading frequency to improve quality".to_string(),
                rationale: "High frequency trading may be leading to lower quality decisions".to_string(),
                expected_impact: "Improved win rate and reduced transaction costs".to_string(),
                implementation_steps: vec![
                    "Set daily trade limits".to_string(),
                    "Implement stricter entry criteria".to_string(),
                    "Wait for higher-conviction setups".to_string(),
                ],
            });
        }

        Ok(adjustments)
    }

    async fn generate_risk_warnings(
        &self,
        user_metrics: &UserPerformanceMetrics,
        _trading_pattern: &TradingPattern,
    ) -> Result<Vec<RiskWarning>, Box<dyn std::error::Error + Send + Sync>> {
        let mut warnings = Vec::new();

        // High drawdown warning
        if user_metrics.max_drawdown > Decimal::from_str("0.20").unwrap() {
            warnings.push(RiskWarning {
                warning_type: WarningType::HighRisk,
                severity: Severity::High,
                description: format!("Maximum drawdown of {:.1}% is concerning", user_metrics.max_drawdown * Decimal::from(100)),
                potential_impact: "Large losses could significantly impact portfolio value".to_string(),
                mitigation_steps: vec![
                    "Implement stricter stop-loss rules".to_string(),
                    "Reduce position sizes".to_string(),
                    "Diversify across more assets".to_string(),
                ],
            });
        }

        Ok(warnings)
    }

    async fn generate_optimization_opportunities(
        &self,
        market_comparison: &MarketComparison,
    ) -> Result<Vec<OptimizationOpportunity>, Box<dyn std::error::Error + Send + Sync>> {
        let mut opportunities = Vec::new();

        // DEX optimization opportunities
        for dex_opt in &market_comparison.dex_performance.optimization_opportunities {
            opportunities.push(OptimizationOpportunity {
                opportunity_type: OpportunityType::CostReduction,
                description: format!("Switch from {} to {} for better rates", dex_opt.current_dex, dex_opt.recommended_dex),
                potential_benefit: dex_opt.potential_savings_percentage,
                implementation_effort: Effort::Low,
                success_probability: dex_opt.confidence_score,
            });
        }

        Ok(opportunities)
    }

    /// Get cached insights for a user
    pub async fn get_cached_insights(&self, user_id: Uuid) -> Option<Vec<TradingInsight>> {
        let cache = self.insights_cache.read().await;
        cache.get(&user_id).cloned()
    }

    /// Get cached recommendations for a user
    pub async fn get_cached_recommendations(&self, user_id: Uuid) -> Option<PerformanceRecommendation> {
        let cache = self.recommendations_cache.read().await;
        cache.get(&user_id).cloned()
    }
}

impl Default for InsightsGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insights_generator_creation() {
        let generator = InsightsGenerator::new();
        assert!(generator.insights_cache.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_performance_insights_generation() {
        // Test performance insight generation logic
    }

    #[tokio::test]
    async fn test_risk_insights_generation() {
        // Test risk insight generation logic
    }
}
