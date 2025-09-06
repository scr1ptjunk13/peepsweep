use crate::execution::{
    SlippagePredictor, OrderSplitter, SlippageProtectionEngine,
    SlippagePrediction, OrderSplitParams, SplittingStrategy, ProtectedSwapParams,
    SlippageProtectionConfig, SwapPriority, SlippageAnalysis
};
use crate::aggregator::DEXAggregator;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum AdvancedExecutionApiError {
    #[error("Invalid request parameters: {0}")]
    InvalidParameters(String),
    #[error("Slippage prediction failed: {0}")]
    SlippagePredictionError(String),
    #[error("Order splitting failed: {0}")]
    OrderSplittingError(String),
    #[error("Slippage protection failed: {0}")]
    SlippageProtectionError(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageEstimateRequest {
    pub from_token: String,
    pub to_token: String,
    pub amount: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageEstimateResponse {
    pub predicted_slippage_bps: String,
    pub confidence_score: String,
    pub market_impact_estimate: String,
    pub recommended_max_trade_size: String,
    pub volatility_adjustment: String,
    pub liquidity_score: String,
    pub prediction_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSwapRequest {
    pub from_token: String,
    pub to_token: String,
    pub amount: String,
    pub max_slippage_bps: String,
    pub protection_config: Option<AdvancedProtectionConfig>,
    pub user_id: Option<String>,
    pub priority: Option<String>, // "speed", "price", "protection", "balanced"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedProtectionConfig {
    pub dynamic_adjustment: Option<bool>,
    pub route_optimization: Option<bool>,
    pub pre_trade_validation: Option<bool>,
    pub post_trade_analysis: Option<bool>,
    pub emergency_stop_threshold_bps: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSwapResponse {
    pub swap_id: String,
    pub original_prediction: SlippageEstimateResponse,
    pub adjusted_prediction: SlippageEstimateResponse,
    pub protection_applied: Vec<String>,
    pub execution_result: Option<ExecutionResult>,
    pub actual_slippage_bps: Option<String>,
    pub protection_effectiveness: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub amount_out: String,
    pub gas_estimate: String,
    pub transaction_hash: String,
    pub route_breakdown: Vec<RouteStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub dex: String,
    pub percentage: String,
    pub amount_in: String,
    pub amount_out: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwapOrderRequest {
    pub from_token: String,
    pub to_token: String,
    pub total_amount: String,
    pub intervals: u32,
    pub time_window_seconds: u64,
    pub max_slippage_bps: String,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwapOrderResponse {
    pub order_id: String,
    pub chunks: Vec<OrderChunkInfo>,
    pub total_amount: String,
    pub estimated_completion_time: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChunkInfo {
    pub chunk_id: String,
    pub amount: String,
    pub execution_time: u64,
    pub target_dexs: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageAnalysisResponse {
    pub trade_id: String,
    pub predicted_slippage_bps: String,
    pub actual_slippage_bps: String,
    pub prediction_accuracy: String,
    pub protection_effectiveness: String,
    pub market_conditions: MarketConditionsInfo,
    pub lessons_learned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketConditionsInfo {
    pub timestamp: u64,
    pub volatility: String,
    pub liquidity_depth: String,
    pub spread_bps: String,
    pub recent_volume: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<String>,
}

#[derive(Clone)]
pub struct AdvancedExecutionApiState {
    dex_aggregator: Arc<DEXAggregator>,
    pub slippage_predictor: Arc<SlippagePredictor>,
    pub order_splitter: Arc<OrderSplitter>,
    pub protection_engine: Arc<SlippageProtectionEngine>,
}

impl AdvancedExecutionApiState {
    pub fn new(dex_aggregator: Arc<DEXAggregator>) -> Self {
        let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
        let order_splitter = Arc::new(OrderSplitter::new(
            dex_aggregator.clone(),
            slippage_predictor.clone(),
        ));
        let protection_engine = Arc::new(SlippageProtectionEngine::new(
            dex_aggregator.clone(),
            slippage_predictor.clone(),
        ));

        Self {
            dex_aggregator,
            slippage_predictor,
            order_splitter,
            protection_engine,
        }
    }
}

pub fn create_advanced_execution_router() -> Router<AdvancedExecutionApiState> {
    Router::new()
        .route("/slippage-estimate", get(get_slippage_estimate))
        .route("/advanced-swap", post(execute_advanced_swap))
        .route("/twap-order", post(create_twap_order))
        .route("/twap-order/:order_id", get(get_twap_order_status))
        .route("/slippage-analysis/:swap_id", get(get_slippage_analysis))
        .route("/protection-stats", get(get_protection_statistics))
}

/// GET /api/execution/slippage-estimate - Pre-trade slippage estimation
pub async fn get_slippage_estimate(
    State(state): State<AdvancedExecutionApiState>,
    Query(params): Query<SlippageEstimateRequest>,
) -> Result<Json<SlippageEstimateResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Slippage estimate request: {} {} -> {}", params.amount, params.from_token, params.to_token);

    let amount = Decimal::from_str(&params.amount)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid amount".to_string(),
                code: "INVALID_AMOUNT".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let prediction = state.slippage_predictor
        .predict_slippage(&params.from_token, &params.to_token, amount)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Slippage prediction failed".to_string(),
                code: "PREDICTION_ERROR".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let response = SlippageEstimateResponse {
        predicted_slippage_bps: prediction.predicted_slippage_bps.to_string(),
        confidence_score: prediction.confidence_score.to_string(),
        market_impact_estimate: prediction.market_impact_estimate.to_string(),
        recommended_max_trade_size: prediction.recommended_max_trade_size.to_string(),
        volatility_adjustment: prediction.volatility_adjustment.to_string(),
        liquidity_score: prediction.liquidity_score.to_string(),
        prediction_timestamp: prediction.prediction_timestamp,
    };

    Ok(Json(response))
}

/// POST /api/execution/advanced-swap - Advanced swap with slippage controls
pub async fn execute_advanced_swap(
    State(state): State<AdvancedExecutionApiState>,
    Json(request): Json<AdvancedSwapRequest>,
) -> Result<Json<AdvancedSwapResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Advanced swap request: {} {} -> {}", request.amount, request.from_token, request.to_token);

    let amount = Decimal::from_str(&request.amount)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid amount".to_string(),
                code: "INVALID_AMOUNT".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let max_slippage_bps = Decimal::from_str(&request.max_slippage_bps)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid slippage tolerance".to_string(),
                code: "INVALID_SLIPPAGE".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    // Convert request to internal types
    let protection_config = convert_protection_config(request.protection_config, max_slippage_bps)?;
    let priority = convert_swap_priority(request.priority)?;
    let user_id = request.user_id.and_then(|id| Uuid::parse_str(&id).ok());

    let protected_swap_params = ProtectedSwapParams {
        from_token: request.from_token,
        to_token: request.to_token,
        amount,
        protection_config,
        user_id,
        priority,
    };

    // Execute protected swap
    let result = state.protection_engine
        .execute_protected_swap(protected_swap_params)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Protected swap execution failed".to_string(),
                code: "EXECUTION_ERROR".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    // Convert result to response format
    let response = AdvancedSwapResponse {
        swap_id: result.swap_id.to_string(),
        original_prediction: convert_prediction_to_response(&result.original_prediction),
        adjusted_prediction: convert_prediction_to_response(&result.adjusted_prediction),
        protection_applied: result.protection_applied.iter().map(|p| format!("{:?}", p)).collect(),
        execution_result: result.execution_result.map(convert_swap_response_to_execution_result),
        actual_slippage_bps: result.actual_slippage_bps.map(|s| s.to_string()),
        protection_effectiveness: result.protection_effectiveness.map(|e| e.to_string()),
        timestamp: result.timestamp,
    };

    Ok(Json(response))
}

/// POST /api/execution/twap-order - TWAP execution
pub async fn create_twap_order(
    State(state): State<AdvancedExecutionApiState>,
    Json(request): Json<TwapOrderRequest>,
) -> Result<Json<TwapOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("TWAP order request: {} {} -> {} over {} intervals", 
          request.total_amount, request.from_token, request.to_token, request.intervals);

    let total_amount = Decimal::from_str(&request.total_amount)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid total amount".to_string(),
                code: "INVALID_AMOUNT".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let max_slippage_bps = Decimal::from_str(&request.max_slippage_bps)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid slippage tolerance".to_string(),
                code: "INVALID_SLIPPAGE".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let split_params = OrderSplitParams {
        from_token: request.from_token,
        to_token: request.to_token,
        total_amount,
        strategy: SplittingStrategy::TWAP { intervals: request.intervals },
        max_slippage_bps,
        time_window_seconds: request.time_window_seconds,
        min_chunk_size: None,
        max_chunks: Some(request.intervals),
    };

    let execution = state.order_splitter
        .split_order(split_params)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Order splitting failed".to_string(),
                code: "SPLITTING_ERROR".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let chunks: Vec<OrderChunkInfo> = execution.chunks.iter().map(|chunk| {
        OrderChunkInfo {
            chunk_id: chunk.chunk_id.to_string(),
            amount: chunk.amount.to_string(),
            execution_time: chunk.execution_time,
            target_dexs: chunk.target_dexs.clone(),
            status: format!("{:?}", chunk.status),
        }
    }).collect();

    let estimated_completion_time = execution.chunks.last()
        .map(|c| c.execution_time)
        .unwrap_or(0);

    let response = TwapOrderResponse {
        order_id: execution.order_id.to_string(),
        chunks,
        total_amount: total_amount.to_string(),
        estimated_completion_time,
        status: format!("{:?}", execution.status),
    };

    Ok(Json(response))
}

/// GET /api/execution/twap-order/:order_id - Get TWAP order status
pub async fn get_twap_order_status(
    State(state): State<AdvancedExecutionApiState>,
    Path(order_id): Path<String>,
) -> Result<Json<TwapOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let order_uuid = Uuid::parse_str(&order_id)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid order ID".to_string(),
                code: "INVALID_ORDER_ID".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let execution = state.order_splitter
        .get_order_status(order_uuid)
        .await
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Order not found".to_string(),
                code: "ORDER_NOT_FOUND".to_string(),
                details: None,
            })
        ))?;

    let chunks: Vec<OrderChunkInfo> = execution.chunks.iter().map(|chunk| {
        OrderChunkInfo {
            chunk_id: chunk.chunk_id.to_string(),
            amount: chunk.amount.to_string(),
            execution_time: chunk.execution_time,
            target_dexs: chunk.target_dexs.clone(),
            status: format!("{:?}", chunk.status),
        }
    }).collect();

    let estimated_completion_time = execution.chunks.last()
        .map(|c| c.execution_time)
        .unwrap_or(0);

    let response = TwapOrderResponse {
        order_id: execution.order_id.to_string(),
        chunks,
        total_amount: execution.total_executed.to_string(),
        estimated_completion_time,
        status: format!("{:?}", execution.status),
    };

    Ok(Json(response))
}

/// GET /api/execution/slippage-analysis/:swap_id - Post-trade analysis
pub async fn get_slippage_analysis(
    State(state): State<AdvancedExecutionApiState>,
    Path(swap_id): Path<String>,
) -> Result<Json<SlippageAnalysisResponse>, (StatusCode, Json<ErrorResponse>)> {
    let swap_uuid = Uuid::parse_str(&swap_id)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid swap ID".to_string(),
                code: "INVALID_SWAP_ID".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let analysis = state.protection_engine
        .analyze_slippage_performance(swap_uuid)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Analysis failed".to_string(),
                code: "ANALYSIS_ERROR".to_string(),
                details: Some(e.to_string()),
            })
        ))?;

    let response = SlippageAnalysisResponse {
        trade_id: analysis.trade_id.to_string(),
        predicted_slippage_bps: analysis.predicted_slippage_bps.to_string(),
        actual_slippage_bps: analysis.actual_slippage_bps.to_string(),
        prediction_accuracy: analysis.prediction_accuracy.to_string(),
        protection_effectiveness: analysis.protection_effectiveness.to_string(),
        market_conditions: MarketConditionsInfo {
            timestamp: analysis.market_conditions_at_execution.timestamp,
            volatility: analysis.market_conditions_at_execution.volatility.to_string(),
            liquidity_depth: analysis.market_conditions_at_execution.liquidity_depth.to_string(),
            spread_bps: analysis.market_conditions_at_execution.spread_bps.to_string(),
            recent_volume: analysis.market_conditions_at_execution.recent_volume.to_string(),
        },
        lessons_learned: analysis.lessons_learned,
    };

    Ok(Json(response))
}

/// GET /api/execution/protection-stats - Protection system statistics
pub async fn get_protection_statistics(
    State(state): State<AdvancedExecutionApiState>,
) -> Result<Json<HashMap<String, String>>, (StatusCode, Json<ErrorResponse>)> {
    let stats = state.protection_engine.get_protection_statistics().await;
    
    let string_stats: HashMap<String, String> = stats
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect();

    Ok(Json(string_stats))
}

// Helper functions for type conversion

fn convert_protection_config(
    config: Option<AdvancedProtectionConfig>,
    max_slippage_bps: Decimal,
) -> Result<SlippageProtectionConfig, (StatusCode, Json<ErrorResponse>)> {
    let default_config = SlippageProtectionConfig::default();
    
    if let Some(config) = config {
        let emergency_threshold = if let Some(threshold) = config.emergency_stop_threshold_bps {
            Decimal::from_str(&threshold).map_err(|e| (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid emergency stop threshold".to_string(),
                    code: "INVALID_THRESHOLD".to_string(),
                    details: Some(e.to_string()),
                })
            ))?
        } else {
            default_config.emergency_stop_threshold_bps
        };

        Ok(SlippageProtectionConfig {
            max_slippage_bps,
            dynamic_adjustment: config.dynamic_adjustment.unwrap_or(default_config.dynamic_adjustment),
            route_optimization: config.route_optimization.unwrap_or(default_config.route_optimization),
            pre_trade_validation: config.pre_trade_validation.unwrap_or(default_config.pre_trade_validation),
            post_trade_analysis: config.post_trade_analysis.unwrap_or(default_config.post_trade_analysis),
            emergency_stop_threshold_bps: emergency_threshold,
        })
    } else {
        Ok(SlippageProtectionConfig {
            max_slippage_bps,
            ..default_config
        })
    }
}

fn convert_swap_priority(priority: Option<String>) -> Result<SwapPriority, (StatusCode, Json<ErrorResponse>)> {
    match priority.as_deref() {
        Some("speed") => Ok(SwapPriority::Speed),
        Some("price") => Ok(SwapPriority::Price),
        Some("protection") => Ok(SwapPriority::Protection),
        Some("balanced") | None => Ok(SwapPriority::Balanced),
        Some(invalid) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid priority: {}", invalid),
                code: "INVALID_PRIORITY".to_string(),
                details: Some("Valid priorities: speed, price, protection, balanced".to_string()),
            })
        )),
    }
}

fn convert_prediction_to_response(prediction: &SlippagePrediction) -> SlippageEstimateResponse {
    SlippageEstimateResponse {
        predicted_slippage_bps: prediction.predicted_slippage_bps.to_string(),
        confidence_score: prediction.confidence_score.to_string(),
        market_impact_estimate: prediction.market_impact_estimate.to_string(),
        recommended_max_trade_size: prediction.recommended_max_trade_size.to_string(),
        volatility_adjustment: prediction.volatility_adjustment.to_string(),
        liquidity_score: prediction.liquidity_score.to_string(),
        prediction_timestamp: prediction.prediction_timestamp,
    }
}

fn convert_swap_response_to_execution_result(swap_response: crate::types::SwapResponse) -> ExecutionResult {
    ExecutionResult {
        amount_out: swap_response.amount_out,
        gas_estimate: swap_response.gas_used.clone(),
        transaction_hash: swap_response.tx_hash.clone(),
        route_breakdown: vec![], // Simplified - would need actual route breakdown
    }
}

use std::str::FromStr;
