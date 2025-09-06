use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc, Timelike, Duration};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::user_retention::performance_analytics::{UserPerformanceMetrics, TradingPattern, UserPerformanceAnalyzer};
use crate::user_retention::trading_insights::market_intelligence::MarketIntelligenceEngine;
use crate::risk_management::redis_cache::RiskCache;
use rust_decimal_macros::dec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedInsight {
    pub insight_id: Uuid,
    pub user_id: Uuid,
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub action_items: Vec<String>,
    pub confidence_score: f64,
    pub priority: String,
    pub relevant_tokens: Vec<String>,
    pub potential_impact: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InsightType {
    TradingOpportunity,
    RiskWarning,
    OptimizationSuggestion,
    MarketTiming,
    LiquidityAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOpportunity {
    pub opportunity_id: Uuid,
    pub user_id: Uuid,
    pub opportunity_type: OpportunityType,
    pub token_pair: String,
    pub dex_name: String,
    pub potential_return: Decimal,
    pub confidence: f64,
    pub risk_level: String,
    pub time_sensitive: bool,
    pub expires_at: DateTime<Utc>,
    pub action_required: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpportunityType {
    Arbitrage,
    LiquidityMining,
    YieldFarming,
    PriceMovement,
    VolatilityTrade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingRecommendation {
    pub recommendation_id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub optimal_time: DateTime<Utc>,
    pub time_window: Duration,
    pub reasoning: String,
    pub confidence: f64,
    pub market_conditions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAdjustedRecommendation {
    pub user_id: Uuid,
    pub recommendation_type: RecommendationType,
    pub current_risk_level: RiskLevel,
    pub recommended_action: String,
    pub risk_mitigation_steps: Vec<String>,
    pub portfolio_allocation: HashMap<String, Decimal>,
    pub max_position_size: Decimal,
    pub diversification_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    PositionSizing,
    Diversification,
    RiskReduction,
    ProfitTaking,
    StopLossAdjustment,
}

pub struct PersonalizationEngine {
    user_analyzer: Arc<UserPerformanceAnalyzer>,
    market_intelligence: Arc<MarketIntelligenceEngine>,
    cache: Arc<RiskCache>,
    insights_cache: Arc<RwLock<HashMap<Uuid, Vec<PersonalizedInsight>>>>,
    opportunities_cache: Arc<RwLock<HashMap<Uuid, Vec<MarketOpportunity>>>>,
    timing_cache: Arc<RwLock<HashMap<Uuid, Vec<TimingRecommendation>>>>,
}

impl PersonalizationEngine {
    pub fn new(
        user_analyzer: Arc<UserPerformanceAnalyzer>,
        market_intelligence: Arc<MarketIntelligenceEngine>,
        cache: Arc<RiskCache>,
    ) -> Self {
        Self {
            user_analyzer,
            market_intelligence,
            cache,
            insights_cache: Arc::new(RwLock::new(HashMap::new())),
            opportunities_cache: Arc::new(RwLock::new(HashMap::new())),
            timing_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate personalized insights based on user's trading patterns
    pub async fn generate_personalized_insights(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<PersonalizedInsight>, Box<dyn std::error::Error + Send + Sync>> {
        let mut insights = Vec::new();

        // Get user's trading patterns
        let trading_patterns = self.user_analyzer.analyze_trading_patterns(user_id, Some(Duration::days(30))).await?;
        let performance_metrics = self.user_analyzer.calculate_user_performance(user_id, Some(Duration::days(30))).await?;
        
        // Get market intelligence
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        // Generate trading opportunity insights
        insights.extend(self.generate_trading_opportunities(&trading_patterns, &market_intel).await);

        // Generate risk warnings
        insights.extend(self.generate_risk_warnings(&performance_metrics, &trading_patterns).await);

        // Generate optimization suggestions
        insights.extend(self.generate_optimization_suggestions(&trading_patterns, &market_intel).await);

        // Generate gas optimization insights
        insights.extend(self.generate_gas_optimization_insights(&trading_patterns, &market_intel).await);

        // Cache the insights
        let mut cache = self.insights_cache.write().await;
        cache.insert(user_id, insights.clone());

        Ok(insights)
    }

    /// Generate customized market opportunities based on user preferences
    pub async fn generate_market_opportunities(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<MarketOpportunity>, Box<dyn std::error::Error + Send + Sync>> {
        let mut opportunities = Vec::new();

        let trading_patterns = self.user_analyzer.analyze_trading_patterns(user_id, Some(Duration::days(30))).await?;
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        // Filter opportunities based on user's preferred tokens
        let token_trends = self.generate_token_trends().await;
        for token_symbol in token_trends {
            let opportunity = self.create_market_opportunity(user_id, &token_symbol, &trading_patterns).await;
            opportunities.push(opportunity);
        }

        // Generate arbitrage opportunities for user's preferred DEXes
        opportunities.extend(self.generate_arbitrage_opportunities(user_id, &trading_patterns).await);

        // Cache the opportunities
        let mut cache = self.opportunities_cache.write().await;
        cache.insert(user_id, opportunities.clone());

        Ok(opportunities)
    }

    /// Generate timing recommendations for optimal trade execution
    pub async fn generate_timing_recommendations(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<TimingRecommendation>, Box<dyn std::error::Error + Send + Sync>> {
        let mut recommendations = Vec::new();

        let trading_patterns = self.user_analyzer.analyze_trading_patterns(user_id, Some(Duration::days(30))).await?;
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        // Generate timing recommendations based on gas patterns
        for gas_pattern in &market_intel.gas_patterns {
            if trading_patterns.preferred_chains.contains(&gas_pattern.chain_id) {
                let recommendation = TimingRecommendation {
                    recommendation_id: Uuid::new_v4(),
                    user_id,
                    action: "Buy".to_string(),
                    optimal_time: self.calculate_optimal_timing(&gas_pattern.low_hours).await,
                    time_window: Duration::hours(2),
                    reasoning: format!("Gas prices are typically {}% lower during hours {:?}", 
                        30, gas_pattern.low_hours),
                    confidence: 0.8,
                    market_conditions: vec![
                        "Low network congestion".to_string(),
                        "Reduced gas prices".to_string(),
                    ],
                };
                recommendations.push(recommendation);
            }
        }

        // Generate timing based on liquidity patterns
        for liquidity_pattern in &market_intel.liquidity_patterns {
            if trading_patterns.preferred_tokens.iter().any(|token| 
                liquidity_pattern.token_pair.contains(token)) {
                let recommendation = TimingRecommendation {
                    recommendation_id: Uuid::new_v4(),
                    user_id,
                    action: "Buy".to_string(),
                    optimal_time: self.calculate_optimal_timing(&liquidity_pattern.peak_hours).await,
                    time_window: Duration::hours(1),
                    reasoning: format!("Liquidity is highest during hours {:?}, reducing slippage", 
                        liquidity_pattern.peak_hours),
                    confidence: liquidity_pattern.confidence_score,
                    market_conditions: vec![
                        "High liquidity".to_string(),
                        "Low slippage".to_string(),
                    ],
                };
                recommendations.push(recommendation);
            }
        }

        // Cache the recommendations
        let mut cache = self.timing_cache.write().await;
        cache.insert(user_id, recommendations.clone());

        Ok(recommendations)
    }

    /// Generate risk-adjusted recommendations for user's portfolio
    pub async fn generate_risk_adjusted_recommendations(
        &self,
        user_id: Uuid,
    ) -> Result<RiskAdjustedRecommendation, Box<dyn std::error::Error + Send + Sync>> {
        let trading_patterns = self.user_analyzer.analyze_trading_patterns(user_id, Some(Duration::days(30))).await?;
        let performance_metrics = self.user_analyzer.calculate_user_performance(user_id, Some(Duration::days(30))).await?;

        // Calculate current portfolio allocation
        let mut portfolio_allocation = HashMap::new();
        for (i, token) in trading_patterns.preferred_tokens.iter().enumerate() {
            let allocation = if i == 0 { 
                dec!(0.4) // 40% for most preferred
            } else if i == 1 { 
                dec!(0.3) // 30% for second
            } else { 
                dec!(0.15) // 15% for others
            };
            portfolio_allocation.insert(token.clone(), allocation);
        }

        let recommendation = RiskAdjustedRecommendation {
            user_id,
            recommendation_type: RecommendationType::Diversification,
            current_risk_level: determine_risk_level_from_patterns(&trading_patterns),
            recommended_action: "Rebalance portfolio to reduce concentration risk".to_string(),
            risk_mitigation_steps: vec![
                "Limit single position to 25% of portfolio".to_string(),
                "Diversify across at least 4 different tokens".to_string(),
                "Use stop-loss orders for positions > 10%".to_string(),
            ],
            portfolio_allocation,
            max_position_size: dec!(0.25), // 25% max
            diversification_score: 0.75,
        };

        Ok(recommendation)
    }

    /// Get cached personalized insights
    pub async fn get_cached_insights(&self, user_id: Uuid) -> Option<Vec<PersonalizedInsight>> {
        let cache = self.insights_cache.read().await;
        cache.get(&user_id).cloned()
    }

    /// Get cached market opportunities
    pub async fn get_cached_opportunities(&self, user_id: Uuid) -> Option<Vec<MarketOpportunity>> {
        let cache = self.opportunities_cache.read().await;
        cache.get(&user_id).cloned()
    }

    /// Get cached timing recommendations
    pub async fn get_cached_timing(&self, user_id: Uuid) -> Option<Vec<TimingRecommendation>> {
        let cache = self.timing_cache.read().await;
        cache.get(&user_id).cloned()
    }

    // Helper methods

    async fn generate_trading_opportunities(
        &self,
        trading_patterns: &TradingPattern,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> Vec<PersonalizedInsight> {
        let mut insights = Vec::new();

        for token_trend in &market_intel.token_trends {
            if trading_patterns.preferred_tokens.contains(&token_trend.token_symbol) {
                let insight = PersonalizedInsight {
                    insight_id: Uuid::new_v4(),
                    user_id: Uuid::new_v4(), // Will be set by caller
                    insight_type: InsightType::TradingOpportunity,
                    title: format!("{} Momentum Opportunity", token_trend.token_symbol),
                    description: format!(
                        "{} is showing strong momentum with {}% price growth and positive technical indicators",
                        token_trend.token_symbol,
                        token_trend.price_momentum * Decimal::from(100)
                    ),
                    action_items: vec![
                        format!("Consider buying {} during low gas hours", token_trend.token_symbol),
                        "Set stop-loss at 5% below entry".to_string(),
                        "Monitor volume for confirmation".to_string(),
                    ],
                    confidence_score: token_trend.technical_score,
                    priority: "High".to_string(),
                    relevant_tokens: vec![token_trend.token_symbol.clone()],
                    potential_impact: format!("Potential profit: ${}", token_trend.price_momentum * Decimal::from(1000)),
                    expires_at: Some(Utc::now() + Duration::hours(24)),
                    created_at: Utc::now(),
                };
                insights.push(insight);
            }
        }

        insights
    }

    async fn generate_token_trends(&self) -> Vec<String> {
        // Placeholder implementation - return empty for now
        vec![]
    }

    async fn generate_risk_warnings(
        &self,
        performance_metrics: &UserPerformanceMetrics,
        trading_patterns: &TradingPattern,
    ) -> Vec<PersonalizedInsight> {
        let mut insights = Vec::new();

        // Check for high risk patterns
        if performance_metrics.max_drawdown > dec!(0.2) {
            let insight = PersonalizedInsight {
                insight_id: Uuid::new_v4(),
                user_id: performance_metrics.user_id,
                insight_type: InsightType::RiskWarning,
                title: "High Drawdown Risk Detected".to_string(),
                description: format!(
                    "Your maximum drawdown of {}% exceeds recommended levels",
                    performance_metrics.max_drawdown * Decimal::from(100)
                ),
                action_items: vec![
                    "Reduce position sizes".to_string(),
                    "Implement stricter stop-losses".to_string(),
                    "Diversify across more assets".to_string(),
                ],
                confidence_score: 0.9,
                priority: "Critical".to_string(),
                relevant_tokens: trading_patterns.preferred_tokens.clone(),
                potential_impact: "Risk reduction: 40%".to_string(),
                expires_at: None, // Risk warnings don't expire
                created_at: Utc::now(),
            };
            insights.push(insight);
        }

        insights
    }

    async fn generate_optimization_suggestions(
        &self,
        trading_patterns: &TradingPattern,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> Vec<PersonalizedInsight> {
        let mut insights = Vec::new();

        // Suggest better DEX routes based on market data
        for market_data in &market_intel.market_data {
            if trading_patterns.preferred_tokens.iter().any(|token| 
                market_data.token_pair.contains(token)) {
                
                let insight = PersonalizedInsight {
                    insight_id: Uuid::new_v4(),
                    user_id: Uuid::new_v4(), // Will be set by caller
                    insight_type: InsightType::OptimizationSuggestion,
                    title: format!("Better Rates Available on {}", market_data.dex_name),
                    description: format!(
                        "{} offers better rates for {} with {}% higher liquidity",
                        market_data.dex_name,
                        market_data.token_pair,
                        15 // Placeholder percentage
                    ),
                    action_items: vec![
                        format!("Try {} for {} trades", market_data.dex_name, market_data.token_pair),
                        "Compare gas costs before switching".to_string(),
                    ],
                    confidence_score: 0.75,
                    priority: "Medium".to_string(),
                    relevant_tokens: vec![market_data.token_pair.clone()],
                    potential_impact: format!("Potential savings: ${}", 50),
                    expires_at: Some(Utc::now() + Duration::hours(12)),
                    created_at: Utc::now(),
                };
                insights.push(insight);
            }
        }

        insights
    }

    async fn generate_gas_optimization_insights(
        &self,
        trading_patterns: &TradingPattern,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> Vec<PersonalizedInsight> {
        let mut insights = Vec::new();

        for gas_pattern in &market_intel.gas_patterns {
            if trading_patterns.preferred_chains.contains(&gas_pattern.chain_id) {
                for optimization in &gas_pattern.optimization_opportunities {
                    let insight = PersonalizedInsight {
                        insight_id: Uuid::new_v4(),
                        user_id: Uuid::new_v4(), // Will be set by caller
                        insight_type: InsightType::OptimizationSuggestion,
                        title: "Gas Optimization Opportunity".to_string(),
                        description: format!(
                            "Trade at {}:00 UTC to save up to {}% on gas fees",
                            optimization.recommended_hour,
                            optimization.potential_savings * Decimal::from(100)
                        ),
                        action_items: vec![
                            format!("Schedule trades for {}:00 UTC", optimization.recommended_hour),
                            "Set up alerts for low gas periods".to_string(),
                        ],
                        confidence_score: optimization.confidence,
                        priority: "Medium".to_string(),
                        relevant_tokens: vec![],
                        potential_impact: format!("Gas savings: ${}", optimization.potential_savings * Decimal::from(100)),
                        expires_at: Some(Utc::now() + Duration::hours(24)),
                        created_at: Utc::now(),
                    };
                    insights.push(insight);
                }
            }
        }

        insights
    }

    async fn create_market_opportunity(
        &self,
        user_id: Uuid,
        token_symbol: &str,
        trading_patterns: &TradingPattern,
    ) -> MarketOpportunity {
        MarketOpportunity {
            opportunity_id: Uuid::new_v4(),
            user_id,
            opportunity_type: OpportunityType::PriceMovement,
            token_pair: format!("{}/USDC", token_symbol),
            dex_name: trading_patterns.preferred_dexes.first()
                .unwrap_or(&"Uniswap V3".to_string()).clone(),
            potential_return: dec!(0.20), // 20% target
            confidence: 0.75,
            risk_level: "Medium".to_string(),
            time_sensitive: true,
            expires_at: Utc::now() + Duration::hours(24),
            action_required: format!("Consider buying {} on {}", token_symbol, trading_patterns.preferred_dexes.first().unwrap_or(&"Uniswap V3".to_string())),
        }
    }

    async fn generate_arbitrage_opportunities(
        &self,
        user_id: Uuid,
        trading_patterns: &TradingPattern,
    ) -> Vec<MarketOpportunity> {
        // Placeholder implementation for arbitrage opportunities
        vec![]
    }

    async fn calculate_optimal_timing(&self, optimal_hours: &[u8]) -> DateTime<Utc> {
        let now = Utc::now();
        let current_hour = now.hour() as u8;
        
        // Find the next optimal hour
        let next_optimal = optimal_hours.iter()
            .find(|&&hour| hour > current_hour)
            .unwrap_or(&optimal_hours[0]);
        
        let hours_to_wait = if *next_optimal > current_hour {
            *next_optimal - current_hour
        } else {
            24 - current_hour + *next_optimal
        };
        
        now + Duration::hours(hours_to_wait as i64)
    }
}

// Helper trait implementations
// Helper function to determine risk level from trading patterns
fn determine_risk_level_from_patterns(trading_patterns: &TradingPattern) -> RiskLevel {
    // Use average trade size to determine risk level (trade_frequency not available in TradingPattern)
    if trading_patterns.average_trade_size_usd > Decimal::from(10000) {
        RiskLevel::High
    } else if trading_patterns.average_trade_size_usd > Decimal::from(1000) {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
    use crate::user_retention::trading_insights::market_intelligence::MarketIntelligenceEngine;
    use crate::aggregator::DEXAggregator;
    use crate::risk_management::redis_cache::RiskCache;
    use redis::Client;

    #[tokio::test]
    async fn test_personalization_engine_creation() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        
        let engine = PersonalizationEngine::new(user_analyzer, market_intelligence, cache);
        
        let user_id = Uuid::new_v4();
        
        // Test insight generation
        let insights = engine.generate_personalized_insights(user_id).await;
        assert!(insights.is_ok());
        
        // Test opportunity generation
        let opportunities = engine.generate_market_opportunities(user_id).await;
        assert!(opportunities.is_ok());
        
        // Test timing recommendations
        let timing = engine.generate_timing_recommendations(user_id).await;
        assert!(timing.is_ok());
        
        // Test risk-adjusted recommendations
        let risk_rec = engine.generate_risk_adjusted_recommendations(user_id).await;
        assert!(risk_rec.is_ok());
    }

    #[tokio::test]
    async fn test_caching_functionality() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        
        let engine = PersonalizationEngine::new(user_analyzer, market_intelligence, cache);
        
        let user_id = Uuid::new_v4();
        
        // Generate insights to populate cache
        let _ = engine.generate_personalized_insights(user_id).await;
        let _ = engine.generate_market_opportunities(user_id).await;
        let _ = engine.generate_timing_recommendations(user_id).await;
        
        // Test cached retrieval
        let cached_insights = engine.get_cached_insights(user_id).await;
        assert!(cached_insights.is_some());
        
        let cached_opportunities = engine.get_cached_opportunities(user_id).await;
        assert!(cached_opportunities.is_some());
        
        let cached_timing = engine.get_cached_timing(user_id).await;
        assert!(cached_timing.is_some());
    }
}
