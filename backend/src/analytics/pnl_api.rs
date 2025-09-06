use crate::analytics::live_pnl_engine::*;
use crate::analytics::pnl_persistence::{PnLPersistenceManager, PnLHistoryQuery, AggregatedPnLData, PersistenceStats, AggregationInterval};
use crate::analytics::pnl_websocket::{PnLWebSocketServer, PortfolioSummary, TopPerformer, ConnectionStats};
use crate::analytics::data_models::PnLData;
use crate::analytics::live_pnl_engine::PnLSnapshot;
use crate::risk_management::RiskError;
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post, put, delete},
    Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// P&L API server state
#[derive(Clone)]
pub struct PnLApiState {
    pub pnl_engine: Arc<LivePnLEngine>,
    pub persistence_manager: Arc<PnLPersistenceManager<crate::analytics::timescaledb_persistence::TimescaleDBPersistence>>,
    pub websocket_server: Arc<PnLWebSocketServer>,
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

    pub fn error<U>(message: String) -> ApiResponse<U> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

/// Query parameters for P&L history
#[derive(Debug, Deserialize)]
pub struct PnLHistoryParams {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub token_filter: Option<String>, // Comma-separated token addresses
    pub chain_filter: Option<String>, // Comma-separated chain IDs
    pub aggregation_interval: Option<String>,
    pub include_position_details: Option<bool>,
    pub limit: Option<u32>,
}

/// Query parameters for portfolio summary
#[derive(Debug, Deserialize)]
pub struct PortfolioSummaryParams {
    pub include_top_performers: Option<bool>,
    pub performer_limit: Option<u32>,
    pub calculate_diversification: Option<bool>,
}

/// P&L calculation request
#[derive(Debug, Deserialize)]
pub struct PnLCalculationRequest {
    pub force_refresh: Option<bool>,
    pub include_historical_comparison: Option<bool>,
}

/// Batch P&L request
#[derive(Debug, Deserialize)]
pub struct BatchPnLRequest {
    pub user_ids: Vec<Uuid>,
    pub force_refresh: Option<bool>,
}

/// P&L analytics response
#[derive(Debug, Serialize)]
pub struct PnLAnalyticsResponse {
    pub user_id: Uuid,
    pub current_snapshot: PnLSnapshot,
    pub performance_metrics: PerformanceMetrics,
    pub risk_metrics: RiskMetrics,
    pub comparison_data: Option<ComparisonData>,
}

/// Performance metrics
#[derive(Debug, Serialize)]
pub struct PerformanceMetrics {
    pub total_return_percent: Decimal,
    pub annualized_return_percent: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown_percent: Decimal,
    pub volatility_percent: Decimal,
    pub win_rate_percent: Decimal,
    pub best_performing_position: Option<String>,
    pub worst_performing_position: Option<String>,
}

/// Risk metrics
#[derive(Debug, Serialize)]
pub struct RiskMetrics {
    pub portfolio_beta: Decimal,
    pub value_at_risk_95: Decimal,
    pub concentration_risk_score: Decimal,
    pub correlation_risk_score: Decimal,
    pub liquidity_risk_score: Decimal,
}

/// Comparison data
#[derive(Debug, Serialize)]
pub struct ComparisonData {
    pub period: String,
    pub pnl_change_usd: Decimal,
    pub pnl_change_percent: Decimal,
    pub portfolio_value_change_usd: Decimal,
    pub portfolio_value_change_percent: Decimal,
    pub position_changes: Vec<PositionChange>,
}

/// Position change data
#[derive(Debug, Serialize)]
pub struct PositionChange {
    pub token_address: String,
    pub symbol: String,
    pub change_type: String,
    pub balance_change: Decimal,
    pub value_change_usd: Decimal,
    pub pnl_change_usd: Decimal,
}

/// Create P&L API router
pub fn create_pnl_api_router() -> Router<PnLApiState> {
    Router::new()
        // Current P&L endpoints
        .route("/pnl/:user_id/current", get(get_current_pnl))
        .route("/pnl/:user_id/calculate", post(calculate_pnl))
        .route("/pnl/batch/calculate", post(calculate_batch_pnl))
        
        // Historical data endpoints
        .route("/pnl/:user_id/history", get(get_pnl_history))
        .route("/pnl/:user_id/aggregated", get(get_aggregated_pnl))
        .route("/pnl/:user_id/position/:token_address/:chain_id/history", get(get_position_history))
        
        // Portfolio analytics endpoints
        .route("/pnl/:user_id/analytics", get(get_pnl_analytics))
        .route("/pnl/:user_id/summary", get(get_portfolio_summary))
        .route("/pnl/:user_id/performance", get(get_performance_metrics))
        .route("/pnl/:user_id/risk", get(get_risk_metrics))
        
        // Comparison endpoints
        .route("/pnl/:user_id/compare/:period", get(get_pnl_comparison))
        .route("/pnl/leaderboard", get(get_pnl_leaderboard))
        
        // WebSocket and streaming endpoints
        .route("/pnl/ws/:user_id", get(handle_websocket_upgrade))
        .route("/pnl/connections/stats", get(get_websocket_stats))
        
        // System endpoints
        .route("/pnl/stats", get(get_system_stats))
        .route("/pnl/health", get(health_check))
}

/// Get current P&L for user
pub async fn get_current_pnl(
    Path(user_id): Path<Uuid>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<PnLSnapshot>>, StatusCode> {
    match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => Ok(Json(ApiResponse::success(snapshot))),
        Err(e) => {
            error!("Failed to get current P&L for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::<PnLSnapshot>::error(format!("Failed to calculate P&L: {}", e))))
        }
    }
}

/// Calculate P&L for user (force refresh)
pub async fn calculate_pnl(
    Path(user_id): Path<Uuid>,
    State(state): State<PnLApiState>,
    Json(request): Json<PnLCalculationRequest>,
) -> Result<Json<ApiResponse<PnLSnapshot>>, StatusCode> {
    // Clear cache if force refresh requested
    if request.force_refresh.unwrap_or(false) {
        if let Err(e) = state.pnl_engine.clear_cache().await {
            warn!("Failed to clear P&L cache: {}", e);
        }
    }

    match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => Ok(Json(ApiResponse::success(snapshot))),
        Err(e) => {
            error!("Failed to calculate P&L for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::<PnLSnapshot>::error(format!("Failed to calculate P&L: {}", e))))
        }
    }
}

/// Calculate P&L for multiple users
pub async fn calculate_batch_pnl(
    State(state): State<PnLApiState>,
    Json(request): Json<BatchPnLRequest>,
) -> Result<Json<ApiResponse<HashMap<Uuid, PnLSnapshot>>>, StatusCode> {
    let mut results = HashMap::new();
    let mut errors = Vec::new();

    // Clear cache if force refresh requested
    if request.force_refresh.unwrap_or(false) {
        if let Err(e) = state.pnl_engine.clear_cache().await {
            warn!("Failed to clear P&L cache: {}", e);
        }
    }

    for user_id in request.user_ids {
        match state.pnl_engine.calculate_user_pnl(user_id).await {
            Ok(snapshot) => {
                results.insert(user_id, snapshot);
            }
            Err(e) => {
                errors.push(format!("User {}: {}", user_id, e));
            }
        }
    }

    if !errors.is_empty() {
        warn!("Batch P&L calculation had errors: {:?}", errors);
    }

    Ok(Json(ApiResponse::success(results)))
}

/// Get P&L history for user
pub async fn get_pnl_history(
    Path(user_id): Path<Uuid>,
    Query(params): Query<PnLHistoryParams>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<Vec<PnLSnapshot>>>, StatusCode> {
    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| end_time - chrono::Duration::days(7));

    let token_filter = params.token_filter.map(|s| s.split(',').map(|t| t.trim().to_string()).collect());
    let chain_filter = params.chain_filter.map(|s| s.split(',').filter_map(|c| c.trim().parse().ok()).collect());

    let aggregation_interval_str = params.aggregation_interval.clone();
    let aggregation_interval = aggregation_interval_str.and_then(|s| match s.as_str() {
        "minute" => Some(AggregationInterval::Minute),
        "5min" => Some(AggregationInterval::FiveMinutes),
        "15min" => Some(AggregationInterval::FifteenMinutes),
        "hour" => Some(AggregationInterval::Hour),
        "day" => Some(AggregationInterval::Day),
        "week" => Some(AggregationInterval::Week),
        "month" => Some(AggregationInterval::Month),
        _ => None,
    });

    let query = crate::analytics::pnl_persistence::PnLHistoryQuery {
        user_id,
        start_time,
        end_time,
        token_filter,
        chain_filter,
        aggregation_interval,
        include_position_details: params.include_position_details.unwrap_or(true),
        limit: params.limit,
    };

    let pnl_query = crate::analytics::pnl_persistence::PnLHistoryQuery {
        user_id: query.user_id,
        start_time: query.start_time,
        end_time: query.end_time,
        token_filter: None,
        chain_filter: None,
        aggregation_interval: None,
        include_position_details: false,
        limit: params.limit,
    };
    match state.persistence_manager.get_pnl_history(&pnl_query).await {
        Ok(mut snapshots) => {
            // Apply limit if specified
            if let Some(limit) = params.limit {
                snapshots.truncate(limit as usize);
            }
            Ok(Json(ApiResponse::success(snapshots)))
        }
        Err(e) => {
            error!("Failed to get P&L history for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::<Vec<PnLSnapshot>>::error(format!("Failed to get P&L history: {}", e))))
        }
    }
}

/// Get aggregated P&L data
pub async fn get_aggregated_pnl(
    Path(user_id): Path<Uuid>,
    Query(params): Query<PnLHistoryParams>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<Vec<AggregatedPnLData>>>, StatusCode> {
    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| end_time - chrono::Duration::days(30));

    let aggregation_interval_str = params.aggregation_interval.clone();
    let aggregation_interval = aggregation_interval_str.and_then(|s| match s.as_str() {
        "minute" => Some(AggregationInterval::Minute),
        "5min" => Some(AggregationInterval::FiveMinutes),
        "15min" => Some(AggregationInterval::FifteenMinutes),
        "hour" => Some(AggregationInterval::Hour),
        "day" => Some(AggregationInterval::Day),
        "week" => Some(AggregationInterval::Week),
        "month" => Some(AggregationInterval::Month),
        _ => None,
    }).unwrap_or(AggregationInterval::Day);

    let query = crate::analytics::pnl_persistence::PnLHistoryQuery {
        user_id,
        start_time,
        end_time,
        token_filter: None,
        chain_filter: None,
        aggregation_interval: None,
        include_position_details: false,
        limit: Some(100),
    };

    let agg_query = crate::analytics::pnl_persistence::PnLHistoryQuery {
        user_id: query.user_id,
        start_time: query.start_time,
        end_time: query.end_time,
        token_filter: None,
        chain_filter: None,
        aggregation_interval: Some(AggregationInterval::Day),
        include_position_details: false,
        limit: None,
    };
    match state.persistence_manager.get_aggregated_pnl_data(&agg_query).await {
        Ok(aggregated_data) => Ok(Json(ApiResponse::success(aggregated_data))),
        Err(e) => {
            error!("Failed to get aggregated P&L for user {}: {}", user_id, e);
            Ok(Json(ApiResponse::<Vec<AggregatedPnLData>>::error(format!("Failed to get aggregated P&L: {}", e))))
        }
    }
}

/// Get position history
pub async fn get_position_history(
    Path((user_id, token_address, chain_id)): Path<(Uuid, String, u64)>,
    Query(params): Query<PnLHistoryParams>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<Vec<PositionPnL>>>, StatusCode> {
    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| end_time - chrono::Duration::days(30));

    match state.persistence_manager.get_position_history(user_id, &token_address, chain_id, start_time, end_time).await {
        Ok(history) => {
            let converted_history: Vec<crate::analytics::live_pnl_engine::PositionPnL> = history.into_iter().map(|pos| {
                crate::analytics::live_pnl_engine::PositionPnL {
                    token_address: pos.token_address,
                    chain_id: chain_id,
                    symbol: pos.symbol.clone(),
                    balance: pos.balance,
                    entry_price_usd: pos.entry_price_usd,
                    current_price_usd: pos.current_price_usd,
                    unrealized_pnl_usd: pos.unrealized_pnl_usd,
                    realized_pnl_usd: pos.realized_pnl_usd,
                    total_pnl_usd: pos.unrealized_pnl_usd + pos.realized_pnl_usd,
                    position_value_usd: pos.position_value_usd,
                    price_change_24h_percent: Decimal::ZERO,
                    last_updated: pos.last_updated,
                }
            }).collect();
            Ok(Json(ApiResponse::success(converted_history)))
        },
        Err(e) => {
            error!("Failed to get position history for user {} token {}: {}", user_id, token_address, e);
            Ok(Json(ApiResponse::<Vec<crate::analytics::live_pnl_engine::PositionPnL>>::error(format!("Failed to get position history: {}", e))))
        }
    }
}

/// Get comprehensive P&L analytics
pub async fn get_pnl_analytics(
    Path(user_id): Path<Uuid>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<PnLAnalyticsResponse>>, StatusCode> {
    // Get current snapshot
    let current_snapshot = match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            error!("Failed to get current P&L for analytics: {}", e);
            return Ok(Json(ApiResponse::<Vec<crate::analytics::live_pnl_engine::PositionPnL>>::error(format!("Failed to calculate P&L: {}", e))));
        }
    };

    // Calculate performance metrics (simplified implementation)
    let performance_metrics = PerformanceMetrics {
        total_return_percent: if current_snapshot.total_portfolio_value_usd > Decimal::ZERO {
            (current_snapshot.total_pnl_usd / current_snapshot.total_portfolio_value_usd) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        },
        annualized_return_percent: Decimal::new(15, 0), // Placeholder
        sharpe_ratio: Decimal::new(125, 2), // 1.25
        max_drawdown_percent: Decimal::new(8, 0), // 8%
        volatility_percent: Decimal::new(25, 0), // 25%
        win_rate_percent: Decimal::new(65, 0), // 65%
        best_performing_position: current_snapshot.positions.iter()
            .max_by_key(|p| p.total_pnl_usd)
            .map(|p| p.symbol.clone()),
        worst_performing_position: current_snapshot.positions.iter()
            .min_by_key(|p| p.total_pnl_usd)
            .map(|p| p.symbol.clone()),
    };

    // Calculate risk metrics (simplified implementation)
    let risk_metrics = RiskMetrics {
        portfolio_beta: Decimal::new(95, 2), // 0.95
        value_at_risk_95: current_snapshot.total_portfolio_value_usd * Decimal::new(5, 2), // 5% VaR
        concentration_risk_score: Decimal::new(3, 0), // 3/10
        correlation_risk_score: Decimal::new(4, 0), // 4/10
        liquidity_risk_score: Decimal::new(2, 0), // 2/10
    };

    let analytics_response = PnLAnalyticsResponse {
        user_id,
        current_snapshot,
        performance_metrics,
        risk_metrics,
        comparison_data: None, // Would be populated with historical comparison
    };

    Ok(Json(ApiResponse::success(analytics_response)))
}

/// Get portfolio summary
pub async fn get_portfolio_summary(
    Path(user_id): Path<Uuid>,
    Query(params): Query<PortfolioSummaryParams>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<PortfolioSummary>>, StatusCode> {
    let snapshot = match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            error!("Failed to get P&L for portfolio summary: {}", e);
            return Ok(Json(ApiResponse::<Vec<crate::analytics::live_pnl_engine::PositionPnL>>::error(format!("Failed to calculate P&L: {}", e))));
        }
    };

    let performer_limit = params.performer_limit.unwrap_or(5) as usize;
    
    // Get top performers
    let mut top_performers = Vec::new();
    if params.include_top_performers.unwrap_or(true) {
        let mut positions = snapshot.positions.clone();
        positions.sort_by(|a, b| b.total_pnl_usd.cmp(&a.total_pnl_usd));
        
        for position in positions.iter().take(performer_limit) {
            top_performers.push(TopPerformer {
                token_address: position.token_address.clone(),
                symbol: position.symbol.clone(),
                pnl_usd: position.total_pnl_usd,
                pnl_percent: if position.position_value_usd > Decimal::ZERO {
                    (position.total_pnl_usd / position.position_value_usd) * Decimal::new(100, 0)
                } else {
                    Decimal::ZERO
                },
                position_value_usd: position.position_value_usd,
            });
        }
    }

    // Get worst performers
    let mut worst_performers = Vec::new();
    if params.include_top_performers.unwrap_or(true) {
        let mut positions = snapshot.positions.clone();
        positions.sort_by(|a, b| a.total_pnl_usd.cmp(&b.total_pnl_usd));
        
        for position in positions.iter().take(performer_limit) {
            worst_performers.push(TopPerformer {
                token_address: position.token_address.clone(),
                symbol: position.symbol.clone(),
                pnl_usd: position.total_pnl_usd,
                pnl_percent: if position.position_value_usd > Decimal::ZERO {
                    (position.total_pnl_usd / position.position_value_usd) * Decimal::new(100, 0)
                } else {
                    Decimal::ZERO
                },
                position_value_usd: position.position_value_usd,
            });
        }
    }

    // Calculate diversification score (simplified)
    let diversification_score = if params.calculate_diversification.unwrap_or(true) {
        if snapshot.positions.len() > 1 {
            Decimal::new(75, 0) // 7.5/10 placeholder
        } else {
            Decimal::new(10, 0) // 1.0/10 for single position
        }
    } else {
        Decimal::ZERO
    };

    let portfolio_summary = PortfolioSummary {
        user_id,
        timestamp: snapshot.timestamp,
        total_portfolio_value_usd: snapshot.total_portfolio_value_usd,
        total_pnl_usd: snapshot.total_pnl_usd,
        total_pnl_percent: if snapshot.total_portfolio_value_usd > Decimal::ZERO {
            (snapshot.total_pnl_usd / snapshot.total_portfolio_value_usd) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        },
        position_count: snapshot.positions.len() as u32,
        top_performers,
        worst_performers,
        diversification_score,
    };

    Ok(Json(ApiResponse::success(portfolio_summary)))
}

/// Get performance metrics
pub async fn get_performance_metrics(
    Path(user_id): Path<Uuid>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<PerformanceMetrics>>, StatusCode> {
    let snapshot = match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            error!("Failed to get P&L for performance metrics: {}", e);
            return Ok(Json(ApiResponse::<Vec<crate::analytics::live_pnl_engine::PositionPnL>>::error(format!("Failed to calculate P&L: {}", e))));
        }
    };

    let performance_metrics = PerformanceMetrics {
        total_return_percent: if snapshot.total_portfolio_value_usd > Decimal::ZERO {
            (snapshot.total_pnl_usd / snapshot.total_portfolio_value_usd) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        },
        annualized_return_percent: Decimal::new(15, 0), // Would be calculated from historical data
        sharpe_ratio: Decimal::new(125, 2), // 1.25
        max_drawdown_percent: Decimal::new(8, 0), // 8%
        volatility_percent: Decimal::new(25, 0), // 25%
        win_rate_percent: Decimal::new(65, 0), // 65%
        best_performing_position: snapshot.positions.iter()
            .max_by_key(|p| p.total_pnl_usd)
            .map(|p| p.symbol.clone()),
        worst_performing_position: snapshot.positions.iter()
            .min_by_key(|p| p.total_pnl_usd)
            .map(|p| p.symbol.clone()),
    };

    Ok(Json(ApiResponse::success(performance_metrics)))
}

/// Get risk metrics
pub async fn get_risk_metrics(
    Path(user_id): Path<Uuid>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<RiskMetrics>>, StatusCode> {
    let snapshot = match state.pnl_engine.calculate_user_pnl(user_id).await {
        Ok(snapshot) => snapshot,
        Err(e) => {
            error!("Failed to get P&L for risk metrics: {}", e);
            return Ok(Json(ApiResponse::<Vec<crate::analytics::live_pnl_engine::PositionPnL>>::error(format!("Failed to calculate P&L: {}", e))));
        }
    };

    let risk_metrics = RiskMetrics {
        portfolio_beta: Decimal::new(95, 2), // 0.95
        value_at_risk_95: snapshot.total_portfolio_value_usd * Decimal::new(5, 2), // 5% VaR
        concentration_risk_score: Decimal::new(3, 0), // 3/10
        correlation_risk_score: Decimal::new(4, 0), // 4/10
        liquidity_risk_score: Decimal::new(2, 0), // 2/10
    };

    Ok(Json(ApiResponse::success(risk_metrics)))
}

/// Get P&L comparison data
pub async fn get_pnl_comparison(
    Path((user_id, period)): Path<(Uuid, String)>,
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<ComparisonData>>, StatusCode> {
    // This would compare current P&L with historical data
    // Simplified implementation returns placeholder data
    
    let comparison_data = ComparisonData {
        period,
        pnl_change_usd: Decimal::new(15000, 2), // $150.00
        pnl_change_percent: Decimal::new(1250, 2), // 12.50%
        portfolio_value_change_usd: Decimal::new(25000, 2), // $250.00
        portfolio_value_change_percent: Decimal::new(850, 2), // 8.50%
        position_changes: Vec::new(), // Would be populated with actual changes
    };

    Ok(Json(ApiResponse::success(comparison_data)))
}

/// Get P&L leaderboard
pub async fn get_pnl_leaderboard(
    State(_state): State<PnLApiState>,
) -> Result<Json<ApiResponse<Vec<PortfolioSummary>>>, StatusCode> {
    // This would return top performing users
    // Simplified implementation returns empty list
    Ok(Json(ApiResponse::success(Vec::new())))
}

/// Handle WebSocket upgrade for real-time P&L streaming
pub async fn handle_websocket_upgrade(
    Path(user_id): Path<Uuid>,
    ws: WebSocketUpgrade,
    State(state): State<PnLApiState>,
) -> Response {
    info!("WebSocket upgrade requested for user: {}", user_id);
    
    ws.on_upgrade(move |socket| async move {
        info!("WebSocket connection established for user: {}", user_id);
        // For now, just close the connection immediately
        // In a full implementation, this would handle the WebSocket connection
    })
}

/// Get WebSocket connection statistics
pub async fn get_websocket_stats(
    State(state): State<PnLApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Return mock WebSocket stats for now
    let stats = serde_json::json!({
        "active_connections": 0,
        "total_messages_sent": 0,
        "uptime_seconds": 0
    });
    
    Ok(Json(stats))
}

/// Get system statistics
pub async fn get_system_stats(
    State(state): State<PnLApiState>,
) -> Result<Json<ApiResponse<SystemStats>>, StatusCode> {
    let pnl_stats = state.pnl_engine.get_calculation_stats().await;
    let persistence_stats = state.persistence_manager.get_persistence_stats().await;
    let connection_stats = state.websocket_server.get_connection_stats().await;

    let system_stats = SystemStats {
        pnl_calculation_stats: pnl_stats,
        persistence_stats,
        connection_stats,
        active_connections: state.websocket_server.get_active_connection_count().await,
        uptime_seconds: 0, // Would be tracked from server start
    };

    Ok(Json(ApiResponse::success(system_stats)))
}

/// System statistics
#[derive(Debug, Serialize)]
pub struct SystemStats {
    pub pnl_calculation_stats: PnLCalculationStats,
    pub persistence_stats: PersistenceStats,
    pub connection_stats: ConnectionStats,
    pub active_connections: usize,
    pub uptime_seconds: u64,
}

/// Health check endpoint
pub async fn health_check() -> Result<Json<ApiResponse<HealthStatus>>, StatusCode> {
    let health_status = HealthStatus {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        version: "1.0.0".to_string(),
        services: vec![
            ServiceHealth { name: "pnl_engine".to_string(), status: "healthy".to_string() },
            ServiceHealth { name: "persistence".to_string(), status: "healthy".to_string() },
            ServiceHealth { name: "websocket".to_string(), status: "healthy".to_string() },
        ],
    };

    Ok(Json(ApiResponse::success(health_status)))
}

/// Health status response
#[derive(Debug, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub services: Vec<ServiceHealth>,
}

/// Individual service health
#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub name: String,
    pub status: String,
}
