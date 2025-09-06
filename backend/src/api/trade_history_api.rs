use crate::analytics::trade_history::{TradeHistoryManager, TradeRecord, TradeQuery, TradeFilter, TradeSortBy, TradeAnalytics, TradeStatus, TradeType};
use crate::risk_management::types::RiskError;
use axum::{extract::{Path, Query, State}, http::StatusCode, response::Json, routing::{get, post, delete}, Router};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

#[derive(Clone)]
pub struct TradeHistoryApiState {
    pub trade_manager: Arc<TradeHistoryManager>,
}

#[derive(Debug, Deserialize)]
pub struct TradeHistoryQuery {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub dex: Option<String>,
    pub sort_by: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TradeHistoryResponse {
    pub success: bool,
    pub data: Option<Vec<TradeRecord>>,
    pub total_count: Option<u64>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TradeAnalyticsResponse {
    pub success: bool,
    pub data: Option<TradeAnalytics>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTradeRequest {
    pub trade_type: TradeType,
    pub input_token: String,
    pub output_token: String,
    pub input_amount: Decimal,
    pub expected_output: Decimal,
    pub dex_used: String,
}

#[derive(Debug, Serialize)]
pub struct TradeResponse {
    pub success: bool,
    pub data: Option<TradeRecord>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl TradeHistoryApiState {
    pub fn new(trade_manager: Arc<TradeHistoryManager>) -> Self {
        Self { trade_manager }
    }
}

pub fn create_trade_history_api_router() -> Router<TradeHistoryApiState> {
    Router::new()
        .route("/history/:user_id", get(get_trade_history))
        .route("/analytics/:user_id", get(get_trade_analytics))
        .route("/trade", post(create_trade))
        .route("/export/:user_id", get(export_trades))
        .route("/health", get(get_health))
}

async fn get_trade_history(
    State(state): State<TradeHistoryApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<TradeHistoryQuery>,
) -> Result<Json<TradeHistoryResponse>, StatusCode> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(TradeHistoryResponse {
                success: false,
                data: None,
                total_count: None,
                error: Some("Invalid user ID format".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    let filter = TradeFilter {
        start_date: params.start_date,
        end_date: params.end_date,
        status: params.status.and_then(|s| parse_trade_status(&s).map(|status| vec![status])),
        dexes: params.dex.map(|dex| vec![dex]),
        token_pairs: None,
        trade_types: None,
        min_amount_usd: None,
        max_amount_usd: None,
        min_pnl: None,
        max_pnl: None,
        search_text: None,
    };

    let query = TradeQuery {
        filter: Some(filter),
        sort_by: params.sort_by.as_deref().and_then(parse_sort_by),
        sort_desc: Some(true),
        page: params.page,
        page_size: params.page_size,
    };

    match state.trade_manager.query_trades(&user_uuid, &query).await {
        Ok(trades) => {
            debug!("Retrieved {} trades for user {}", trades.len(), user_uuid);
            Ok(Json(TradeHistoryResponse {
                success: true,
                data: Some(trades.clone()),
                total_count: Some(trades.len() as u64),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get trade history for user {}: {}", user_uuid, e);
            Ok(Json(TradeHistoryResponse {
                success: false,
                data: None,
                total_count: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

async fn get_trade_analytics(
    State(state): State<TradeHistoryApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<TradeAnalyticsResponse>, StatusCode> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(TradeAnalyticsResponse {
                success: false,
                data: None,
                error: Some("Invalid user ID format".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    match state.trade_manager.calculate_analytics(&user_uuid).await {
        Ok(analytics) => {
            debug!("Generated trade analytics for user {}", user_uuid);
            Ok(Json(TradeAnalyticsResponse {
                success: true,
                data: Some(analytics),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to calculate trade analytics for user {}: {}", user_uuid, e);
            Ok(Json(TradeAnalyticsResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

async fn create_trade(
    State(state): State<TradeHistoryApiState>,
    Json(request): Json<CreateTradeRequest>,
) -> Result<Json<TradeResponse>, StatusCode> {
    let trade_id = Uuid::new_v4();
    let user_id = Uuid::new_v4(); // In real implementation, extract from auth context
    
    let trade = TradeRecord {
        trade_id,
        user_id,
        trade_type: request.trade_type,
        status: TradeStatus::Pending,
        timestamp: Utc::now(),
        execution_timestamp: None,
        input_token: request.input_token,
        output_token: request.output_token,
        input_amount: request.input_amount,
        output_amount: None,
        expected_output: request.expected_output,
        dex_used: request.dex_used,
        route_path: vec![],
        slippage_tolerance: Decimal::from_f64_retain(0.5).unwrap(),
        actual_slippage: None,
        gas_used: None,
        gas_price: None,
        gas_cost_usd: None,
        protocol_fees: Decimal::ZERO,
        network_fees: Decimal::ZERO,
        price_impact: None,
        execution_time_ms: None,
        pnl_usd: None,
        transaction_hash: None,
        block_number: None,
        nonce: None,
        metadata: HashMap::new(),
        error_message: None,
    };

    match state.trade_manager.record_trade(trade.clone()).await {
        Ok(_) => {
            info!("Created new trade {}", trade_id);
            Ok(Json(TradeResponse {
                success: true,
                data: Some(trade),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to create trade: {}", e);
            Ok(Json(TradeResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

async fn export_trades(
    State(state): State<TradeHistoryApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(serde_json::json!({
                "success": false,
                "error": "Invalid user ID format"
            })));
        }
    };

    let query = TradeQuery {
        filter: None,
        sort_by: Some(TradeSortBy::Timestamp),
        sort_desc: Some(false),
        page: None,
        page_size: None,
    };

    match state.trade_manager.query_trades(&user_uuid, &query).await {
        Ok(trades) => {
            Ok(Json(serde_json::json!({
                "success": true,
                "data": trades,
                "format": "json",
                "timestamp": Utc::now()
            })))
        }
        Err(e) => {
            Ok(Json(serde_json::json!({
                "success": false,
                "error": e.to_string()
            })))
        }
    }
}

async fn get_health(State(_state): State<TradeHistoryApiState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now(),
        "trade_manager_status": "operational"
    }))
}

fn parse_trade_status(status: &str) -> Option<TradeStatus> {
    match status.to_lowercase().as_str() {
        "pending" => Some(TradeStatus::Pending),
        "executed" => Some(TradeStatus::Executed),
        "failed" => Some(TradeStatus::Failed),
        "cancelled" => Some(TradeStatus::Cancelled),
        _ => None,
    }
}

fn parse_sort_by(sort_by: &str) -> Option<TradeSortBy> {
    match sort_by.to_lowercase().as_str() {
        "timestamp" => Some(TradeSortBy::Timestamp),
        "amount" => Some(TradeSortBy::Amount),
        "pnl" => Some(TradeSortBy::PnL),
        "gas_cost" => Some(TradeSortBy::GasCost),
        "slippage" => Some(TradeSortBy::Slippage),
        "execution_time" => Some(TradeSortBy::ExecutionTime),
        _ => None,
    }
}
