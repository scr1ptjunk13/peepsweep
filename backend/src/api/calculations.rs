use crate::{
    ApiError, ApiResult, CalculationError, DatabaseError,
    calculations::{impermanent_loss, CalculationEngine},
    database::{models::UserPositionSummary, queries},
};
use crate::api::AppState;
use rust_decimal::prelude::ToPrimitive;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use futures::TryFutureExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    cache::CacheManager,
    database::models::IlSnapshot,
};

#[derive(Debug, Serialize)]
pub struct ILResponse {
    pub position_id: i64,
    pub current_value_usd: Decimal,
    pub hodl_value_usd: Decimal,
    pub il_percentage: Decimal,
    pub il_absolute_usd: Decimal,
    pub fees_earned_usd: Decimal,
    pub net_result_usd: Decimal,
    pub breakeven_price_change: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct FeesResponse {
    pub position_id: i64,
    pub fees_earned_token0: Decimal,
    pub fees_earned_token1: Decimal,
    pub fees_earned_usd: Decimal,
    pub fee_apr: Option<Decimal>,
    pub daily_fees_usd: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct BatchCalculationResponse {
    pub total_positions: usize,
    pub successful_calculations: usize,
    pub failed_calculations: usize,
    pub results: Vec<BatchCalculationResult>,
    pub summary: BatchSummary,
}

#[derive(Debug, Serialize)]
pub struct BatchCalculationResult {
    pub position_id: i64,
    pub success: bool,
    pub il_data: Option<ILResponse>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchSummary {
    pub total_il_usd: Decimal,
    pub total_fees_usd: Decimal,
    pub net_result_usd: Decimal,
    pub worst_il_percentage: Decimal,
    pub best_il_percentage: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct BatchCalculationRequest {
    pub user_address: String,
    pub position_ids: Option<Vec<i64>>,
    pub include_fees: Option<bool>,
    pub include_projections: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ILCalculationQuery {
    pub include_history: Option<bool>,
    pub days_back: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct FeesQuery {
    pub include_projections: Option<bool>,
    pub projection_days: Option<i64>,
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/il/:position_id", axum::routing::get(calculate_impermanent_loss))
        .route("/fees/:position_id", axum::routing::get(calculate_fees))
        .route("/batch", axum::routing::post(batch_calculate))
        .route("/user/:address/summary", axum::routing::get(calculate_user_summary))
        .route("/pool/:pool_address/metrics", axum::routing::get(calculate_pool_metrics))
}

pub async fn calculate_impermanent_loss(
    Path(position_id): Path<i64>,
    Query(params): Query<ILCalculationQuery>,
    State(state): State<AppState>,
) -> ApiResult<Json<ILResponse>> {
    // Get position data
    let position = queries::get_position_by_id(&state.db_pool, &position_id.to_string()).await
        .map_err(|e| ApiError::DatabaseError(e))?
        .ok_or_else(|| ApiError::NotFound("Position not found".to_string()))?;

    // Get current token prices
    let token0_price = state.pricing_engine.get_token_price(&position.token0_address().parse().unwrap()).await
        .map_err(|e| ApiError::CalculationError(e))?;
    
    let token1_price = state.pricing_engine.get_token_price(&position.token1_address().parse().unwrap()).await
        .map_err(|e| ApiError::CalculationError(e))?;

    // Calculate IL
    let il_result = impermanent_loss::calculate_impermanent_loss_v2(
        position.initial_token0_amount(),
        position.initial_token1_amount(),
        position.current_token0_amount(),
        position.current_token1_amount(),
        position.entry_price_token0().unwrap_or(Decimal::ZERO),
        position.entry_price_token1().unwrap_or(Decimal::ZERO),
        token0_price,
        token1_price,
    ).await?;

    // Calculate fees earned
    let fees_usd = position.fees_earned_usd.unwrap_or(Decimal::ZERO);

    let response = ILResponse {
        position_id,
        current_value_usd: Decimal::try_from(il_result.current_position_value_usd).unwrap_or_default(),
        hodl_value_usd: Decimal::try_from(il_result.hodl_value_usd).unwrap_or_default(),
        il_percentage: Decimal::try_from(il_result.il_percentage).unwrap_or_default(),
        il_absolute_usd: Decimal::try_from(il_result.il_usd_amount).unwrap_or_default(),
        fees_earned_usd: fees_usd,
        net_result_usd: fees_usd - Decimal::try_from(il_result.il_usd_amount).unwrap_or_default(),
        breakeven_price_change: None,
    };

    // Cache the result
    let snapshot = IlSnapshot {
        id: 0,
        user_address: position.user_address.clone(),
        position_id: position_id.to_string(),
        version: position.version.clone(),
        il_percentage: Decimal::try_from(il_result.il_percentage).unwrap_or_default(),
        hodl_value_usd: Decimal::try_from(il_result.hodl_value_usd).unwrap_or_default(),
        position_value_usd: Decimal::try_from(il_result.current_position_value_usd).unwrap_or_default(),
        fees_earned_usd: fees_usd,
        net_result_usd: fees_usd - Decimal::try_from(il_result.il_usd_amount).unwrap_or_default(),
        block_number: 0,
        timestamp: chrono::Utc::now(),
    };

    if let Err(e) = state.cache_manager.set_il_snapshot(position_id, &snapshot).await {
        tracing::warn!("Failed to cache IL snapshot: {}", e);
    }

    Ok(Json(response))
}

pub async fn calculate_fees(
    Path(position_id): Path<i64>,
    Query(params): Query<FeesQuery>,
    State(state): State<AppState>,
) -> ApiResult<Json<FeesResponse>> {
    // Get position data
    let position = queries::get_position_by_id(&state.db_pool, &position_id.to_string()).await
        .map_err(|e| ApiError::DatabaseError(e))?
        .ok_or_else(|| ApiError::NotFound("Position not found".to_string()))?;

    // Use placeholder token prices for now
    let token0_price = rust_decimal::Decimal::ZERO;
    let token1_price = rust_decimal::Decimal::ZERO;

    // Calculate fees using FeesCalculator
    let fees_calculator = crate::calculations::fees::FeesCalculator::new();
    let fees_result = match position.version.as_str() {
        "v2" => {
            // For V2, we need to create a PositionV2 from the position data
            let position_v2 = crate::database::models::PositionV2 {
                id: 0,
                user_address: position.user_address.clone(),
                pair_address: position.pool_address.clone(),
                token0: position.token0.clone(),
                token1: position.token1.clone(),
                liquidity: rust_decimal::Decimal::ZERO,
                token0_amount: rust_decimal::Decimal::ZERO,
                token1_amount: rust_decimal::Decimal::ZERO,
                block_number: 0,
                transaction_hash: "".to_string(),
                timestamp: chrono::Utc::now(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                current_il_percentage: None,
                fees_earned_usd: None,
            };
            fees_calculator.calculate_v2_fees(&position_v2, rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ONE)?
        },
        _ => {
            // For V3, create a PositionV3
            let position_v3 = crate::database::models::PositionV3 {
                id: 0,
                user_address: position.user_address.clone(),
                pool_address: position.pool_address.clone(),
                token_id: 0,
                token0: position.token0.clone(),
                token1: position.token1.clone(),
                fee_tier: 3000,
                tick_lower: 0,
                tick_upper: 0,
                liquidity: rust_decimal::Decimal::ZERO,
                token0_amount: Some(rust_decimal::Decimal::ZERO),
                token1_amount: Some(rust_decimal::Decimal::ZERO),
                fees_token0: rust_decimal::Decimal::ZERO,
                fees_token1: rust_decimal::Decimal::ZERO,
                block_number: 0,
                transaction_hash: "".to_string(),
                timestamp: chrono::Utc::now(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                current_tick: None,
                in_range: None,
                current_il_percentage: None,
                fees_earned_usd: None,
            };
            fees_calculator.calculate_v3_fees(&position_v3, rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO, rust_decimal::Decimal::ZERO)?
        }
    };

    let fees_earned_usd = fees_result.fees_earned_usd;

    // Calculate APR if requested
    let fee_apr = if params.include_projections.unwrap_or(false) {
        crate::calculations::fees::calculate_fee_apr(
            fees_earned_usd,
            rust_decimal::Decimal::ZERO, // position value placeholder
            30, // days elapsed placeholder
        ).ok()
    } else {
        None
    };

    // Calculate daily fees (placeholder calculation)
    let daily_fees_usd = Some(fees_earned_usd / rust_decimal::Decimal::from(30));

    let response = FeesResponse {
        position_id,
        fees_earned_token0: fees_result.fees_earned_token0,
        fees_earned_token1: fees_result.fees_earned_token1,
        fees_earned_usd,
        fee_apr,
        daily_fees_usd,
    };

    Ok(Json(response))
}

pub async fn batch_calculate(
    State(state): State<AppState>,
    Json(request): Json<BatchCalculationRequest>,
) -> ApiResult<Json<BatchCalculationResponse>> {
    // Get positions to calculate
    let positions = if let Some(position_ids) = request.position_ids {
        // Get specific positions
        let mut positions = Vec::new();
        for id in position_ids {
            if let Ok(Some(position)) = queries::get_position_by_id(&state.db_pool, &id.to_string()).await {
                positions.push(position);
            }
        }
        positions
    } else {
        // Get all user positions
        queries::get_user_positions(&state.db_pool, &request.user_address).await
            .map_err(ApiError::DatabaseError)?
    };

    let total_positions = positions.len();
    let mut successful_calculations = 0;
    let mut failed_calculations = 0;
    let mut results = Vec::new();
    let mut total_il_usd = Decimal::ZERO;
    let mut total_fees_usd = Decimal::ZERO;
    let mut worst_il = Decimal::ZERO;
    let mut best_il = Decimal::ZERO;

    // Process each position
    for position in positions {
        match calculate_position_il(&state, &position).await {
            Ok(il_response) => {
                successful_calculations += 1;
                total_il_usd += il_response.il_absolute_usd;
                total_fees_usd += il_response.fees_earned_usd;
                
                if il_response.il_percentage < worst_il {
                    worst_il = il_response.il_percentage;
                }
                if il_response.il_percentage > best_il {
                    best_il = il_response.il_percentage;
                }

                results = Vec::new();
                results.push(BatchCalculationResult {
                    position_id: position.id(),
                    success: true,
                    il_data: Some(il_response),
                    error: None,
                });
            }
            Err(e) => {
                failed_calculations += 1;
                results = Vec::new();
                results.push(BatchCalculationResult {
                    position_id: position.id(),
                    success: false,
                    il_data: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let response = BatchCalculationResponse {
        total_positions,
        successful_calculations,
        failed_calculations,
        results,
        summary: BatchSummary {
            total_il_usd,
            total_fees_usd,
            net_result_usd: total_fees_usd - total_il_usd,
            worst_il_percentage: worst_il,
            best_il_percentage: best_il,
        },
    };

    Ok(Json(response))
}

pub async fn calculate_user_summary(
    Path(user_address): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let positions = queries::get_user_positions(&state.db_pool, &user_address).await
        .map_err(ApiError::DatabaseError)?;

    let mut total_value_usd = Decimal::ZERO;
    let mut total_il_usd = Decimal::ZERO;
    let mut total_fees_usd = Decimal::ZERO;
    let mut active_positions = 0;
    for position in positions {
        if let Ok(il_response) = calculate_position_il(&state, &position).await {
            total_value_usd += il_response.current_value_usd;
            total_il_usd += il_response.il_absolute_usd;
            total_fees_usd += il_response.fees_earned_usd;
            active_positions += 1;
        }
    }

    let summary = serde_json::json!({
        "user_address": user_address,
        "active_positions": active_positions,
        "total_value_usd": total_value_usd,
        "total_il_usd": total_il_usd,
        "total_fees_usd": total_fees_usd,
        "net_result_usd": total_fees_usd - total_il_usd,
        "overall_il_percentage": if total_value_usd > Decimal::ZERO {
            (total_il_usd / total_value_usd) * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    });

    Ok(Json(summary))
}

pub async fn calculate_pool_metrics(
    Path(pool_address): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    // Get pool statistics
    let pool_stats = queries::get_pool_statistics(&state.db_pool, &pool_address).await
        .map_err(ApiError::DatabaseError)?;

    let metrics = serde_json::json!({
        "pool_address": pool_address,
        "total_positions": pool_stats.as_ref().and_then(|s| s.total_positions),
        "total_liquidity_usd": pool_stats.as_ref().and_then(|s| s.total_liquidity_usd),
        "average_il_percentage": pool_stats.as_ref().and_then(|s| s.average_il_percentage),
        "total_fees_earned_usd": pool_stats.as_ref().and_then(|s| s.total_fees_earned_usd),
        "active_positions": pool_stats.as_ref().and_then(|s| s.active_positions)
    });

    Ok(Json(metrics))
}

// Helper function to calculate IL for a single position
async fn calculate_position_il(
    state: &AppState,
    position: &UserPositionSummary,
) -> Result<ILResponse, ApiError> {
    // Get current token prices
    let token0_price = state.pricing_engine.get_token_price(&position.token0_address().parse().unwrap()).await
        .map_err(|e| ApiError::CalculationError(e))?;
    
    let token1_price = state.pricing_engine.get_token_price(&position.token1_address().parse().unwrap()).await
        .map_err(|e| ApiError::CalculationError(e))?;

    // Calculate IL
    let il_result = impermanent_loss::calculate_impermanent_loss_v2(
        position.initial_token0_amount(),
        position.initial_token1_amount(),
        position.current_token0_amount(),
        position.current_token1_amount(),
        position.entry_price_token0().unwrap_or(Decimal::ZERO),
        position.entry_price_token1().unwrap_or(Decimal::ZERO),
        token0_price,
        token1_price,
    ).await?;

    // Calculate fees earned
    let fees_usd = position.fees_earned_usd.unwrap_or(Decimal::ZERO);

    Ok(ILResponse {
        position_id: position.id(),
        current_value_usd: Decimal::try_from(il_result.current_position_value_usd).unwrap_or_default(),
        hodl_value_usd: Decimal::try_from(il_result.hodl_value_usd).unwrap_or_default(),
        il_percentage: Decimal::try_from(il_result.il_percentage).unwrap_or_default(),
        il_absolute_usd: Decimal::try_from(il_result.il_usd_amount).unwrap_or_default(),
        fees_earned_usd: fees_usd,
        net_result_usd: fees_usd - Decimal::try_from(il_result.il_usd_amount).unwrap_or_default(),
        breakeven_price_change: None,
    })
}
