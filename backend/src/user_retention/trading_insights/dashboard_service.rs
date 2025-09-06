use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

use crate::user_retention::trading_insights::personalization_engine::{PersonalizationEngine, PersonalizedInsight, MarketOpportunity, TimingRecommendation, RiskAdjustedRecommendation};
use crate::user_retention::trading_insights::market_intelligence::MarketIntelligenceEngine;
use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
use crate::risk_management::redis_cache::RiskCache;
use rust_decimal_macros::dec;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub user_id: Uuid,
    pub market_overview: MarketOverview,
    pub personalized_feed: PersonalizedFeed,
    pub performance_summary: PerformanceSummary,
    pub active_opportunities: Vec<MarketOpportunity>,
    pub timing_recommendations: Vec<TimingRecommendation>,
    pub risk_alerts: Vec<PersonalizedInsight>,
    pub gas_optimization: GasOptimizationWidget,
    pub liquidity_insights: LiquidityWidget,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOverview {
    pub market_sentiment: f64,
    pub volatility_index: f64,
    pub top_gainers: Vec<TokenPerformance>,
    pub top_losers: Vec<TokenPerformance>,
    pub trending_pairs: Vec<TrendingPair>,
    pub total_market_volume: Decimal,
    pub active_dexes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPerformance {
    pub symbol: String,
    pub price_change_24h: Decimal,
    pub volume_24h: Decimal,
    pub current_price: Decimal,
    pub market_cap: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendingPair {
    pub pair: String,
    pub volume_growth: Decimal,
    pub price_momentum: Decimal,
    pub liquidity_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizedFeed {
    pub insights: Vec<PersonalizedInsight>,
    pub opportunities: Vec<MarketOpportunity>,
    pub recommendations: Vec<String>,
    pub priority_alerts: Vec<PersonalizedInsight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_return: Decimal,
    pub win_rate: Decimal,
    pub sharpe_ratio: Decimal,
    pub portfolio_value: Decimal,
    pub daily_pnl: Decimal,
    pub best_performing_token: String,
    pub worst_performing_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasOptimizationWidget {
    pub current_gas_prices: HashMap<u64, Decimal>, // chain_id -> gas_price
    pub optimal_trading_hours: Vec<u8>,
    pub potential_savings: Decimal,
    pub next_optimal_time: DateTime<Utc>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityWidget {
    pub high_liquidity_pairs: Vec<LiquidityInfo>,
    pub low_liquidity_warnings: Vec<LiquidityWarning>,
    pub optimal_trade_sizes: HashMap<String, Decimal>, // pair -> optimal_size
    pub slippage_estimates: HashMap<String, Decimal>, // pair -> estimated_slippage
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityInfo {
    pub pair: String,
    pub dex: String,
    pub liquidity: Decimal,
    pub depth: Decimal,
    pub spread: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityWarning {
    pub pair: String,
    pub current_liquidity: Decimal,
    pub recommended_min: Decimal,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfiguration {
    pub user_id: Uuid,
    pub layout_preferences: LayoutPreferences,
    pub widget_settings: WidgetSettings,
    pub notification_preferences: NotificationPreferences,
    pub refresh_intervals: RefreshIntervals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPreferences {
    pub widget_order: Vec<String>,
    pub grid_layout: HashMap<String, GridPosition>,
    pub theme: String,
    pub compact_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSettings {
    pub show_market_overview: bool,
    pub show_personalized_feed: bool,
    pub show_performance_summary: bool,
    pub show_gas_optimization: bool,
    pub show_liquidity_insights: bool,
    pub max_insights_displayed: u32,
    pub max_opportunities_displayed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    pub critical_alerts: bool,
    pub opportunity_alerts: bool,
    pub gas_optimization_alerts: bool,
    pub performance_updates: bool,
    pub market_updates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshIntervals {
    pub market_data: u32, // seconds
    pub personalized_insights: u32,
    pub performance_data: u32,
    pub gas_prices: u32,
    pub liquidity_data: u32,
}

pub struct DashboardService {
    market_intelligence: Arc<MarketIntelligenceEngine>,
    personalization_engine: Arc<PersonalizationEngine>,
    user_analyzer: Arc<UserPerformanceAnalyzer>,
    cache: Arc<RiskCache>,
    dashboard_cache: Arc<RwLock<HashMap<Uuid, DashboardData>>>,
    config_cache: Arc<RwLock<HashMap<Uuid, DashboardConfiguration>>>,
}

impl DashboardService {
    pub fn new(
        market_intelligence: Arc<MarketIntelligenceEngine>,
        personalization_engine: Arc<PersonalizationEngine>,
        user_analyzer: Arc<UserPerformanceAnalyzer>,
        cache: Arc<RiskCache>,
    ) -> Self {
        Self {
            market_intelligence,
            personalization_engine,
            user_analyzer,
            cache,
            dashboard_cache: Arc::new(RwLock::new(HashMap::new())),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate complete dashboard data for a user
    pub async fn generate_dashboard_data(
        &self,
        user_id: Uuid,
    ) -> Result<DashboardData, Box<dyn std::error::Error + Send + Sync>> {
        // Get market intelligence
        let market_intel = self.generate_market_intelligence().await?;
        
        // Get personalized insights and opportunities
        let insights = self.personalization_engine.generate_personalized_insights(user_id).await?;
        let opportunities = self.personalization_engine.generate_market_opportunities(user_id).await?;
        let timing_recs = self.personalization_engine.generate_timing_recommendations(user_id).await?;
        
        // Get user performance data
        let performance_metrics = self.user_analyzer.calculate_user_performance(user_id, Some(Duration::days(30))).await?;
        
        // Build dashboard components
        let market_overview = self.build_market_overview(&market_intel).await;
        let personalized_feed = self.build_personalized_feed(insights, opportunities.clone()).await;
        let performance_summary = self.build_performance_summary(&performance_metrics).await;
        let gas_optimization = self.build_gas_optimization_widget(&market_intel).await;
        let liquidity_insights = self.build_liquidity_widget(&market_intel).await;
        
        // Filter risk alerts
        let risk_alerts = self.filter_risk_alerts(&personalized_feed.insights).await;

        let dashboard_data = DashboardData {
            user_id,
            market_overview,
            personalized_feed,
            performance_summary,
            active_opportunities: opportunities,
            timing_recommendations: timing_recs,
            risk_alerts,
            gas_optimization,
            liquidity_insights,
            generated_at: Utc::now(),
        };

        // Cache the dashboard data
        let mut cache = self.dashboard_cache.write().await;
        cache.insert(user_id, dashboard_data.clone());

        Ok(dashboard_data)
    }

    /// Get cached dashboard data
    pub async fn get_cached_dashboard(&self, user_id: Uuid) -> Option<DashboardData> {
        let cache = self.dashboard_cache.read().await;
        cache.get(&user_id).cloned()
    }

    /// Update dashboard configuration
    pub async fn update_dashboard_config(
        &self,
        user_id: Uuid,
        config: DashboardConfiguration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = self.config_cache.write().await;
        cache.insert(user_id, config);
        Ok(())
    }

    /// Get dashboard configuration
    pub async fn get_dashboard_config(&self, user_id: Uuid) -> DashboardConfiguration {
        let cache = self.config_cache.read().await;
        cache.get(&user_id).cloned().unwrap_or_else(|| self.default_config(user_id))
    }

    /// Get real-time market overview
    pub async fn get_market_overview(&self) -> Result<MarketOverview, Box<dyn std::error::Error + Send + Sync>> {
        let market_intel = self.generate_market_intelligence().await?;
        Ok(self.build_market_overview(&market_intel).await)
    }

    /// Generate market intelligence data
    async fn generate_market_intelligence(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;
        Ok("Market intelligence placeholder".to_string())
    }

    /// Get personalized opportunity feed
    pub async fn get_opportunity_feed(
        &self,
        user_id: Uuid,
        limit: Option<u32>,
    ) -> Result<Vec<MarketOpportunity>, Box<dyn std::error::Error + Send + Sync>> {
        let mut opportunities = self.personalization_engine.generate_market_opportunities(user_id).await?;
        
        // Sort by confidence and potential return
        opportunities.sort_by(|a, b| {
            let score_a = a.confidence * a.potential_return.try_into().unwrap_or(0.0f64);
            let score_b = b.confidence * b.potential_return.try_into().unwrap_or(0.0f64);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(limit) = limit {
            opportunities.truncate(limit as usize);
        }

        Ok(opportunities)
    }

    /// Get interactive chart data
    pub async fn get_chart_data(
        &self,
        chart_type: ChartType,
        user_id: Option<Uuid>,
        time_range: Duration,
    ) -> Result<ChartData, Box<dyn std::error::Error + Send + Sync>> {
        match chart_type {
            ChartType::PortfolioPerformance => {
                if let Some(user_id) = user_id {
                    self.get_portfolio_chart_data(user_id, time_range).await
                } else {
                    Err("User ID required for portfolio performance chart".into())
                }
            }
            ChartType::MarketOverview => self.get_market_chart_data(time_range).await,
            ChartType::GasPrices => self.get_gas_chart_data(time_range).await,
            ChartType::LiquidityTrends => self.get_liquidity_chart_data(time_range).await,
        }
    }

    /// Refresh specific dashboard components
    pub async fn refresh_component(
        &self,
        user_id: Uuid,
        component: DashboardComponent,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match component {
            DashboardComponent::MarketOverview => {
                let overview = self.get_market_overview().await?;
                Ok(serde_json::to_value(overview)?)
            }
            DashboardComponent::PersonalizedFeed => {
                let insights = self.personalization_engine.generate_personalized_insights(user_id).await?;
                let opportunities = self.personalization_engine.generate_market_opportunities(user_id).await?;
                let feed = self.build_personalized_feed(insights, opportunities).await;
                Ok(serde_json::to_value(feed)?)
            }
            DashboardComponent::PerformanceSummary => {
                let metrics = self.user_analyzer.calculate_user_performance(user_id, Some(Duration::days(30))).await?;
                let summary = self.build_performance_summary(&metrics).await;
                Ok(serde_json::to_value(summary)?)
            }
            DashboardComponent::GasOptimization => {
                let market_intel = self.generate_market_intelligence().await?;
                let widget = self.build_gas_optimization_widget(&market_intel).await;
                Ok(serde_json::to_value(widget)?)
            }
            DashboardComponent::LiquidityInsights => {
                let market_intel = self.generate_market_intelligence().await?;
                let widget = self.build_liquidity_widget(&market_intel).await;
                Ok(serde_json::to_value(widget)?)
            }
        }
    }

    // Helper methods

    async fn build_market_overview(&self, market_intel: &str) -> MarketOverview {
        let mut top_gainers = Vec::new();
        let mut top_losers = Vec::new();
        let mut trending_pairs = Vec::new();

        // Placeholder market data processing
        let performance = TokenPerformance {
            symbol: "ETH".to_string(),
            price_change_24h: dec!(0.05),
            volume_24h: dec!(1000000),
            current_price: dec!(2000),
            market_cap: None,
        };
        top_gainers.push(performance);

        // Create trending pairs placeholder
        let trending = TrendingPair {
            pair: "ETH/USDC".to_string(),
            volume_growth: dec!(0.15),
            price_momentum: dec!(0.05),
            liquidity_score: 0.8,
        };
        trending_pairs.push(trending);

        // Sort and limit
        top_gainers.sort_by(|a, b| b.price_change_24h.cmp(&a.price_change_24h));
        top_gainers.truncate(5);
        
        top_losers.sort_by(|a: &TokenPerformance, b: &TokenPerformance| a.price_change_24h.cmp(&b.price_change_24h));
        top_losers.truncate(5);
        
        trending_pairs.sort_by(|a, b| b.volume_growth.cmp(&a.volume_growth));
        trending_pairs.truncate(10);

        MarketOverview {
            market_sentiment: 0.75, // Placeholder (0.0-1.0 scale)
            volatility_index: 0.25, // Placeholder
            top_gainers,
            top_losers,
            trending_pairs,
            total_market_volume: dec!(1000000000), // Placeholder
            active_dexes: 14, // Placeholder
        }
    }

    async fn build_personalized_feed(
        &self,
        insights: Vec<PersonalizedInsight>,
        opportunities: Vec<MarketOpportunity>,
    ) -> PersonalizedFeed {
        // Filter priority alerts
        let priority_alerts: Vec<PersonalizedInsight> = insights.iter()
            .filter(|i| i.priority == "Critical" || i.priority == "High")
            .cloned()
            .collect();

        // Generate recommendations
        let recommendations = vec![
            "Consider diversifying across more DEXes for better rates".to_string(),
            "Monitor gas prices for optimal trading times".to_string(),
            "Review stop-loss levels for high-risk positions".to_string(),
        ];

        PersonalizedFeed {
            insights,
            opportunities,
            recommendations,
            priority_alerts,
        }
    }

    async fn build_performance_summary(
        &self,
        metrics: &crate::user_retention::performance_analytics::UserPerformanceMetrics,
    ) -> PerformanceSummary {
        PerformanceSummary {
            total_return: metrics.total_return,
            win_rate: Decimal::try_from(metrics.win_rate).unwrap_or(dec!(0)),
            sharpe_ratio: Decimal::try_from(metrics.sharpe_ratio).unwrap_or(dec!(0)),
            portfolio_value: metrics.portfolio_value,
            daily_pnl: Decimal::from(250), // Placeholder
            best_performing_token: "ETH".to_string(), // Placeholder
            worst_performing_token: "LINK".to_string(), // Placeholder
        }
    }

    async fn build_gas_optimization_widget(&self, market_intel: &str) -> GasOptimizationWidget {
        let mut current_gas_prices = HashMap::new();
        let mut optimal_hours = Vec::new();
        let mut next_optimal = Utc::now() + Duration::hours(2);

        // Placeholder gas optimization data
        current_gas_prices.insert(1, dec!(25)); // Ethereum mainnet
        optimal_hours.push(2); // 2 AM UTC
        optimal_hours.push(14); // 2 PM UTC

        optimal_hours.sort();
        optimal_hours.dedup();

        GasOptimizationWidget {
            current_gas_prices,
            optimal_trading_hours: optimal_hours,
            potential_savings: Decimal::from(50), // $50 potential savings
            next_optimal_time: next_optimal,
            recommendations: vec![
                "Trade during off-peak hours to save on gas".to_string(),
                "Consider Layer 2 solutions for smaller trades".to_string(),
                "Batch multiple transactions when possible".to_string(),
            ],
        }
    }

    async fn build_liquidity_widget(&self, market_intel: &str) -> LiquidityWidget {
        let mut high_liquidity_pairs = Vec::new();
        let mut low_liquidity_warnings = Vec::new();
        let mut optimal_trade_sizes = HashMap::new();
        let mut slippage_estimates = HashMap::new();

        // Placeholder liquidity data
        let liquidity_info = LiquidityInfo {
            pair: "ETH/USDC".to_string(),
            dex: "Uniswap V3".to_string(),
            liquidity: dec!(10000000),
            depth: dec!(1000000),
            spread: dec!(0.001),
        };

        high_liquidity_pairs.push(liquidity_info);
        optimal_trade_sizes.insert("ETH/USDC".to_string(), Decimal::from(10000));
        slippage_estimates.insert("ETH/USDC".to_string(), dec!(0.005));

        LiquidityWidget {
            high_liquidity_pairs,
            low_liquidity_warnings,
            optimal_trade_sizes,
            slippage_estimates,
        }
    }

    async fn filter_risk_alerts(&self, insights: &[PersonalizedInsight]) -> Vec<PersonalizedInsight> {
        insights.iter()
            .filter(|i| matches!(i.insight_type, crate::user_retention::trading_insights::personalization_engine::InsightType::RiskWarning))
            .cloned()
            .collect()
    }

    fn default_config(&self, user_id: Uuid) -> DashboardConfiguration {
        DashboardConfiguration {
            user_id,
            layout_preferences: LayoutPreferences {
                widget_order: vec![
                    "market_overview".to_string(),
                    "personalized_feed".to_string(),
                    "performance_summary".to_string(),
                    "gas_optimization".to_string(),
                    "liquidity_insights".to_string(),
                ],
                grid_layout: HashMap::new(),
                theme: "dark".to_string(),
                compact_mode: false,
            },
            widget_settings: WidgetSettings {
                show_market_overview: true,
                show_personalized_feed: true,
                show_performance_summary: true,
                show_gas_optimization: true,
                show_liquidity_insights: true,
                max_insights_displayed: 10,
                max_opportunities_displayed: 5,
            },
            notification_preferences: NotificationPreferences {
                critical_alerts: true,
                opportunity_alerts: true,
                gas_optimization_alerts: false,
                performance_updates: true,
                market_updates: false,
            },
            refresh_intervals: RefreshIntervals {
                market_data: 30,
                personalized_insights: 300,
                performance_data: 60,
                gas_prices: 60,
                liquidity_data: 120,
            },
        }
    }

    // Chart data methods (placeholder implementations)
    async fn get_portfolio_chart_data(&self, user_id: Uuid, time_range: Duration) -> Result<ChartData, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ChartData {
            chart_type: ChartType::PortfolioPerformance,
            data_points: vec![], // Placeholder
            labels: vec![],
            metadata: HashMap::new(),
        })
    }

    async fn get_market_chart_data(&self, time_range: Duration) -> Result<ChartData, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ChartData {
            chart_type: ChartType::MarketOverview,
            data_points: vec![], // Placeholder
            labels: vec![],
            metadata: HashMap::new(),
        })
    }

    async fn get_gas_chart_data(&self, time_range: Duration) -> Result<ChartData, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ChartData {
            chart_type: ChartType::GasPrices,
            data_points: vec![], // Placeholder
            labels: vec![],
            metadata: HashMap::new(),
        })
    }

    async fn get_liquidity_chart_data(&self, time_range: Duration) -> Result<ChartData, Box<dyn std::error::Error + Send + Sync>> {
        Ok(ChartData {
            chart_type: ChartType::LiquidityTrends,
            data_points: vec![], // Placeholder
            labels: vec![],
            metadata: HashMap::new(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartType {
    PortfolioPerformance,
    MarketOverview,
    GasPrices,
    LiquidityTrends,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DashboardComponent {
    MarketOverview,
    PersonalizedFeed,
    PerformanceSummary,
    GasOptimization,
    LiquidityInsights,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub chart_type: ChartType,
    pub data_points: Vec<DataPoint>,
    pub labels: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
    use crate::user_retention::trading_insights::market_intelligence::MarketIntelligenceEngine;
    use crate::user_retention::trading_insights::personalization_engine::PersonalizationEngine;
    use crate::aggregator::DEXAggregator;
    use crate::risk_management::redis_cache::RiskCache;
    use redis::Client;

    #[tokio::test]
    async fn test_dashboard_service_creation() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        let personalization_engine = Arc::new(PersonalizationEngine::new(
            user_analyzer.clone(),
            market_intelligence.clone(),
            cache.clone(),
        ));
        
        let service = DashboardService::new(
            market_intelligence,
            personalization_engine,
            user_analyzer,
            cache,
        );
        
        let user_id = Uuid::new_v4();
        
        // Test dashboard data generation
        let dashboard = service.generate_dashboard_data(user_id).await;
        assert!(dashboard.is_ok());
        
        // Test configuration management
        let config = service.get_dashboard_config(user_id).await;
        assert_eq!(config.user_id, user_id);
        
        // Test market overview
        let overview = service.get_market_overview().await;
        assert!(overview.is_ok());
        
        // Test opportunity feed
        let opportunities = service.get_opportunity_feed(user_id, Some(5)).await;
        assert!(opportunities.is_ok());
    }

    #[tokio::test]
    async fn test_dashboard_caching() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        let personalization_engine = Arc::new(PersonalizationEngine::new(
            user_analyzer.clone(),
            market_intelligence.clone(),
            cache.clone(),
        ));
        
        let service = DashboardService::new(
            market_intelligence,
            personalization_engine,
            user_analyzer,
            cache,
        );
        
        let user_id = Uuid::new_v4();
        
        // Generate dashboard to populate cache
        let _ = service.generate_dashboard_data(user_id).await;
        
        // Test cached retrieval
        let cached = service.get_cached_dashboard(user_id).await;
        assert!(cached.is_some());
    }
}
