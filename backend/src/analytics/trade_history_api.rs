use crate::analytics::trade_history::{TradeHistoryManager, TradeRecord, TradeQuery, TradeFilter, TradeSortBy, TradeAnalytics};
use crate::risk_management::types::{RiskError, UserId};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Trade history API state
#[derive(Clone)]
pub struct TradeHistoryApiState {
    pub trade_manager: Arc<TradeHistoryManager>,
}

impl TradeHistoryApiState {
    pub fn new(trade_manager: Arc<TradeHistoryManager>) -> Self {
        Self { trade_manager }
    }
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Trade history query parameters
#[derive(Debug, Deserialize)]
pub struct TradeHistoryQueryParams {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub status: Option<String>,
    pub trade_type: Option<String>,
    pub dex: Option<String>,
    pub token_pair: Option<String>,
    pub min_amount: Option<Decimal>,
    pub max_amount: Option<Decimal>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

/// Export format options
#[derive(Debug, Deserialize)]
pub struct ExportQueryParams {
    pub format: Option<String>, // json, csv, xlsx
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub include_analytics: Option<bool>,
}

/// Export response
#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: String,
    pub format: String,
    pub total_records: u64,
    pub file_size_bytes: u64,
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
}

/// Trade history response with pagination
#[derive(Debug, Serialize)]
pub struct TradeHistoryResponse {
    pub trades: Vec<TradeRecord>,
    pub total_count: u64,
    pub page: u64,
    pub page_size: u64,
    pub has_next: bool,
    pub analytics: Option<TradeAnalytics>,
}

/// Create trade history API router
pub fn create_trade_history_router() -> Router<TradeHistoryApiState> {
    Router::new()
        .route("/trades/history/:user_id", get(get_trade_history))
        .route("/trades/analytics/:user_id", get(get_trade_analytics))
        .route("/trades/export/:user_id", get(export_trade_history))
        .route("/trades/:trade_id", get(get_trade_by_id))
        .route("/trades/search/:user_id", get(search_trades))
        .route("/health", get(health_check))
}

/// Get trade history with filtering and pagination
pub async fn get_trade_history(
    Path(user_id): Path<Uuid>,
    Query(params): Query<TradeHistoryQueryParams>,
    State(state): State<TradeHistoryApiState>,
) -> Result<Json<ApiResponse<TradeHistoryResponse>>, StatusCode> {
    debug!("Getting trade history for user: {}", user_id);

    // Build trade query from parameters
    let filter = if params.start_date.is_some() || params.end_date.is_some() || 
                     params.status.is_some() || params.trade_type.is_some() ||
                     params.dex.is_some() || params.token_pair.is_some() ||
                     params.min_amount.is_some() || params.max_amount.is_some() {
        Some(TradeFilter {
            start_date: params.start_date,
            end_date: params.end_date,
            status: params.status.map(|s| vec![match s.as_str() {
                "pending" => crate::analytics::trade_history::TradeStatus::Pending,
                "executed" => crate::analytics::trade_history::TradeStatus::Executed,
                "failed" => crate::analytics::trade_history::TradeStatus::Failed,
                "cancelled" => crate::analytics::trade_history::TradeStatus::Cancelled,
                _ => crate::analytics::trade_history::TradeStatus::PartiallyFilled,
            }]),
            trade_types: params.trade_type.map(|t| vec![match t.as_str() {
                "swap" => crate::analytics::trade_history::TradeType::Swap,
                "limit_order" => crate::analytics::trade_history::TradeType::LimitOrder,
                "market_order" => crate::analytics::trade_history::TradeType::MarketOrder,
                "bridge" => crate::analytics::trade_history::TradeType::Bridge,
                _ => crate::analytics::trade_history::TradeType::Arbitrage,
            }]),
            dexes: params.dex.map(|d| vec![d]),
            token_pairs: params.token_pair.map(|tp| {
                let parts: Vec<&str> = tp.split('-').collect();
                if parts.len() == 2 {
                    vec![(parts[0].to_string(), parts[1].to_string())]
                } else {
                    vec![]
                }
            }),
            min_amount_usd: params.min_amount,
            max_amount_usd: params.max_amount,
            min_pnl: None,
            max_pnl: None,
            search_text: None,
        })
    } else {
        None
    };

    let sort_by = params.sort_by.as_deref().map(|s| match s {
        "timestamp" => TradeSortBy::Timestamp,
        "amount" => TradeSortBy::Amount,
        "pnl" => TradeSortBy::PnL,
        _ => TradeSortBy::Timestamp,
    });

    let query = TradeQuery {
        filter,
        sort_by,
        sort_desc: params.sort_desc,
        page: params.page.map(|p| p as u32),
        page_size: params.page_size.map(|p| p as u32),
    };

    match state.trade_manager.query_trades(&user_id, &query).await {
        Ok(trades) => {
            let total_count = state.trade_manager.count_trades(&user_id, &query.filter).await.unwrap_or(0);
            let page = params.page.unwrap_or(0);
            let page_size = params.page_size.unwrap_or(50);
            let has_next = (page + 1) * page_size < total_count;

            // Get analytics if requested
            let analytics = if params.page.is_none() || params.page == Some(0) {
                state.trade_manager.calculate_analytics(&user_id).await.ok()
            } else {
                None
            };

            let response = TradeHistoryResponse {
                trades,
                total_count,
                page,
                page_size,
                has_next,
                analytics,
            };

            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to get trade history for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::error(format!("Failed to get trade history: {}", e))))
        }
    }
}

/// Get trade analytics for a user
pub async fn get_trade_analytics(
    Path(user_id): Path<Uuid>,
    State(state): State<TradeHistoryApiState>,
) -> Result<Json<ApiResponse<TradeAnalytics>>, StatusCode> {
    debug!("Getting trade analytics for user: {}", user_id);

    match state.trade_manager.calculate_analytics(&user_id).await {
        Ok(analytics) => Ok(Json(ApiResponse::success(analytics))),
        Err(e) => {
            error!("Failed to calculate analytics for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::error(format!("Failed to calculate analytics: {}", e))))
        }
    }
}

/// Export trade history in various formats
pub async fn export_trade_history(
    Path(user_id): Path<Uuid>,
    Query(params): Query<ExportQueryParams>,
    State(state): State<TradeHistoryApiState>,
) -> Result<Json<ApiResponse<ExportResponse>>, StatusCode> {
    debug!("Exporting trade history for user: {} in format: {:?}", user_id, params.format);

    let format = params.format.unwrap_or_else(|| "json".to_string());
    
    // Build query for export
    let filter = if params.start_date.is_some() || params.end_date.is_some() {
        Some(TradeFilter {
            start_date: params.start_date,
            end_date: params.end_date,
            status: None,
            trade_types: None,
            dexes: None,
            token_pairs: None,
            min_amount_usd: None,
            max_amount_usd: None,
            min_pnl: None,
            max_pnl: None,
            search_text: None,
        })
    } else {
        None
    };

    let query = TradeQuery {
        filter,
        sort_by: Some(TradeSortBy::Timestamp),
        sort_desc: Some(false),
        page: None,
        page_size: None,
    };

    match state.trade_manager.query_trades(&user_id, &query).await {
        Ok(trades) => {
            let export_id = Uuid::new_v4().to_string();
            let total_records = trades.len() as u64;
            
            // Generate export data based on format
            let (file_size, download_url) = match format.as_str() {
                "csv" => generate_csv_export(&trades, &export_id).await,
                "xlsx" => generate_xlsx_export(&trades, &export_id).await,
                _ => generate_json_export(&trades, &export_id, params.include_analytics.unwrap_or(false), &state, &user_id).await,
            };

            let response = ExportResponse {
                export_id,
                format,
                total_records,
                file_size_bytes: file_size,
                download_url,
                expires_at: Utc::now() + chrono::Duration::hours(24),
            };

            info!("Generated export for user {} with {} records", user_id, total_records);
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to export trade history for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::error(format!("Failed to export trade history: {}", e))))
        }
    }
}

/// Get specific trade by ID
pub async fn get_trade_by_id(
    Path(trade_id): Path<Uuid>,
    State(state): State<TradeHistoryApiState>,
) -> Result<Json<ApiResponse<TradeRecord>>, StatusCode> {
    debug!("Getting trade by ID: {}", trade_id);

    match state.trade_manager.get_trade(&trade_id).await {
        Ok(Some(trade)) => Ok(Json(ApiResponse::success(trade))),
        Ok(None) => Ok(Json(ApiResponse::error("Trade not found".to_string()))),
        Err(e) => {
            error!("Failed to get trade {}: {}", trade_id, e);
            Ok(Json(ApiResponse::error(format!("Failed to get trade: {}", e))))
        }
    }
}

/// Search trades by text query
pub async fn search_trades(
    Path(user_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<TradeHistoryApiState>,
) -> Result<Json<ApiResponse<Vec<TradeRecord>>>, StatusCode> {
    let search_query = params.get("q").cloned().unwrap_or_default();
    debug!("Searching trades for user {} with query: {}", user_id, search_query);

    match state.trade_manager.search_trades(&user_id, &search_query).await {
        Ok(trades) => Ok(Json(ApiResponse::success(trades))),
        Err(e) => {
            error!("Failed to search trades for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::error(format!("Failed to search trades: {}", e))))
        }
    }
}

/// Health check endpoint
pub async fn health_check() -> Result<Json<ApiResponse<HashMap<String, String>>>, StatusCode> {
    let mut status = HashMap::new();
    status.insert("status".to_string(), "healthy".to_string());
    status.insert("service".to_string(), "trade-history-api".to_string());
    status.insert("timestamp".to_string(), Utc::now().to_rfc3339());
    
    Ok(Json(ApiResponse::success(status)))
}

// Export generation functions
async fn generate_json_export(
    trades: &[TradeRecord], 
    export_id: &str,
    include_analytics: bool,
    state: &TradeHistoryApiState,
    user_id: &Uuid,
) -> (u64, String) {
    let mut export_data = serde_json::json!({
        "export_id": export_id,
        "generated_at": Utc::now(),
        "total_trades": trades.len(),
        "trades": trades
    });

    if include_analytics {
        if let Ok(analytics) = state.trade_manager.calculate_analytics(user_id).await {
            export_data["analytics"] = serde_json::to_value(analytics).unwrap_or_default();
        }
    }

    let json_string = serde_json::to_string_pretty(&export_data).unwrap_or_default();
    let file_size = json_string.len() as u64;
    let download_url = format!("/api/exports/{}.json", export_id);
    
    (file_size, download_url)
}

async fn generate_csv_export(trades: &[TradeRecord], export_id: &str) -> (u64, String) {
    let mut csv_content = String::from("trade_id,user_id,trade_type,status,timestamp,input_token,output_token,input_amount,output_amount,dex_used,slippage_tolerance,actual_slippage,gas_used,gas_cost_usd,fees_usd,pnl_usd\n");
    
    for trade in trades {
        csv_content.push_str(&format!(
            "{},{},{:?},{:?},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            trade.trade_id,
            trade.user_id,
            trade.trade_type,
            trade.status,
            trade.timestamp.to_rfc3339(),
            trade.input_token,
            trade.output_token,
            trade.input_amount,
            trade.output_amount.unwrap_or_default(),
            trade.dex_used,
            trade.slippage_tolerance,
            trade.actual_slippage.unwrap_or_default(),
            trade.gas_used.unwrap_or_default(),
            trade.gas_cost_usd.unwrap_or_default(),
            (trade.protocol_fees + trade.network_fees),
            trade.pnl_usd.unwrap_or_default()
        ));
    }
    
    let file_size = csv_content.len() as u64;
    let download_url = format!("/api/exports/{}.csv", export_id);
    
    (file_size, download_url)
}

async fn generate_xlsx_export(trades: &[TradeRecord], export_id: &str) -> (u64, String) {
    // Simplified XLSX export - in production would use a proper XLSX library
    let file_size = trades.len() as u64 * 200; // Estimated size
    let download_url = format!("/api/exports/{}.xlsx", export_id);
    
    (file_size, download_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::trade_history::{MockTradeDataStore, MockTradeSearchIndex, MockTradeDataValidator};

    #[tokio::test]
    async fn test_trade_history_api() {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        
        let trade_manager = Arc::new(TradeHistoryManager::new(data_store, search_index, validator));
        let state = TradeHistoryApiState::new(trade_manager);
        
        // Test health check
        let health_response = health_check().await.unwrap();
        assert!(health_response.0.success);
    }

    #[tokio::test]
    async fn test_export_formats() {
        let trades = vec![];
        let export_id = "test_export";
        
        // Test JSON export
        let (size, url) = generate_json_export(&trades, export_id, false, &create_test_state(), &Uuid::new_v4()).await;
        assert!(size > 0);
        assert!(url.contains("json"));
        
        // Test CSV export
        let (size, url) = generate_csv_export(&trades, export_id).await;
        assert!(size > 0);
        assert!(url.contains("csv"));
    }

    fn create_test_state() -> TradeHistoryApiState {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        let trade_manager = Arc::new(TradeHistoryManager::new(data_store, search_index, validator));
        TradeHistoryApiState::new(trade_manager)
    }
}
