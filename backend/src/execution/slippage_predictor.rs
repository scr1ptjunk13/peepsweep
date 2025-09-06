use crate::aggregator::DEXAggregator;
use crate::types::QuoteParams;
use crate::cache::CacheManager;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum SlippagePredictionError {
    #[error("Insufficient historical data for prediction")]
    InsufficientData,
    #[error("Invalid trade parameters: {0}")]
    InvalidParameters(String),
    #[error("Market data unavailable")]
    MarketDataUnavailable,
    #[error("Prediction calculation failed: {0}")]
    CalculationError(String),
    #[error("DEX aggregator error: {0}")]
    DexAggregatorError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageDataPoint {
    pub timestamp: u64,
    pub trade_size_usd: Decimal,
    pub expected_output: Decimal,
    pub actual_output: Decimal,
    pub slippage_bps: Decimal,
    pub dex_name: String,
    pub token_pair: String,
    pub volatility: Decimal,
    pub liquidity_depth: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditions {
    pub volatility_24h: Decimal,
    pub volume_24h: Decimal,
    pub liquidity_depth: Decimal,
    pub spread_bps: Decimal,
    pub market_impact_factor: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippagePrediction {
    pub predicted_slippage_bps: Decimal,
    pub confidence_score: Decimal,
    pub market_impact_estimate: Decimal,
    pub recommended_max_trade_size: Decimal,
    pub volatility_adjustment: Decimal,
    pub liquidity_score: Decimal,
    pub prediction_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityAnalysis {
    pub total_liquidity_usd: Decimal,
    pub depth_at_1_percent: Decimal,
    pub depth_at_5_percent: Decimal,
    pub average_spread_bps: Decimal,
    pub liquidity_distribution: HashMap<String, Decimal>,
}

pub struct SlippagePredictor {
    historical_data: Arc<RwLock<Vec<SlippageDataPoint>>>,
    market_conditions_cache: Arc<RwLock<HashMap<String, MarketConditions>>>,
    dex_aggregator: Arc<DEXAggregator>,
    prediction_models: PredictionModels,
}

struct PredictionModels {
    base_slippage_model: BaseSlippageModel,
    volatility_model: VolatilityModel,
    market_impact_model: MarketImpactModel,
}

struct BaseSlippageModel {
    trade_size_coefficient: Decimal,
    liquidity_coefficient: Decimal,
    volatility_coefficient: Decimal,
    intercept: Decimal,
}

struct VolatilityModel {
    volatility_multiplier: Decimal,
}

struct MarketImpactModel {
    square_root_coefficient: Decimal,
    linear_coefficient: Decimal,
    liquidity_adjustment: Decimal,
}

impl SlippagePredictor {
    pub fn new(dex_aggregator: Arc<DEXAggregator>) -> Self {
        Self {
            historical_data: Arc::new(RwLock::new(Vec::new())),
            market_conditions_cache: Arc::new(RwLock::new(HashMap::new())),
            dex_aggregator,
            prediction_models: PredictionModels::default(),
        }
    }

    pub async fn predict_slippage(
        &self,
        from_token: &str,
        to_token: &str,
        amount_in: Decimal,
    ) -> Result<SlippagePrediction, SlippagePredictionError> {
        if amount_in <= Decimal::ZERO {
            return Err(SlippagePredictionError::InvalidParameters(
                "Trade amount must be positive".to_string(),
            ));
        }

        let token_pair = format!("{}/{}", from_token, to_token);
        let market_conditions = self.get_market_conditions(&token_pair).await?;
        let liquidity_analysis = self.analyze_liquidity(from_token, to_token, amount_in).await?;
        let historical_data = self.get_relevant_historical_data(&token_pair).await;
        
        let base_slippage = self.calculate_base_slippage(
            amount_in,
            &market_conditions,
            &liquidity_analysis,
            &historical_data,
        )?;
        
        let volatility_adjustment = self.calculate_volatility_adjustment(&market_conditions)?;
        let market_impact = self.calculate_market_impact(amount_in, &liquidity_analysis)?;
        let confidence_score = self.calculate_confidence_score(&historical_data, &market_conditions)?;
        
        let raw_predicted_slippage = base_slippage + volatility_adjustment + market_impact;
        let predicted_slippage_bps = raw_predicted_slippage.min(Decimal::from(150));
        
        info!("Slippage calculation: base={}, volatility={}, market_impact={}, raw_total={}, capped_total={}", 
              base_slippage, volatility_adjustment, market_impact, raw_predicted_slippage, predicted_slippage_bps);
        let recommended_max_trade_size = self.calculate_recommended_max_size(&liquidity_analysis)?;
        
        Ok(SlippagePrediction {
            predicted_slippage_bps,
            confidence_score,
            market_impact_estimate: market_impact,
            recommended_max_trade_size,
            volatility_adjustment,
            liquidity_score: liquidity_analysis.depth_at_1_percent,
            prediction_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    pub async fn analyze_liquidity(
        &self,
        token_in: &str,
        token_out: &str,
        amount_in: Decimal,
    ) -> Result<LiquidityAnalysis, SlippagePredictionError> {
        let quote_params = QuoteParams {
            token_in: token_in.to_string(),
            token_in_address: None,
            token_in_decimals: None,
            token_out: token_out.to_string(),
            token_out_address: None,
            token_out_decimals: None,
            amount_in: amount_in.to_string(),
            chain: Some("ethereum".to_string()),
            slippage: Some(1.0),
        };

        let quote = self.dex_aggregator.get_quote_with_guaranteed_routes(&quote_params).await
            .map_err(|e| SlippagePredictionError::DexAggregatorError(e.to_string()))?;

        let mut total_liquidity = Decimal::ZERO;
        let mut liquidity_distribution = HashMap::new();

        for route in &quote.routes {
            let route_liquidity = Decimal::from(1_000_000); // Simplified estimation
            total_liquidity += route_liquidity;
            
            let dex_liquidity = liquidity_distribution
                .entry(route.dex.clone())
                .or_insert(Decimal::ZERO);
            *dex_liquidity += route_liquidity;
        }

        let depth_1_percent = total_liquidity / Decimal::from(10);
        let depth_5_percent = total_liquidity / Decimal::from(2);
        let average_spread_bps = Decimal::from(30);

        for (_, liquidity) in liquidity_distribution.iter_mut() {
            if total_liquidity > Decimal::ZERO {
                *liquidity = (*liquidity / total_liquidity) * Decimal::from(100);
            }
        }

        Ok(LiquidityAnalysis {
            total_liquidity_usd: total_liquidity,
            depth_at_1_percent: depth_1_percent,
            depth_at_5_percent: depth_5_percent,
            average_spread_bps,
            liquidity_distribution,
        })
    }

    pub async fn record_slippage_data(&self, data_point: SlippageDataPoint) -> Result<(), SlippagePredictionError> {
        let mut historical_data = self.historical_data.write().await;
        historical_data.push(data_point.clone());

        if historical_data.len() > 10_000 {
            historical_data.drain(0..1_000);
        }

        info!(
            "Recorded slippage data: {} bps for {} trade on {}",
            data_point.slippage_bps, data_point.token_pair, data_point.dex_name
        );

        Ok(())
    }

    async fn get_market_conditions(&self, token_pair: &str) -> Result<MarketConditions, SlippagePredictionError> {
        let cache = self.market_conditions_cache.read().await;
        
        if let Some(conditions) = cache.get(token_pair) {
            return Ok(conditions.clone());
        }
        
        drop(cache);

        let historical_data = self.get_relevant_historical_data(token_pair).await;
        
        let conditions = if historical_data.len() < 10 {
            MarketConditions {
                volatility_24h: Decimal::from(10),
                volume_24h: Decimal::from(1_000_000),
                liquidity_depth: Decimal::from(500_000),
                spread_bps: Decimal::from(5),
                market_impact_factor: Decimal::from(3),
            }
        } else {
            let volatility_24h = self.calculate_volatility(&historical_data)?;
            let volume_24h = self.calculate_volume(&historical_data)?;
            let liquidity_depth = self.estimate_liquidity_depth(&historical_data)?;
            let spread_bps = self.calculate_average_spread(&historical_data)?;
            let market_impact_factor = self.calculate_market_impact_factor(&historical_data)?;

            MarketConditions {
                volatility_24h,
                volume_24h,
                liquidity_depth,
                spread_bps,
                market_impact_factor,
            }
        };

        let mut cache = self.market_conditions_cache.write().await;
        cache.insert(token_pair.to_string(), conditions.clone());

        Ok(conditions)
    }

    fn calculate_base_slippage(
        &self,
        trade_size: Decimal,
        market_conditions: &MarketConditions,
        liquidity_analysis: &LiquidityAnalysis,
        _historical_data: &[SlippageDataPoint],
    ) -> Result<Decimal, SlippagePredictionError> {
        let model = &self.prediction_models.base_slippage_model;

        let normalized_trade_size = if liquidity_analysis.total_liquidity_usd > Decimal::ZERO {
            trade_size / liquidity_analysis.total_liquidity_usd
        } else {
            Decimal::from(1)
        };

        let base_slippage = model.intercept
            + (model.trade_size_coefficient * normalized_trade_size)
            + (model.liquidity_coefficient * liquidity_analysis.depth_at_1_percent)
            + (model.volatility_coefficient * market_conditions.volatility_24h);

        Ok(base_slippage.max(Decimal::from(1)).min(Decimal::from(50)))
    }

    fn calculate_volatility_adjustment(&self, market_conditions: &MarketConditions) -> Result<Decimal, SlippagePredictionError> {
        let model = &self.prediction_models.volatility_model;
        let volatility_impact = market_conditions.volatility_24h * model.volatility_multiplier;
        Ok(volatility_impact.min(Decimal::from(10)))
    }

    fn calculate_market_impact(&self, trade_size: Decimal, liquidity_analysis: &LiquidityAnalysis) -> Result<Decimal, SlippagePredictionError> {
        let model = &self.prediction_models.market_impact_model;
        
        if liquidity_analysis.total_liquidity_usd <= Decimal::ZERO {
            return Ok(Decimal::from(5));
        }

        let size_ratio = trade_size / liquidity_analysis.total_liquidity_usd;
        let sqrt_impact = model.square_root_coefficient * size_ratio.sqrt().unwrap_or(Decimal::ZERO);
        let linear_impact = model.linear_coefficient * size_ratio;
        
        let liquidity_adjustment = if liquidity_analysis.depth_at_1_percent > Decimal::ZERO {
            model.liquidity_adjustment / liquidity_analysis.depth_at_1_percent.sqrt().unwrap_or(Decimal::ONE)
        } else {
            model.liquidity_adjustment
        };

        let total_impact = sqrt_impact + linear_impact + liquidity_adjustment;
        Ok(total_impact.min(Decimal::from(20)))
    }

    fn calculate_confidence_score(
        &self,
        historical_data: &[SlippageDataPoint],
        market_conditions: &MarketConditions,
    ) -> Result<Decimal, SlippagePredictionError> {
        let data_confidence = if historical_data.len() >= 100 {
            Decimal::from(80)
        } else if historical_data.len() >= 50 {
            Decimal::from(60)
        } else if historical_data.len() >= 10 {
            Decimal::from(40)
        } else {
            Decimal::from(20)
        };

        let volatility_penalty = market_conditions.volatility_24h / Decimal::from(10);
        let adjusted_confidence = (data_confidence - volatility_penalty).max(Decimal::from(10));

        Ok(adjusted_confidence / Decimal::from(100))
    }

    fn calculate_recommended_max_size(&self, liquidity_analysis: &LiquidityAnalysis) -> Result<Decimal, SlippagePredictionError> {
        let max_size = liquidity_analysis.total_liquidity_usd * Decimal::from_str("0.01").unwrap();
        Ok(max_size.max(Decimal::from(1_000)).min(Decimal::from(10_000_000)))
    }

    async fn get_relevant_historical_data(&self, token_pair: &str) -> Vec<SlippageDataPoint> {
        let historical_data = self.historical_data.read().await;
        let cutoff_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() - 86400;

        historical_data
            .iter()
            .filter(|data| data.token_pair == token_pair && data.timestamp >= cutoff_time)
            .cloned()
            .collect()
    }

    fn calculate_volatility(&self, data: &[SlippageDataPoint]) -> Result<Decimal, SlippagePredictionError> {
        if data.len() < 2 {
            return Ok(Decimal::from(50));
        }

        let slippages: Vec<Decimal> = data.iter().map(|d| d.slippage_bps).collect();
        let mean = slippages.iter().sum::<Decimal>() / Decimal::from(slippages.len());
        
        let variance = slippages
            .iter()
            .map(|s| (*s - mean).powi(2))
            .sum::<Decimal>() / Decimal::from(slippages.len());
            
        Ok(variance.sqrt().unwrap_or(Decimal::from(50)))
    }

    fn calculate_volume(&self, data: &[SlippageDataPoint]) -> Result<Decimal, SlippagePredictionError> {
        Ok(data.iter().map(|d| d.trade_size_usd).sum())
    }

    fn estimate_liquidity_depth(&self, data: &[SlippageDataPoint]) -> Result<Decimal, SlippagePredictionError> {
        if data.is_empty() {
            return Ok(Decimal::from(500_000));
        }

        let avg_trade_size = data.iter().map(|d| d.trade_size_usd).sum::<Decimal>() / Decimal::from(data.len());
        let avg_slippage = data.iter().map(|d| d.slippage_bps).sum::<Decimal>() / Decimal::from(data.len());
        
        let estimated_depth = if avg_slippage > Decimal::ZERO {
            avg_trade_size * Decimal::from(100) / avg_slippage
        } else {
            avg_trade_size * Decimal::from(50)
        };

        Ok(estimated_depth.max(Decimal::from(100_000)))
    }

    fn calculate_average_spread(&self, data: &[SlippageDataPoint]) -> Result<Decimal, SlippagePredictionError> {
        if data.is_empty() {
            return Ok(Decimal::from(30));
        }

        let min_slippage = data.iter().map(|d| d.slippage_bps).min().unwrap_or(Decimal::from(30));
        Ok(min_slippage.max(Decimal::from(5)))
    }

    fn calculate_market_impact_factor(&self, data: &[SlippageDataPoint]) -> Result<Decimal, SlippagePredictionError> {
        if data.len() < 5 {
            return Ok(Decimal::from(15));
        }

        let sizes: Vec<Decimal> = data.iter().map(|d| d.trade_size_usd).collect();
        let slippages: Vec<Decimal> = data.iter().map(|d| d.slippage_bps).collect();
        
        let size_mean = sizes.iter().sum::<Decimal>() / Decimal::from(sizes.len());
        let slippage_mean = slippages.iter().sum::<Decimal>() / Decimal::from(slippages.len());
        
        let correlation = sizes
            .iter()
            .zip(slippages.iter())
            .map(|(s, sl)| (*s - size_mean) * (*sl - slippage_mean))
            .sum::<Decimal>() / Decimal::from(sizes.len());
            
        Ok(correlation.abs().max(Decimal::from(10)))
    }
}

impl Default for MarketConditions {
    fn default() -> Self {
        Self {
            volatility_24h: Decimal::from_str("1.0").unwrap(),
            volume_24h: Decimal::from(1_000_000),
            liquidity_depth: Decimal::from(500_000),
            spread_bps: Decimal::from_str("2.0").unwrap(),
            market_impact_factor: Decimal::from_str("0.1").unwrap(),
        }
    }
}

impl Default for PredictionModels {
    fn default() -> Self {
        Self {
            base_slippage_model: BaseSlippageModel {
                intercept: Decimal::from_str("0.1").unwrap(),
                trade_size_coefficient: Decimal::from_str("0.001").unwrap(),
                liquidity_coefficient: Decimal::from_str("-0.0001").unwrap(),
                volatility_coefficient: Decimal::from_str("0.01").unwrap(),
            },
            volatility_model: VolatilityModel {
                volatility_multiplier: Decimal::from_str("0.1").unwrap(),
            },
            market_impact_model: MarketImpactModel {
                square_root_coefficient: Decimal::from_str("0.1").unwrap(),
                linear_coefficient: Decimal::from_str("0.01").unwrap(),
                liquidity_adjustment: Decimal::from_str("0.01").unwrap(),
            },
        }
    }
}
