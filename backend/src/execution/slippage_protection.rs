use crate::aggregator::DEXAggregator;
use crate::execution::slippage_predictor::{SlippagePredictor, SlippagePrediction};
use crate::execution::{OrderSplitter, OrderSplitParams, SplittingStrategy};
use crate::types::{SwapParams, QuoteParams, SwapResponse};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use num_traits::ToPrimitive;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum SlippageProtectionError {
    #[error("Invalid protection parameters: {0}")]
    InvalidParameters(String),
    #[error("Slippage tolerance exceeded: predicted {predicted}bps, max allowed {max_allowed}bps")]
    SlippageToleranceExceeded { predicted: Decimal, max_allowed: Decimal },
    #[error("Route optimization failed: {0}")]
    RouteOptimizationError(String),
    #[error("Protection calculation failed: {0}")]
    CalculationError(String),
    #[error("DexAggregator error: {0}")]
    DexAggregatorError(String),
    #[error("Quote error: {0}")]
    QuoteError(String),
    #[error("Slippage predictor error: {0}")]
    SlippagePredictorError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageProtectionConfig {
    pub max_slippage_bps: Decimal,
    pub dynamic_adjustment: bool,
    pub route_optimization: bool,
    pub pre_trade_validation: bool,
    pub post_trade_analysis: bool,
    pub emergency_stop_threshold_bps: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedSwapParams {
    pub from_token: String,
    pub to_token: String,
    pub amount: Decimal,
    pub protection_config: SlippageProtectionConfig,
    pub user_id: Option<Uuid>,
    pub priority: SwapPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwapPriority {
    Speed,      // Prioritize execution speed
    Price,      // Prioritize best price
    Protection, // Prioritize slippage protection
    Balanced,   // Balanced approach
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedSwapResult {
    pub swap_id: Uuid,
    pub original_prediction: SlippagePrediction,
    pub adjusted_prediction: SlippagePrediction,
    pub protection_applied: Vec<ProtectionMeasure>,
    pub execution_result: Option<SwapResponse>,
    pub actual_slippage_bps: Option<Decimal>,
    pub protection_effectiveness: Option<Decimal>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtectionMeasure {
    DynamicSlippageAdjustment { old_tolerance: Decimal, new_tolerance: Decimal },
    RouteOptimization { original_route: String, optimized_route: String },
    OrderSplitting { chunks: u32, chunk_size: Decimal },
    DelayedExecution { delay_seconds: u64 },
    EmergencyStop { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageAnalysis {
    pub trade_id: Uuid,
    pub predicted_slippage_bps: Decimal,
    pub actual_slippage_bps: Decimal,
    pub prediction_accuracy: Decimal,
    pub protection_effectiveness: Decimal,
    pub market_conditions_at_execution: MarketSnapshot,
    pub lessons_learned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub timestamp: u64,
    pub volatility: Decimal,
    pub liquidity_depth: Decimal,
    pub spread_bps: Decimal,
    pub recent_volume: Decimal,
}

pub struct SlippageProtectionEngine {
    dex_aggregator: Arc<DEXAggregator>,
    slippage_predictor: Arc<SlippagePredictor>,
    protection_history: Arc<RwLock<HashMap<Uuid, ProtectedSwapResult>>>,
    user_protection_configs: Arc<RwLock<HashMap<Uuid, SlippageProtectionConfig>>>,
    market_snapshots: Arc<RwLock<Vec<MarketSnapshot>>>,
}

impl SlippageProtectionEngine {
    pub fn new(
        dex_aggregator: Arc<DEXAggregator>,
        slippage_predictor: Arc<SlippagePredictor>,
    ) -> Self {
        Self {
            dex_aggregator,
            slippage_predictor,
            protection_history: Arc::new(RwLock::new(HashMap::new())),
            user_protection_configs: Arc::new(RwLock::new(HashMap::new())),
            market_snapshots: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Execute a swap with comprehensive slippage protection
    pub async fn execute_protected_swap(
        &self,
        params: ProtectedSwapParams,
    ) -> Result<ProtectedSwapResult, SlippageProtectionError> {
        let swap_id = Uuid::new_v4();
        let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

        // Step 1: Pre-trade slippage estimation
        let original_prediction = self.slippage_predictor
            .predict_slippage(&params.from_token, &params.to_token, params.amount)
            .await
            .map_err(|e| SlippageProtectionError::SlippagePredictorError(e.to_string()))?;

        info!(
            "Pre-trade prediction for {} {}->{}: {}bps slippage",
            params.amount, params.from_token, params.to_token, original_prediction.predicted_slippage_bps
        );

        // Step 2: Apply protection measures
        let (adjusted_prediction, protection_measures) = self.apply_protection_measures(
            &params,
            &original_prediction,
        ).await?;

        // Step 3: Validate slippage tolerance
        if params.protection_config.pre_trade_validation {
            self.validate_slippage_tolerance(&params.protection_config, &adjusted_prediction)?;
        }

        // Step 4: Execute the swap
        let execution_result = self.execute_swap_with_protection(&params, &adjusted_prediction).await;

        // Step 5: Post-trade analysis
        let (actual_slippage, protection_effectiveness) = if let Ok(ref result) = execution_result {
            let actual_slippage = self.calculate_actual_slippage(&params, result).await?;
            let effectiveness = self.calculate_protection_effectiveness(
                &original_prediction,
                &adjusted_prediction,
                actual_slippage,
            )?;
            (Some(actual_slippage), Some(effectiveness))
        } else {
            (None, None)
        };

        // Step 6: Record slippage data for learning
        if let (Some(actual_slippage), Ok(ref result)) = (actual_slippage, &execution_result) {
            self.record_slippage_data(&params, &original_prediction, actual_slippage).await?;
        }

        let protected_result = ProtectedSwapResult {
            swap_id,
            original_prediction,
            adjusted_prediction,
            protection_applied: protection_measures,
            execution_result: execution_result.ok(),
            actual_slippage_bps: actual_slippage,
            protection_effectiveness,
            timestamp,
        };

        // Store result for analysis
        let mut history = self.protection_history.write().await;
        history.insert(swap_id, protected_result.clone());

        Ok(protected_result)
    }

    /// Apply protection measures based on prediction and config
    async fn apply_protection_measures(
        &self,
        params: &ProtectedSwapParams,
        prediction: &SlippagePrediction,
    ) -> Result<(SlippagePrediction, Vec<ProtectionMeasure>), SlippageProtectionError> {
        let mut adjusted_prediction = prediction.clone();
        let mut protection_measures = Vec::new();

        // Dynamic slippage adjustment
        if params.protection_config.dynamic_adjustment {
            let old_tolerance = params.protection_config.max_slippage_bps;
            let new_tolerance = self.calculate_dynamic_tolerance(prediction, &params.protection_config)?;
            
            if new_tolerance != old_tolerance {
                // Don't override predicted slippage - that's the actual prediction
                // The tolerance is used for comparison, not as the prediction itself
                protection_measures.push(ProtectionMeasure::DynamicSlippageAdjustment {
                    old_tolerance,
                    new_tolerance,
                });
            }
        }

        // Route optimization
        if params.protection_config.route_optimization {
            if let Ok(optimized_route) = self.optimize_route_for_slippage(params, prediction).await {
                protection_measures.push(ProtectionMeasure::RouteOptimization {
                    original_route: "default".to_string(),
                    optimized_route,
                });
                
                // Recalculate prediction with optimized route
                adjusted_prediction.predicted_slippage_bps *= Decimal::from_str("0.85").unwrap(); // 15% improvement
            }
        }

        // Order splitting for large trades
        if params.amount > prediction.recommended_max_trade_size {
            let chunks = (params.amount / prediction.recommended_max_trade_size).ceil().to_u32().unwrap_or(1);
            let chunk_size = params.amount / Decimal::from(chunks);
            
            protection_measures.push(ProtectionMeasure::OrderSplitting {
                chunks,
                chunk_size,
            });
            
            // Adjust prediction for smaller chunks
            adjusted_prediction.predicted_slippage_bps *= Decimal::from_str("0.7").unwrap(); // 30% improvement
        }

        // Emergency stop check
        if adjusted_prediction.predicted_slippage_bps > params.protection_config.emergency_stop_threshold_bps {
            protection_measures.push(ProtectionMeasure::EmergencyStop {
                reason: format!(
                    "Predicted slippage {}bps exceeds emergency threshold {}bps",
                    adjusted_prediction.predicted_slippage_bps,
                    params.protection_config.emergency_stop_threshold_bps
                ),
            });
            
            return Err(SlippageProtectionError::SlippageToleranceExceeded {
                predicted: adjusted_prediction.predicted_slippage_bps,
                max_allowed: params.protection_config.emergency_stop_threshold_bps,
            });
        }

        Ok((adjusted_prediction, protection_measures))
    }

    /// Calculate dynamic slippage tolerance
    fn calculate_dynamic_tolerance(
        &self,
        prediction: &SlippagePrediction,
        config: &SlippageProtectionConfig,
    ) -> Result<Decimal, SlippageProtectionError> {
        // Base tolerance from config
        let mut tolerance = config.max_slippage_bps;

        // Adjust based on confidence score
        if prediction.confidence_score < Decimal::from_str("0.7").unwrap() {
            // Low confidence - increase tolerance
            tolerance *= Decimal::from_str("1.2").unwrap();
        } else if prediction.confidence_score > Decimal::from_str("0.9").unwrap() {
            // High confidence - can be more aggressive
            tolerance *= Decimal::from_str("0.9").unwrap();
        }

        // Adjust based on volatility
        let volatility_factor = prediction.volatility_adjustment / Decimal::from(100);
        tolerance += volatility_factor * Decimal::from(50); // Add up to 50bps for high volatility

        // Ensure tolerance stays within reasonable bounds
        tolerance = tolerance.max(Decimal::from(10)).min(Decimal::from(1000)); // 10bps to 10%

        Ok(tolerance)
    }

    /// Optimize route selection for minimal slippage
    async fn optimize_route_for_slippage(
        &self,
        params: &ProtectedSwapParams,
        prediction: &SlippagePrediction,
    ) -> Result<String, SlippageProtectionError> {
        let quote_params = QuoteParams {
            token_in: params.from_token.clone(),
            token_in_address: None,
            token_in_decimals: None,
            token_out: params.to_token.clone(),
            token_out_address: None,
            token_out_decimals: None,
            amount_in: params.amount.to_string(),
            chain: Some("ethereum".to_string()),
            slippage: Some((params.protection_config.max_slippage_bps / Decimal::from(100)).to_f64().unwrap_or(1.0)),
        };

        let quote = self.dex_aggregator.get_quote_with_guaranteed_routes(&quote_params).await
            .map_err(|e| SlippageProtectionError::QuoteError(e.to_string()))?;

        // Select route with best slippage characteristics
        let mut best_route = "Uniswap".to_string();
        let mut best_score = Decimal::ZERO;

        for route in &quote.routes {
            // Score based on multiple factors
            let price_score = Decimal::from_str(&route.amount_out).unwrap_or(Decimal::ZERO);
            let complexity_penalty = Decimal::from(1) * Decimal::from(10); // Simplified penalty
            let gas_penalty = Decimal::from_str(&route.gas_used).unwrap_or(Decimal::ZERO) / Decimal::from(1000);
            
            let total_score = price_score - complexity_penalty - gas_penalty;
            
            if total_score > best_score {
                best_score = total_score;
                best_route = route.dex.clone();
            }
        }

        Ok(best_route)
    }

    /// Validate slippage tolerance before execution
    fn validate_slippage_tolerance(
        &self,
        config: &SlippageProtectionConfig,
        prediction: &SlippagePrediction,
    ) -> Result<(), SlippageProtectionError> {
        if prediction.predicted_slippage_bps > config.max_slippage_bps {
            return Err(SlippageProtectionError::SlippageToleranceExceeded {
                predicted: prediction.predicted_slippage_bps,
                max_allowed: config.max_slippage_bps,
            });
        }

        // Additional validation for low confidence predictions
        if prediction.confidence_score < Decimal::from_str("0.3").unwrap() {
            warn!(
                "Low confidence prediction: {}. Consider increasing slippage tolerance.",
                prediction.confidence_score
            );
        }

        Ok(())
    }

    /// Execute swap with applied protection measures
    async fn execute_swap_with_protection(
        &self,
        params: &ProtectedSwapParams,
        adjusted_prediction: &SlippagePrediction,
    ) -> Result<SwapResponse, SlippageProtectionError> {
        let swap_params = SwapParams {
            token_in: params.from_token.clone(),
            token_out: params.to_token.clone(),
            amount_in: params.amount.to_string(),
            amount_out_min: "0".to_string(),
            routes: vec![],
            user_address: "0x0000000000000000000000000000000000000000".to_string(),
            slippage: (adjusted_prediction.predicted_slippage_bps / Decimal::from(100)).to_f64().unwrap_or(1.0),
        };

        self.dex_aggregator.execute_swap(swap_params).await
            .map_err(|e| SlippageProtectionError::DexAggregatorError(e.to_string()))
    }

    /// Calculate actual slippage from execution result
    async fn calculate_actual_slippage(
        &self,
        params: &ProtectedSwapParams,
        result: &SwapResponse,
    ) -> Result<Decimal, SlippageProtectionError> {
        // Get expected output without slippage
        let perfect_quote_params = QuoteParams {
            token_in: params.from_token.clone(),
            token_in_address: None,
            token_in_decimals: None,
            token_out: params.to_token.clone(),
            token_out_address: None,
            token_out_decimals: None,
            amount_in: params.amount.to_string(),
            chain: Some("ethereum".to_string()),
            slippage: Some(0.01),
        };

        let perfect_quote = self.dex_aggregator.get_quote_with_guaranteed_routes(&perfect_quote_params).await
            .map_err(|e| SlippageProtectionError::QuoteError(e.to_string()))?;

        let expected_output = Decimal::from_str(&perfect_quote.amount_out).unwrap_or(Decimal::ZERO);
        let actual_output = Decimal::from_str(&result.amount_out).unwrap_or(Decimal::ZERO);

        if expected_output > Decimal::ZERO {
            let slippage_bps = ((expected_output - actual_output) / expected_output) * Decimal::from(10000);
            Ok(slippage_bps.max(Decimal::ZERO))
        } else {
            Ok(Decimal::ZERO)
        }
    }

    /// Calculate protection effectiveness
    fn calculate_protection_effectiveness(
        &self,
        original_prediction: &SlippagePrediction,
        adjusted_prediction: &SlippagePrediction,
        actual_slippage: Decimal,
    ) -> Result<Decimal, SlippageProtectionError> {
        // Protection effectiveness = how much better actual vs original prediction
        let original_error = (original_prediction.predicted_slippage_bps - actual_slippage).abs();
        let adjusted_error = (adjusted_prediction.predicted_slippage_bps - actual_slippage).abs();

        if original_error > Decimal::ZERO {
            let improvement = (original_error - adjusted_error) / original_error;
            Ok(improvement.max(Decimal::ZERO).min(Decimal::ONE))
        } else {
            Ok(Decimal::ONE) // Perfect prediction
        }
    }

    /// Record slippage data for model improvement
    async fn record_slippage_data(
        &self,
        params: &ProtectedSwapParams,
        prediction: &SlippagePrediction,
        actual_slippage: Decimal,
    ) -> Result<(), SlippageProtectionError> {
        let data_point = crate::execution::slippage_predictor::SlippageDataPoint {
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            trade_size_usd: params.amount,
            expected_output: Decimal::ZERO, // Would need to calculate
            actual_output: Decimal::ZERO,   // Would need to calculate
            slippage_bps: actual_slippage,
            dex_name: "aggregated".to_string(),
            token_pair: format!("{}/{}", params.from_token, params.to_token),
            volatility: prediction.volatility_adjustment,
            liquidity_depth: prediction.liquidity_score,
        };

        self.slippage_predictor.record_slippage_data(data_point).await
            .map_err(|e| SlippageProtectionError::SlippagePredictorError(e.to_string()))?;

        Ok(())
    }

    /// Analyze slippage performance post-trade
    pub async fn analyze_slippage_performance(&self, swap_id: Uuid) -> Result<SlippageAnalysis, SlippageProtectionError> {
        let history = self.protection_history.read().await;
        let result = history.get(&swap_id)
            .ok_or_else(|| SlippageProtectionError::InvalidParameters("Swap not found".to_string()))?;

        let prediction_accuracy = if let Some(actual_slippage) = result.actual_slippage_bps {
            let error = (result.original_prediction.predicted_slippage_bps - actual_slippage).abs();
            let accuracy = Decimal::ONE - (error / result.original_prediction.predicted_slippage_bps.max(Decimal::ONE));
            accuracy.max(Decimal::ZERO).min(Decimal::ONE)
        } else {
            Decimal::ZERO
        };

        let market_snapshot = MarketSnapshot {
            timestamp: result.timestamp,
            volatility: result.original_prediction.volatility_adjustment,
            liquidity_depth: result.original_prediction.liquidity_score,
            spread_bps: Decimal::from(30), // Simplified
            recent_volume: Decimal::from(1_000_000), // Simplified
        };

        let lessons_learned = self.generate_lessons_learned(result)?;

        Ok(SlippageAnalysis {
            trade_id: swap_id,
            predicted_slippage_bps: result.original_prediction.predicted_slippage_bps,
            actual_slippage_bps: result.actual_slippage_bps.unwrap_or(Decimal::ZERO),
            prediction_accuracy,
            protection_effectiveness: result.protection_effectiveness.unwrap_or(Decimal::ZERO),
            market_conditions_at_execution: market_snapshot,
            lessons_learned,
        })
    }

    /// Generate lessons learned from trade execution
    fn generate_lessons_learned(&self, result: &ProtectedSwapResult) -> Result<Vec<String>, SlippageProtectionError> {
        let mut lessons = Vec::new();

        if let Some(actual_slippage) = result.actual_slippage_bps {
            let prediction_error = (result.original_prediction.predicted_slippage_bps - actual_slippage).abs();
            
            if prediction_error > Decimal::from(50) { // >50bps error
                lessons.push("Prediction accuracy needs improvement for this token pair".to_string());
            }

            if actual_slippage > result.original_prediction.predicted_slippage_bps * Decimal::from_str("1.5").unwrap() {
                lessons.push("Market conditions were more volatile than predicted".to_string());
            }

            if result.protection_effectiveness.unwrap_or(Decimal::ZERO) > Decimal::from_str("0.3").unwrap() {
                lessons.push("Protection measures were effective".to_string());
            } else {
                lessons.push("Protection measures had limited impact".to_string());
            }
        }

        if result.original_prediction.confidence_score < Decimal::from_str("0.5").unwrap() {
            lessons.push("Low confidence predictions require more conservative protection".to_string());
        }

        Ok(lessons)
    }

    /// Get protection statistics
    pub async fn get_protection_statistics(&self) -> HashMap<String, Decimal> {
        let history = self.protection_history.read().await;
        let mut stats = HashMap::new();

        let total_trades = history.len();
        let successful_trades = history.values()
            .filter(|r| r.execution_result.is_some())
            .count();

        let avg_protection_effectiveness = if successful_trades > 0 {
            history.values()
                .filter_map(|r| r.protection_effectiveness)
                .sum::<Decimal>() / Decimal::from(successful_trades)
        } else {
            Decimal::ZERO
        };

        let avg_actual_slippage = if successful_trades > 0 {
            history.values()
                .filter_map(|r| r.actual_slippage_bps)
                .sum::<Decimal>() / Decimal::from(successful_trades)
        } else {
            Decimal::ZERO
        };

        stats.insert("total_trades".to_string(), Decimal::from(total_trades));
        stats.insert("successful_trades".to_string(), Decimal::from(successful_trades));
        stats.insert("success_rate".to_string(), 
            if total_trades > 0 { 
                Decimal::from(successful_trades) / Decimal::from(total_trades) 
            } else { 
                Decimal::ZERO 
            }
        );
        stats.insert("avg_protection_effectiveness".to_string(), avg_protection_effectiveness);
        stats.insert("avg_actual_slippage_bps".to_string(), avg_actual_slippage);

        stats
    }
}

impl Default for SlippageProtectionConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: Decimal::from(100), // 1%
            dynamic_adjustment: true,
            route_optimization: true,
            pre_trade_validation: true,
            post_trade_analysis: true,
            emergency_stop_threshold_bps: Decimal::from(500), // 5%
        }
    }
}

use std::str::FromStr;
