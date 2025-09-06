use crate::analytics::gas_usage_tracker::{GasUsageTracker, GasUsageRecord, GasEfficiencyMetrics, RouteEfficiencyComparison};
use crate::risk_management::types::{UserId, RiskError};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Gas optimization insights and recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasOptimizationInsights {
    pub user_id: UserId,
    pub analysis_period: DateRange,
    pub current_efficiency_score: Decimal, // 0-100 scale
    pub potential_savings_usd: Decimal,
    pub recommendations: Vec<GasOptimizationRecommendation>,
    pub inefficient_routes: Vec<InefficientRoute>,
    pub optimal_timing_windows: Vec<OptimalTimingWindow>,
    pub batch_opportunity_savings: Decimal,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasOptimizationRecommendation {
    pub recommendation_id: String,
    pub recommendation_type: RecommendationType,
    pub title: String,
    pub description: String,
    pub potential_savings_usd: Decimal,
    pub confidence_score: Decimal, // 0-1 scale
    pub implementation_difficulty: DifficultyLevel,
    pub estimated_impact: ImpactLevel,
    pub supporting_data: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    RouteOptimization,
    TimingOptimization,
    BatchTransactions,
    GasPriceStrategy,
    DexSelection,
    SlippageAdjustment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DifficultyLevel {
    Easy,    // User can implement immediately
    Medium,  // Requires some configuration changes
    Hard,    // Requires significant strategy changes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactLevel {
    Low,     // <5% savings
    Medium,  // 5-15% savings
    High,    // >15% savings
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InefficientRoute {
    pub route_identifier: String,
    pub dex_name: String,
    pub token_pair: String,
    pub average_gas_cost_usd: Decimal,
    pub efficiency_ratio: Decimal,
    pub transaction_count: u64,
    pub alternative_routes: Vec<AlternativeRoute>,
    pub savings_potential: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeRoute {
    pub route_identifier: String,
    pub dex_name: String,
    pub estimated_gas_savings: Decimal,
    pub estimated_savings_usd: Decimal,
    pub confidence_score: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimalTimingWindow {
    pub hour_of_day: u8, // 0-23
    pub day_of_week: u8, // 0-6 (Sunday = 0)
    pub average_gas_price: Decimal,
    pub potential_savings_percent: Decimal,
    pub sample_size: u64,
}

/// Route gas analysis for optimization
#[async_trait::async_trait]
pub trait RouteGasAnalyzer: Send + Sync {
    async fn analyze_route_efficiency(&self, routes: &[GasUsageRecord]) -> Result<Vec<RouteEfficiencyAnalysis>, RiskError>;
    async fn identify_inefficient_routes(&self, user_id: UserId, threshold: Decimal) -> Result<Vec<InefficientRoute>, RiskError>;
    async fn suggest_alternative_routes(&self, inefficient_route: &InefficientRoute) -> Result<Vec<AlternativeRoute>, RiskError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEfficiencyAnalysis {
    pub route_identifier: String,
    pub efficiency_score: Decimal, // 0-100
    pub gas_cost_percentile: Decimal, // Where this route ranks vs others
    pub success_rate: Decimal,
    pub average_confirmation_time: u64, // seconds
    pub recommendation: String,
}

/// Gas optimization engine for advanced analysis
#[async_trait::async_trait]
pub trait GasOptimizationEngine: Send + Sync {
    async fn analyze_timing_patterns(&self, user_id: UserId) -> Result<Vec<OptimalTimingWindow>, RiskError>;
    async fn calculate_batch_savings_potential(&self, user_id: UserId) -> Result<Decimal, RiskError>;
    async fn generate_gas_price_strategy(&self, user_id: UserId) -> Result<GasPriceStrategy, RiskError>;
    async fn predict_gas_savings(&self, user_id: UserId, recommendations: &[GasOptimizationRecommendation]) -> Result<Decimal, RiskError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasPriceStrategy {
    pub strategy_type: GasPriceStrategyType,
    pub recommended_gas_price_multiplier: Decimal,
    pub optimal_confirmation_target: u64, // blocks
    pub expected_savings_percent: Decimal,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GasPriceStrategyType {
    Conservative, // Higher gas prices, faster confirmation
    Balanced,     // Standard gas prices
    Aggressive,   // Lower gas prices, slower confirmation
    Dynamic,      // Adjust based on market conditions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,    // Very safe, minimal chance of failure
    Medium, // Some risk of delayed confirmation
    High,   // Higher risk of transaction failure
}

/// Main gas optimization analyzer
pub struct GasOptimizationAnalyzer {
    usage_tracker: Arc<GasUsageTracker>,
    route_analyzer: Arc<MockRouteGasAnalyzer>,
    optimization_engine: Arc<MockGasOptimizationEngine>,
}

impl GasOptimizationAnalyzer {
    pub fn new(
        usage_tracker: Arc<GasUsageTracker>,
        route_analyzer: Arc<MockRouteGasAnalyzer>,
        optimization_engine: Arc<MockGasOptimizationEngine>,
    ) -> Self {
        Self {
            usage_tracker,
            route_analyzer,
            optimization_engine,
        }
    }

    /// Generate comprehensive gas optimization insights for a user
    pub async fn generate_optimization_insights(
        &self,
        user_id: UserId,
        analysis_period_days: u32,
    ) -> Result<GasOptimizationInsights, RiskError> {
        let end_date = Utc::now();
        let start_date = end_date - Duration::days(analysis_period_days as i64);
        
        // Get user's gas usage data
        let gas_records = self.usage_tracker.get_user_gas_usage(user_id, start_date, end_date).await?;
        
        if gas_records.is_empty() {
            return Err(RiskError::InsufficientData("No gas usage data found for analysis".to_string()));
        }

        // Calculate current efficiency metrics
        let efficiency_metrics = self.usage_tracker.calculate_gas_efficiency_metrics(user_id, start_date, end_date).await?;
        
        // Analyze route efficiency
        let route_analysis = self.route_analyzer.analyze_route_efficiency(&gas_records).await?;
        
        // Identify inefficient routes
        let inefficient_routes = self.route_analyzer.identify_inefficient_routes(user_id, Decimal::try_from(0.05f64).unwrap()).await?;
        
        // Analyze timing patterns
        let optimal_timing = self.optimization_engine.analyze_timing_patterns(user_id).await?;
        
        // Calculate batch savings potential
        let batch_savings = self.optimization_engine.calculate_batch_savings_potential(user_id).await?;
        
        // Generate recommendations
        let recommendations = self.generate_recommendations(
            &efficiency_metrics,
            &route_analysis,
            &inefficient_routes,
            &optimal_timing,
            batch_savings,
        ).await?;

        // Calculate efficiency score (0-100)
        let efficiency_score = self.calculate_efficiency_score(&efficiency_metrics, &route_analysis);
        
        // Calculate total potential savings
        let potential_savings = recommendations.iter().map(|r| r.potential_savings_usd).sum();

        Ok(GasOptimizationInsights {
            user_id,
            analysis_period: DateRange { start: start_date, end: end_date },
            current_efficiency_score: efficiency_score,
            potential_savings_usd: potential_savings,
            recommendations,
            inefficient_routes,
            optimal_timing_windows: optimal_timing,
            batch_opportunity_savings: batch_savings,
            generated_at: Utc::now(),
        })
    }

    /// Generate specific recommendations based on analysis
    async fn generate_recommendations(
        &self,
        efficiency_metrics: &GasEfficiencyMetrics,
        route_analysis: &[RouteEfficiencyAnalysis],
        inefficient_routes: &[InefficientRoute],
        optimal_timing: &[OptimalTimingWindow],
        batch_savings: Decimal,
    ) -> Result<Vec<GasOptimizationRecommendation>, RiskError> {
        let mut recommendations = Vec::new();

        // Route optimization recommendations
        for inefficient_route in inefficient_routes {
            if inefficient_route.savings_potential > Decimal::from(10) { // $10+ savings potential
                recommendations.push(GasOptimizationRecommendation {
                    recommendation_id: Uuid::new_v4().to_string(),
                    recommendation_type: RecommendationType::RouteOptimization,
                    title: format!("Optimize {} route on {}", inefficient_route.token_pair, inefficient_route.dex_name),
                    description: format!(
                        "Your {} trades on {} are using {}% more gas than optimal alternatives. Consider switching to more efficient routes.",
                        inefficient_route.token_pair,
                        inefficient_route.dex_name,
                        (inefficient_route.efficiency_ratio * Decimal::from(100)).round()
                    ),
                    potential_savings_usd: inefficient_route.savings_potential,
                    confidence_score: Decimal::from_str("0.75").unwrap(),
                    implementation_difficulty: DifficultyLevel::Easy,
                    estimated_impact: if inefficient_route.savings_potential > Decimal::from(50) {
                        ImpactLevel::High
                    } else if inefficient_route.savings_potential > Decimal::from(20) {
                        ImpactLevel::Medium
                    } else {
                        ImpactLevel::Low
                    },
                    supporting_data: vec![
                        format!("Average gas cost: ${}", inefficient_route.average_gas_cost_usd),
                        format!("Transaction count: {}", inefficient_route.transaction_count),
                        format!("Efficiency ratio: {}", inefficient_route.efficiency_ratio),
                    ],
                });
            }
        }

        // Timing optimization recommendations
        if let Some(best_timing) = optimal_timing.iter().max_by_key(|t| t.potential_savings_percent.to_string()) {
            if best_timing.potential_savings_percent > Decimal::try_from(0.10f64).unwrap() { // 10%+ savings
                recommendations.push(GasOptimizationRecommendation {
                    recommendation_id: Uuid::new_v4().to_string(),
                    recommendation_type: RecommendationType::TimingOptimization,
                    title: "Optimize transaction timing".to_string(),
                    description: format!(
                        "Trading during hour {} on day {} could save you {}% on gas costs based on historical patterns.",
                        best_timing.hour_of_day,
                        best_timing.day_of_week,
                        (best_timing.potential_savings_percent * Decimal::from(100)).round()
                    ),
                    potential_savings_usd: efficiency_metrics.total_gas_spent_usd * best_timing.potential_savings_percent,
                    confidence_score: Decimal::try_from(0.70f64).unwrap(),
                    implementation_difficulty: DifficultyLevel::Medium,
                    estimated_impact: ImpactLevel::Medium,
                    supporting_data: vec![
                        format!("Optimal hour: {}", best_timing.hour_of_day),
                        format!("Average gas price: {} Gwei", best_timing.average_gas_price),
                        format!("Sample size: {} transactions", best_timing.sample_size),
                    ],
                });
            }
        }

        // Batch transaction recommendations
        if batch_savings > Decimal::from(25) { // $25+ potential savings
            recommendations.push(GasOptimizationRecommendation {
                recommendation_id: Uuid::new_v4().to_string(),
                recommendation_type: RecommendationType::BatchTransactions,
                title: "Batch multiple transactions".to_string(),
                description: "You could save on gas costs by batching multiple transactions together instead of executing them individually.".to_string(),
                potential_savings_usd: batch_savings,
                confidence_score: Decimal::from_str("0.75").unwrap(),
                implementation_difficulty: DifficultyLevel::Hard,
                estimated_impact: ImpactLevel::Medium,
                supporting_data: vec![
                    format!("Estimated batch savings: ${}", batch_savings),
                    "Requires transaction batching support".to_string(),
                ],
            });
        }

        // Failed transaction optimization
        if efficiency_metrics.failed_transaction_count > 0 {
            recommendations.push(GasOptimizationRecommendation {
                recommendation_id: Uuid::new_v4().to_string(),
                recommendation_type: RecommendationType::GasPriceStrategy,
                title: "Reduce failed transactions".to_string(),
                description: format!(
                    "You've had {} failed transactions wasting ${} in gas. Consider using higher gas prices or better slippage settings.",
                    efficiency_metrics.failed_transaction_count,
                    efficiency_metrics.gas_wasted_on_failures
                ),
                potential_savings_usd: efficiency_metrics.gas_wasted_on_failures,
                confidence_score: Decimal::try_from(0.90f64).unwrap(),
                implementation_difficulty: DifficultyLevel::Easy,
                estimated_impact: ImpactLevel::High,
                supporting_data: vec![
                    format!("Failed transactions: {}", efficiency_metrics.failed_transaction_count),
                    format!("Gas wasted: ${}", efficiency_metrics.gas_wasted_on_failures),
                ],
            });
        }

        Ok(recommendations)
    }

    /// Calculate overall efficiency score (0-100)
    fn calculate_efficiency_score(
        &self,
        efficiency_metrics: &GasEfficiencyMetrics,
        route_analysis: &[RouteEfficiencyAnalysis],
    ) -> Decimal {
        let mut score = Decimal::from(100);

        // Penalize high efficiency ratio (gas cost / trade value)
        let efficiency_penalty = efficiency_metrics.average_efficiency_ratio * Decimal::from(1000); // Scale for scoring
        score -= efficiency_penalty.min(Decimal::from(30)); // Max 30 point penalty

        // Penalize failed transactions
        if efficiency_metrics.transaction_count > 0 {
            let failure_rate = Decimal::from(efficiency_metrics.failed_transaction_count) / Decimal::from(efficiency_metrics.transaction_count);
            score -= failure_rate * Decimal::from(40); // Max 40 point penalty
        }

        // Factor in route efficiency
        if !route_analysis.is_empty() {
            let avg_route_score: Decimal = route_analysis.iter().map(|r| r.efficiency_score).sum::<Decimal>() / Decimal::from(route_analysis.len());
            score = (score + avg_route_score) / Decimal::from(2); // Average with route scores
        }

        score.max(Decimal::ZERO).min(Decimal::from(100))
    }

    /// Get gas optimization recommendations for immediate implementation
    pub async fn get_immediate_recommendations(
        &self,
        user_id: UserId,
    ) -> Result<Vec<GasOptimizationRecommendation>, RiskError> {
        let insights = self.generate_optimization_insights(user_id, 7).await?; // Last 7 days
        
        // Filter for easy-to-implement, high-impact recommendations
        let immediate_recs: Vec<GasOptimizationRecommendation> = insights.recommendations
            .into_iter()
            .filter(|rec| {
                matches!(rec.implementation_difficulty, DifficultyLevel::Easy) &&
                rec.potential_savings_usd > Decimal::from(5) // At least $5 savings
            })
            .collect();

        Ok(immediate_recs)
    }

    /// Calculate potential savings from implementing all recommendations
    pub async fn calculate_total_savings_potential(
        &self,
        user_id: UserId,
        recommendations: &[GasOptimizationRecommendation],
    ) -> Result<Decimal, RiskError> {
        self.optimization_engine.predict_gas_savings(user_id, recommendations).await
    }
}

// Mock implementations for testing
pub struct MockRouteGasAnalyzer;

#[async_trait::async_trait]
impl RouteGasAnalyzer for MockRouteGasAnalyzer {
    async fn analyze_route_efficiency(&self, routes: &[GasUsageRecord]) -> Result<Vec<RouteEfficiencyAnalysis>, RiskError> {
        let mut analysis = Vec::new();
        let mut route_groups: HashMap<String, Vec<&GasUsageRecord>> = HashMap::new();
        
        // Group routes by identifier
        for record in routes {
            let route_id = format!("{}_{}", record.dex_name, record.route_type);
            route_groups.entry(route_id).or_default().push(record);
        }

        for (route_id, records) in route_groups {
            let avg_efficiency: Decimal = records.iter().map(|r| r.gas_efficiency).sum::<Decimal>() / Decimal::from(records.len());
            let success_count = records.iter().filter(|r| matches!(r.transaction_status, crate::analytics::gas_usage_tracker::TransactionStatus::Confirmed)).count();
            let success_rate = Decimal::from(success_count) / Decimal::from(records.len());
            
            // Convert efficiency ratio to score (lower is better, so invert)
            let efficiency_score = (Decimal::ONE / (avg_efficiency + Decimal::try_from(0.001f64).unwrap())) * Decimal::from(10);
            let efficiency_score = efficiency_score.min(Decimal::from(100));

            analysis.push(RouteEfficiencyAnalysis {
                route_identifier: route_id,
                efficiency_score,
                gas_cost_percentile: Decimal::from(50), // Mock percentile
                success_rate,
                average_confirmation_time: 30, // Mock 30 seconds
                recommendation: if efficiency_score > Decimal::from(80) {
                    "Excellent efficiency".to_string()
                } else if efficiency_score > Decimal::from(60) {
                    "Good efficiency".to_string()
                } else {
                    "Consider alternative routes".to_string()
                },
            });
        }

        Ok(analysis)
    }

    async fn identify_inefficient_routes(&self, _user_id: UserId, threshold: Decimal) -> Result<Vec<InefficientRoute>, RiskError> {
        // Mock implementation - return sample inefficient route
        Ok(vec![
            InefficientRoute {
                route_identifier: "uniswap_v3_direct".to_string(),
                dex_name: "Uniswap V3".to_string(),
                token_pair: "ETH/USDC".to_string(),
                average_gas_cost_usd: Decimal::from(25),
                efficiency_ratio: threshold + Decimal::try_from(0.02f64).unwrap(),
                transaction_count: 15,
                alternative_routes: vec![
                    AlternativeRoute {
                        route_identifier: "curve_stable".to_string(),
                        dex_name: "Curve".to_string(),
                        estimated_gas_savings: Decimal::from(5000), // 5000 gas units
                        estimated_savings_usd: Decimal::from(8),
                        confidence_score: Decimal::try_from(0.85f64).unwrap(),
                    }
                ],
                savings_potential: Decimal::from(120), // $120 total savings
            }
        ])
    }

    async fn suggest_alternative_routes(&self, _inefficient_route: &InefficientRoute) -> Result<Vec<AlternativeRoute>, RiskError> {
        Ok(vec![
            AlternativeRoute {
                route_identifier: "alternative_route".to_string(),
                dex_name: "Alternative DEX".to_string(),
                estimated_gas_savings: Decimal::from(3000),
                estimated_savings_usd: Decimal::from(5),
                confidence_score: Decimal::try_from(0.75f64).unwrap(),
            }
        ])
    }
}

pub struct MockGasOptimizationEngine;

#[async_trait::async_trait]
impl GasOptimizationEngine for MockGasOptimizationEngine {
    async fn analyze_timing_patterns(&self, _user_id: UserId) -> Result<Vec<OptimalTimingWindow>, RiskError> {
        Ok(vec![
            OptimalTimingWindow {
                hour_of_day: 3, // 3 AM UTC
                day_of_week: 2, // Tuesday
                average_gas_price: Decimal::from(18), // 18 Gwei
                potential_savings_percent: Decimal::try_from(0.15f64).unwrap(), // 15%
                sample_size: 50,
            },
            OptimalTimingWindow {
                hour_of_day: 14, // 2 PM UTC
                day_of_week: 6, // Saturday
                average_gas_price: Decimal::from(22), // 22 Gwei
                potential_savings_percent: Decimal::try_from(0.08f64).unwrap(), // 8%
                sample_size: 30,
            }
        ])
    }

    async fn calculate_batch_savings_potential(&self, _user_id: UserId) -> Result<Decimal, RiskError> {
        Ok(Decimal::from(35)) // $35 potential savings from batching
    }

    async fn generate_gas_price_strategy(&self, _user_id: UserId) -> Result<GasPriceStrategy, RiskError> {
        Ok(GasPriceStrategy {
            strategy_type: GasPriceStrategyType::Balanced,
            recommended_gas_price_multiplier: Decimal::try_from(1.1f64).unwrap(), // 10% above standard
            optimal_confirmation_target: 3, // 3 blocks
            expected_savings_percent: Decimal::try_from(0.12f64).unwrap(), // 12%
            risk_level: RiskLevel::Low,
        })
    }

    async fn predict_gas_savings(&self, _user_id: UserId, recommendations: &[GasOptimizationRecommendation]) -> Result<Decimal, RiskError> {
        // Sum up potential savings with confidence weighting
        let total_savings: Decimal = recommendations
            .iter()
            .map(|rec| rec.potential_savings_usd * rec.confidence_score)
            .sum();
        
        Ok(total_savings)
    }
}
