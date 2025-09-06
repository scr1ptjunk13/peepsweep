use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

use crate::user_retention::trading_insights::market_intelligence::{MarketIntelligenceEngine, MarketData, TokenTrend};
use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
use crate::risk_management::redis_cache::RiskCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePrediction {
    pub token_symbol: String,
    pub current_price: Decimal,
    pub predicted_price: Decimal,
    pub prediction_timeframe: Duration,
    pub confidence: f64,
    pub prediction_type: PredictionType,
    pub supporting_factors: Vec<String>,
    pub risk_factors: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictionType {
    Bullish,
    Bearish,
    Neutral,
    Volatile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingPrediction {
    pub action: TimingAction,
    pub token_pair: String,
    pub optimal_time: DateTime<Utc>,
    pub time_window: Duration,
    pub confidence: f64,
    pub expected_conditions: ExpectedConditions,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimingAction {
    Buy,
    Sell,
    Hold,
    Accumulate,
    Distribute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedConditions {
    pub gas_price_range: (Decimal, Decimal),
    pub liquidity_threshold: Decimal,
    pub volatility_range: (f64, f64),
    pub volume_threshold: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityForecast {
    pub token_pair: String,
    pub dex: String,
    pub current_liquidity: Decimal,
    pub predicted_liquidity: Decimal,
    pub forecast_timeframe: Duration,
    pub confidence: f64,
    pub trend: LiquidityTrend,
    pub impact_factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LiquidityTrend {
    Increasing,
    Decreasing,
    Stable,
    Cyclical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSentimentAnalysis {
    pub overall_sentiment: f64, // -1.0 to 1.0
    pub sentiment_trend: SentimentTrend,
    pub confidence: f64,
    pub key_indicators: Vec<SentimentIndicator>,
    pub token_sentiments: HashMap<String, f64>,
    pub sector_sentiments: HashMap<String, f64>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SentimentTrend {
    Improving,
    Deteriorating,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentIndicator {
    pub indicator_type: IndicatorType,
    pub value: f64,
    pub weight: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndicatorType {
    VolumeAnalysis,
    PriceAction,
    LiquidityFlow,
    GasUsage,
    DEXActivity,
    CrossChainFlow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveModel {
    pub model_id: String,
    pub model_type: ModelType,
    pub accuracy: f64,
    pub last_trained: DateTime<Utc>,
    pub training_data_size: u32,
    pub features: Vec<String>,
    pub parameters: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    LinearRegression,
    RandomForest,
    NeuralNetwork,
    TimeSeriesARIMA,
    EnsembleMethod,
}

pub struct PredictiveAnalytics {
    market_intelligence: Arc<MarketIntelligenceEngine>,
    user_analyzer: Arc<UserPerformanceAnalyzer>,
    cache: Arc<RiskCache>,
    models: Arc<RwLock<HashMap<String, PredictiveModel>>>,
    predictions_cache: Arc<RwLock<HashMap<String, Vec<PricePrediction>>>>,
    timing_cache: Arc<RwLock<HashMap<String, Vec<TimingPrediction>>>>,
    liquidity_cache: Arc<RwLock<HashMap<String, Vec<LiquidityForecast>>>>,
    sentiment_cache: Arc<RwLock<Option<MarketSentimentAnalysis>>>,
}

impl PredictiveAnalytics {
    pub fn new(
        market_intelligence: Arc<MarketIntelligenceEngine>,
        user_analyzer: Arc<UserPerformanceAnalyzer>,
        cache: Arc<RiskCache>,
    ) -> Self {
        let mut models = HashMap::new();
        
        // Initialize default models
        models.insert("price_predictor".to_string(), PredictiveModel {
            model_id: "price_predictor".to_string(),
            model_type: ModelType::EnsembleMethod,
            accuracy: 0.72,
            last_trained: Utc::now() - Duration::days(1),
            training_data_size: 10000,
            features: vec![
                "price_momentum".to_string(),
                "volume_trend".to_string(),
                "liquidity_change".to_string(),
                "gas_price_correlation".to_string(),
                "cross_dex_arbitrage".to_string(),
            ],
            parameters: HashMap::new(),
        });

        Self {
            market_intelligence,
            user_analyzer,
            cache,
            models: Arc::new(RwLock::new(models)),
            predictions_cache: Arc::new(RwLock::new(HashMap::new())),
            timing_cache: Arc::new(RwLock::new(HashMap::new())),
            liquidity_cache: Arc::new(RwLock::new(HashMap::new())),
            sentiment_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Generate price trend predictions using historical data
    pub async fn predict_price_trends(
        &self,
        tokens: Vec<String>,
        timeframe: Duration,
    ) -> Result<Vec<PricePrediction>, Box<dyn std::error::Error + Send + Sync>> {
        let mut predictions = Vec::new();
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        for token in tokens {
            // Find token trend data
            if let Some(token_trend) = market_intel.token_trends.iter()
                .find(|t| t.token_symbol == token) {
                
                let prediction = self.generate_price_prediction(token_trend, timeframe).await;
                predictions.push(prediction);
            }
        }

        // Cache predictions
        let mut cache = self.predictions_cache.write().await;
        cache.insert("latest".to_string(), predictions.clone());

        Ok(predictions)
    }

    /// Generate optimal timing recommendations
    pub async fn predict_optimal_timing(
        &self,
        token_pairs: Vec<String>,
    ) -> Result<Vec<TimingPrediction>, Box<dyn std::error::Error + Send + Sync>> {
        let mut predictions = Vec::new();
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        for pair in token_pairs {
            // Analyze gas patterns for timing
            let gas_prediction = self.predict_gas_optimal_timing(&pair, &market_intel).await;
            predictions.push(gas_prediction);

            // Analyze liquidity patterns for timing
            let liquidity_prediction = self.predict_liquidity_optimal_timing(&pair, &market_intel).await;
            predictions.push(liquidity_prediction);
        }

        // Cache timing predictions
        let mut cache = self.timing_cache.write().await;
        cache.insert("latest".to_string(), predictions.clone());

        Ok(predictions)
    }

    /// Forecast liquidity for better execution
    pub async fn forecast_liquidity(
        &self,
        token_pairs: Vec<String>,
        timeframe: Duration,
    ) -> Result<Vec<LiquidityForecast>, Box<dyn std::error::Error + Send + Sync>> {
        let mut forecasts = Vec::new();
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;

        for pair in token_pairs {
            // Find liquidity pattern for this pair
            if let Some(pattern) = market_intel.liquidity_patterns.iter()
                .find(|p| p.token_pair == pair) {
                
                let forecast = LiquidityForecast {
                    token_pair: pair.clone(),
                    dex: "Uniswap V3".to_string(),
                    current_liquidity: pattern.average_liquidity,
                    predicted_liquidity: self.predict_future_liquidity(pattern, timeframe).await,
                    forecast_timeframe: timeframe,
                    confidence: pattern.confidence_score,
                    trend: self.determine_liquidity_trend_prediction(pattern).await,
                    impact_factors: vec![
                        "Market volatility".to_string(),
                        "Token price momentum".to_string(),
                        "DEX incentives".to_string(),
                    ],
                };
                forecasts.push(forecast);
            }
        }

        // Cache liquidity forecasts
        let mut cache = self.liquidity_cache.write().await;
        cache.insert("latest".to_string(), forecasts.clone());

        Ok(forecasts)
    }

    /// Analyze market sentiment using multiple indicators
    pub async fn analyze_market_sentiment(&self) -> Result<MarketSentimentAnalysis, Box<dyn std::error::Error + Send + Sync>> {
        let market_intel = self.market_intelligence.generate_market_intelligence().await?;
        
        // Calculate sentiment indicators
        let volume_indicator = self.calculate_volume_sentiment(&market_intel).await;
        let price_indicator = self.calculate_price_action_sentiment(&market_intel).await;
        let liquidity_indicator = self.calculate_liquidity_sentiment(&market_intel).await;
        let gas_indicator = self.calculate_gas_sentiment(&market_intel).await;

        let indicators = vec![volume_indicator, price_indicator, liquidity_indicator, gas_indicator];
        
        // Calculate weighted overall sentiment
        let overall_sentiment = indicators.iter()
            .map(|i| i.value * i.weight)
            .sum::<f64>() / indicators.iter().map(|i| i.weight).sum::<f64>();

        // Determine sentiment trend
        let sentiment_trend = if overall_sentiment > 0.1 {
            SentimentTrend::Improving
        } else if overall_sentiment < -0.1 {
            SentimentTrend::Deteriorating
        } else {
            SentimentTrend::Stable
        };

        // Calculate token-specific sentiments
        let mut token_sentiments = HashMap::new();
        for trend in &market_intel.token_trends {
            let sentiment = self.calculate_token_sentiment(trend).await;
            token_sentiments.insert(trend.token_symbol.clone(), sentiment);
        }

        // Calculate sector sentiments (placeholder)
        let mut sector_sentiments = HashMap::new();
        sector_sentiments.insert("DeFi".to_string(), 0.3);
        sector_sentiments.insert("Layer1".to_string(), 0.1);
        sector_sentiments.insert("Layer2".to_string(), 0.4);

        let analysis = MarketSentimentAnalysis {
            overall_sentiment,
            sentiment_trend,
            confidence: 0.75,
            key_indicators: indicators,
            token_sentiments,
            sector_sentiments,
            generated_at: Utc::now(),
        };

        // Cache sentiment analysis
        let mut cache = self.sentiment_cache.write().await;
        *cache = Some(analysis.clone());

        Ok(analysis)
    }

    /// Get cached predictions
    pub async fn get_cached_predictions(&self, cache_key: &str) -> Option<Vec<PricePrediction>> {
        let cache = self.predictions_cache.read().await;
        cache.get(cache_key).cloned()
    }

    /// Get cached timing predictions
    pub async fn get_cached_timing(&self, cache_key: &str) -> Option<Vec<TimingPrediction>> {
        let cache = self.timing_cache.read().await;
        cache.get(cache_key).cloned()
    }

    /// Get cached liquidity forecasts
    pub async fn get_cached_liquidity(&self, cache_key: &str) -> Option<Vec<LiquidityForecast>> {
        let cache = self.liquidity_cache.read().await;
        cache.get(cache_key).cloned()
    }

    /// Get cached sentiment analysis
    pub async fn get_cached_sentiment(&self) -> Option<MarketSentimentAnalysis> {
        let cache = self.sentiment_cache.read().await;
        cache.clone()
    }

    // Helper methods

    async fn generate_price_prediction(&self, token_trend: &TokenTrend, timeframe: Duration) -> PricePrediction {
        // Simulate ML-based price prediction
        let momentum_factor: f64 = token_trend.price_momentum.try_into().unwrap_or(0.0);
        let technical_factor = token_trend.technical_score;
        let sentiment_factor = token_trend.social_sentiment;

        // Weighted prediction calculation
        let prediction_multiplier = 1.0 + (momentum_factor * 0.4 + technical_factor * 0.3 + sentiment_factor * 0.3) * 0.1;
        let current_price = Decimal::from(100); // Placeholder current price
        let predicted_price = current_price * Decimal::try_from(prediction_multiplier).unwrap_or(Decimal::ONE);

        let prediction_type = if prediction_multiplier > 1.05 {
            PredictionType::Bullish
        } else if prediction_multiplier < 0.95 {
            PredictionType::Bearish
        } else {
            PredictionType::Neutral
        };

        PricePrediction {
            token_symbol: token_trend.token_symbol.clone(),
            current_price,
            predicted_price,
            prediction_timeframe: timeframe,
            confidence: (technical_factor + sentiment_factor) / 2.0,
            prediction_type,
            supporting_factors: vec![
                format!("Strong momentum: {}%", momentum_factor * 100.0),
                format!("Technical score: {:.1}/1.0", technical_factor),
                format!("Positive sentiment: {:.1}%", sentiment_factor * 100.0),
            ],
            risk_factors: vec![
                "Market volatility".to_string(),
                "Regulatory uncertainty".to_string(),
            ],
            generated_at: Utc::now(),
        }
    }

    async fn predict_gas_optimal_timing(
        &self,
        pair: &str,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> TimingPrediction {
        // Find the best gas optimization opportunity
        let mut best_hour = 4u8; // Default to 4 AM UTC
        let mut best_savings = 0.3;

        for gas_pattern in &market_intel.gas_patterns {
            if let Some(opt) = gas_pattern.optimization_opportunities.first() {
                if let Ok(savings) = TryInto::<f64>::try_into(opt.potential_savings) {
                    if savings > best_savings {
                        best_hour = opt.recommended_hour;
                        best_savings = savings;
                    }
                }
            }
        }

        let optimal_time = Utc::now().date_naive()
            .and_hms_opt(best_hour as u32, 0, 0)
            .unwrap()
            .and_utc();

        TimingPrediction {
            action: TimingAction::Buy,
            token_pair: pair.to_string(),
            optimal_time,
            time_window: Duration::hours(2),
            confidence: 0.8,
            expected_conditions: ExpectedConditions {
                gas_price_range: (Decimal::from(10), Decimal::from(20)),
                liquidity_threshold: Decimal::from(1000000),
                volatility_range: (0.1, 0.3),
                volume_threshold: Decimal::from(100000),
            },
            reasoning: format!("Gas prices are typically {}% lower at {}:00 UTC", 
                best_savings * 100.0, best_hour),
        }
    }

    async fn predict_liquidity_optimal_timing(
        &self,
        pair: &str,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> TimingPrediction {
        // Find peak liquidity hours for this pair
        let mut peak_hour = 15u8; // Default to 3 PM UTC
        
        for pattern in &market_intel.liquidity_patterns {
            if pattern.token_pair == pair && !pattern.peak_hours.is_empty() {
                peak_hour = pattern.peak_hours[0];
                break;
            }
        }

        let optimal_time = Utc::now().date_naive()
            .and_hms_opt(peak_hour as u32, 0, 0)
            .unwrap()
            .and_utc();

        TimingPrediction {
            action: TimingAction::Buy,
            token_pair: pair.to_string(),
            optimal_time,
            time_window: Duration::hours(3),
            confidence: 0.75,
            expected_conditions: ExpectedConditions {
                gas_price_range: (Decimal::from(20), Decimal::from(40)),
                liquidity_threshold: Decimal::from(2000000),
                volatility_range: (0.05, 0.2),
                volume_threshold: Decimal::from(500000),
            },
            reasoning: format!("Liquidity is typically highest at {}:00 UTC, reducing slippage", peak_hour),
        }
    }

    async fn predict_future_liquidity(
        &self,
        pattern: &crate::user_retention::trading_insights::market_intelligence::LiquidityPattern,
        timeframe: Duration,
    ) -> Decimal {
        // Simple trend-based prediction
        let trend_multiplier = match pattern.trend {
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Increasing => 1.1,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Decreasing => 0.9,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Stable => 1.0,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Volatile => 1.05,
        };

        pattern.average_liquidity * Decimal::try_from(trend_multiplier).unwrap_or(Decimal::ONE)
    }

    async fn determine_liquidity_trend_prediction(
        &self,
        pattern: &crate::user_retention::trading_insights::market_intelligence::LiquidityPattern,
    ) -> LiquidityTrend {
        match pattern.trend {
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Increasing => LiquidityTrend::Increasing,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Decreasing => LiquidityTrend::Decreasing,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Stable => LiquidityTrend::Stable,
            crate::user_retention::trading_insights::market_intelligence::LiquidityTrend::Volatile => LiquidityTrend::Cyclical,
        }
    }

    async fn calculate_volume_sentiment(
        &self,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> SentimentIndicator {
        let avg_volume: f64 = market_intel.market_data.iter()
            .map(|d| TryInto::<f64>::try_into(d.volume_24h).unwrap_or(0.0))
            .sum::<f64>() / market_intel.market_data.len() as f64;

        let sentiment = if avg_volume > 1000000.0 { 0.3 } else { -0.1 };

        SentimentIndicator {
            indicator_type: IndicatorType::VolumeAnalysis,
            value: sentiment,
            weight: 0.25,
            description: format!("Average 24h volume: ${:.0}K", avg_volume / 1000.0),
        }
    }

    async fn calculate_price_action_sentiment(
        &self,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> SentimentIndicator {
        let avg_change: f64 = market_intel.market_data.iter()
            .map(|d| TryInto::<f64>::try_into(d.price_change_24h).unwrap_or(0.0))
            .sum::<f64>() / market_intel.market_data.len() as f64;

        SentimentIndicator {
            indicator_type: IndicatorType::PriceAction,
            value: avg_change.clamp(-1.0, 1.0),
            weight: 0.3,
            description: format!("Average price change: {:.2}%", avg_change * 100.0),
        }
    }

    async fn calculate_liquidity_sentiment(
        &self,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> SentimentIndicator {
        let avg_liquidity: f64 = market_intel.liquidity_patterns.iter()
            .map(|p| TryInto::<f64>::try_into(p.average_liquidity).unwrap_or(0.0))
            .sum::<f64>() / market_intel.liquidity_patterns.len() as f64;

        let sentiment = if avg_liquidity > 2000000.0 { 0.2 } else { -0.2 };

        SentimentIndicator {
            indicator_type: IndicatorType::LiquidityFlow,
            value: sentiment,
            weight: 0.2,
            description: format!("Average liquidity: ${:.1}M", avg_liquidity / 1000000.0),
        }
    }

    async fn calculate_gas_sentiment(
        &self,
        market_intel: &crate::user_retention::trading_insights::market_intelligence::MarketIntelligence,
    ) -> SentimentIndicator {
        let avg_gas: f64 = market_intel.gas_patterns.iter()
            .map(|g| TryInto::<f64>::try_into(g.average_gas_price).unwrap_or(0.0))
            .sum::<f64>() / market_intel.gas_patterns.len() as f64;

        let sentiment = if avg_gas < 25.0 { 0.1 } else { -0.3 };

        SentimentIndicator {
            indicator_type: IndicatorType::GasUsage,
            value: sentiment,
            weight: 0.25,
            description: format!("Average gas price: {:.1} gwei", avg_gas),
        }
    }

    async fn calculate_token_sentiment(&self, trend: &TokenTrend) -> f64 {
        let momentum_score: f64 = trend.price_momentum.try_into().unwrap_or(0.0);
        let technical_score = trend.technical_score;
        let social_score = trend.social_sentiment;

        (momentum_score * 0.4 + technical_score * 0.3 + social_score * 0.3).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
    use crate::user_retention::trading_insights::MarketIntelligenceEngine;
    use crate::aggregator::DEXAggregator;
    use crate::risk_management::redis_cache::RiskCache;
    use redis::Client;

    #[tokio::test]
    async fn test_predictive_analytics_creation() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        
        let analytics = PredictiveAnalytics::new(market_intelligence, user_analyzer, cache);
        
        // Test price predictions
        let tokens = vec!["ETH".to_string(), "WBTC".to_string()];
        let predictions = analytics.predict_price_trends(tokens, Duration::days(1)).await;
        assert!(predictions.is_ok());
        
        // Test timing predictions
        let pairs = vec!["ETH/USDC".to_string()];
        let timing = analytics.predict_optimal_timing(pairs).await;
        assert!(timing.is_ok());
        
        // Test liquidity forecasts
        let pairs = vec!["ETH/USDC".to_string()];
        let forecasts = analytics.forecast_liquidity(pairs, Duration::hours(24)).await;
        assert!(forecasts.is_ok());
        
        // Test sentiment analysis
        let sentiment = analytics.analyze_market_sentiment().await;
        assert!(sentiment.is_ok());
    }
}
