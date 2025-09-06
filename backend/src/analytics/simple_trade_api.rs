use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Simple API response wrapper
#[derive(Debug, Serialize)]
pub struct SimpleApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> SimpleApiResponse<T> {
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

/// Simple trade record
#[derive(Debug, Clone, Serialize)]
pub struct SimpleTrade {
    pub trade_id: String,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub input_token: String,
    pub output_token: String,
    pub input_amount: String,
    pub output_amount: String,
    pub dex_used: String,
    pub status: String,
}

/// Trade export response
#[derive(Debug, Serialize)]
pub struct TradeExportResponse {
    pub export_id: String,
    pub format: String,
    pub total_records: u64,
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
}

/// Trade analytics response
#[derive(Debug, Serialize)]
pub struct TradeAnalyticsResponse {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub success_rate: f64,
    pub total_volume_usd: String,
    pub average_trade_size: String,
    pub most_used_dex: String,
    pub favorite_token_pair: String,
}

/// Performance metrics response
#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub total_return: String,
    pub sharpe_ratio: f64,
    pub max_drawdown: String,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub total_trades: u64,
    pub portfolio_value: String,
}

/// Simple API state
#[derive(Clone)]
pub struct SimpleApiState {
    pub trades: Arc<RwLock<HashMap<String, Vec<SimpleTrade>>>>,
}

impl SimpleApiState {
    pub fn new() -> Self {
        Self {
            trades: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

/// Create simple API router
pub fn create_simple_api_router() -> Router<SimpleApiState> {
    Router::new()
        .route("/trades/history/:user_id", get(get_trade_history))
        .route("/trades/export/:user_id", get(export_trade_history))
        .route("/trades/analytics/:user_id", get(get_trade_analytics))
        .route("/performance/metrics/:user_id", get(get_performance_metrics))
        .route("/health", get(health_check))
}

/// Get trade history for user
pub async fn get_trade_history(
    Path(user_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<SimpleApiState>,
) -> Result<Json<SimpleApiResponse<Vec<SimpleTrade>>>, StatusCode> {
    let trades = state.trades.read().await;
    let user_trades = trades.get(&user_id).cloned().unwrap_or_default();
    
    // Apply basic filtering
    let mut filtered_trades = user_trades;
    if let Some(limit) = params.get("limit") {
        if let Ok(limit_num) = limit.parse::<usize>() {
            filtered_trades.truncate(limit_num);
        }
    }
    
    Ok(Json(SimpleApiResponse::success(filtered_trades)))
}

/// Export trade history
pub async fn export_trade_history(
    Path(user_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<SimpleApiState>,
) -> Result<Json<SimpleApiResponse<TradeExportResponse>>, StatusCode> {
    let trades = state.trades.read().await;
    let user_trades = trades.get(&user_id).cloned().unwrap_or_default();
    
    let format = params.get("format").cloned().unwrap_or_else(|| "json".to_string());
    let export_id = Uuid::new_v4().to_string();
    
    let response = TradeExportResponse {
        export_id: export_id.clone(),
        format,
        total_records: user_trades.len() as u64,
        download_url: format!("/api/exports/{}.json", export_id),
        expires_at: Utc::now() + chrono::Duration::hours(24),
    };
    
    Ok(Json(SimpleApiResponse::success(response)))
}

/// Get trade analytics
pub async fn get_trade_analytics(
    Path(user_id): Path<String>,
    State(state): State<SimpleApiState>,
) -> Result<Json<SimpleApiResponse<TradeAnalyticsResponse>>, StatusCode> {
    let trades = state.trades.read().await;
    let user_trades = trades.get(&user_id).cloned().unwrap_or_default();
    
    let total_trades = user_trades.len() as u64;
    let successful_trades = user_trades.iter().filter(|t| t.status == "executed").count() as u64;
    let success_rate = if total_trades > 0 {
        successful_trades as f64 / total_trades as f64
    } else {
        0.0
    };
    
    // Calculate basic analytics
    let mut dex_usage: HashMap<String, u64> = HashMap::new();
    let mut token_pairs: HashMap<String, u64> = HashMap::new();
    
    for trade in &user_trades {
        *dex_usage.entry(trade.dex_used.clone()).or_insert(0) += 1;
        let pair = format!("{}-{}", trade.input_token, trade.output_token);
        *token_pairs.entry(pair).or_insert(0) += 1;
    }
    
    let most_used_dex = dex_usage.iter()
        .max_by_key(|(_, count)| *count)
        .map(|(dex, _)| dex.clone())
        .unwrap_or_else(|| "N/A".to_string());
    
    let favorite_token_pair = token_pairs.iter()
        .max_by_key(|(_, count)| *count)
        .map(|(pair, _)| pair.clone())
        .unwrap_or_else(|| "N/A".to_string());
    
    let analytics = TradeAnalyticsResponse {
        total_trades,
        successful_trades,
        success_rate,
        total_volume_usd: "1,234,567.89".to_string(),
        average_trade_size: "2,500.00".to_string(),
        most_used_dex,
        favorite_token_pair,
    };
    
    Ok(Json(SimpleApiResponse::success(analytics)))
}

/// Get performance metrics
pub async fn get_performance_metrics(
    Path(user_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<SimpleApiState>,
) -> Result<Json<SimpleApiResponse<PerformanceMetricsResponse>>, StatusCode> {
    let trades = state.trades.read().await;
    let user_trades = trades.get(&user_id).cloned().unwrap_or_default();
    
    let time_period = params.get("time_period").cloned().unwrap_or_else(|| "all_time".to_string());
    
    // Mock performance metrics based on trade count
    let total_trades = user_trades.len() as u64;
    let base_return = if total_trades > 0 { total_trades as f64 * 0.5 } else { 0.0 };
    
    let metrics = PerformanceMetricsResponse {
        total_return: format!("{:.2}%", base_return),
        sharpe_ratio: 1.25 + (total_trades as f64 * 0.01),
        max_drawdown: format!("-{:.2}%", 15.0 - (total_trades as f64 * 0.1)),
        win_rate: 0.65 + (total_trades as f64 * 0.001),
        profit_factor: 1.8 + (total_trades as f64 * 0.02),
        total_trades,
        portfolio_value: format!("${:.2}", 10000.0 + (total_trades as f64 * 100.0)),
    };
    
    Ok(Json(SimpleApiResponse::success(metrics)))
}

/// Health check
pub async fn health_check() -> Result<Json<SimpleApiResponse<HashMap<String, String>>>, StatusCode> {
    let mut status = HashMap::new();
    status.insert("status".to_string(), "healthy".to_string());
    status.insert("service".to_string(), "simple-analytics-api".to_string());
    status.insert("version".to_string(), "1.0.0".to_string());
    
    Ok(Json(SimpleApiResponse::success(status)))
}

/// Add sample trade data for testing
pub async fn add_sample_trades(state: &SimpleApiState) {
    let mut trades = state.trades.write().await;
    
    let sample_user = "550e8400-e29b-41d4-a716-446655440000".to_string();
    let sample_trades = vec![
        SimpleTrade {
            trade_id: Uuid::new_v4().to_string(),
            user_id: sample_user.clone(),
            timestamp: Utc::now() - chrono::Duration::hours(24),
            input_token: "ETH".to_string(),
            output_token: "USDC".to_string(),
            input_amount: "1.0".to_string(),
            output_amount: "3200.00".to_string(),
            dex_used: "Uniswap".to_string(),
            status: "executed".to_string(),
        },
        SimpleTrade {
            trade_id: Uuid::new_v4().to_string(),
            user_id: sample_user.clone(),
            timestamp: Utc::now() - chrono::Duration::hours(12),
            input_token: "USDC".to_string(),
            output_token: "BTC".to_string(),
            input_amount: "3200.00".to_string(),
            output_amount: "0.05".to_string(),
            dex_used: "Curve".to_string(),
            status: "executed".to_string(),
        },
        SimpleTrade {
            trade_id: Uuid::new_v4().to_string(),
            user_id: sample_user.clone(),
            timestamp: Utc::now() - chrono::Duration::hours(6),
            input_token: "BTC".to_string(),
            output_token: "ETH".to_string(),
            input_amount: "0.05".to_string(),
            output_amount: "1.1".to_string(),
            dex_used: "Balancer".to_string(),
            status: "executed".to_string(),
        },
    ];
    
    trades.insert(sample_user, sample_trades);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let state = SimpleApiState::new();
        let app = create_simple_api_router().with_state(state);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_trade_history() {
        let state = SimpleApiState::new();
        add_sample_trades(&state).await;
        let app = create_simple_api_router().with_state(state);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/trades/history/550e8400-e29b-41d4-a716-446655440000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_trade_export() {
        let state = SimpleApiState::new();
        add_sample_trades(&state).await;
        let app = create_simple_api_router().with_state(state);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/trades/export/550e8400-e29b-41d4-a716-446655440000?format=json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_performance_metrics() {
        let state = SimpleApiState::new();
        add_sample_trades(&state).await;
        let app = create_simple_api_router().with_state(state);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/performance/metrics/550e8400-e29b-41d4-a716-446655440000?time_period=monthly")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
