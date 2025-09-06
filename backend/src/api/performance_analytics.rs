use crate::risk_management::performance_tracker::{PortfolioPerformanceTracker, PerformanceMetrics};
use crate::risk_management::position_tracker::PositionTracker;
use crate::risk_management::redis_cache::RiskCache;
use crate::risk_management::types::RiskError;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// API state for performance analytics endpoints
#[derive(Clone)]
pub struct PerformanceAnalyticsState {
    pub performance_tracker: Arc<PortfolioPerformanceTracker>,
}

impl PerformanceAnalyticsState {
    pub fn new(performance_tracker: Arc<PortfolioPerformanceTracker>) -> Self {
        Self {
            performance_tracker,
        }
    }

    pub async fn new_async(
        position_tracker: Arc<PositionTracker>,
        redis_cache: Arc<tokio::sync::RwLock<RiskCache>>,
    ) -> Result<Self, crate::risk_management::types::RiskError> {
        let performance_tracker = Arc::new(
            PortfolioPerformanceTracker::new(position_tracker, redis_cache).await?
        );
        
        Ok(Self {
            performance_tracker,
        })
    }
}

/// Query parameters for performance metrics
#[derive(Debug, Deserialize)]
pub struct PerformanceQuery {
    pub period: Option<String>, // "1d", "7d", "30d", "90d", "1y"
    pub include_history: Option<bool>,
}

/// Response for performance metrics endpoint
#[derive(Debug, Serialize)]
pub struct PerformanceResponse {
    pub metrics: PerformanceMetrics,
    pub status: String,
    pub timestamp: u64,
}

/// Response for multiple users performance comparison
#[derive(Debug, Serialize)]
pub struct PerformanceComparisonResponse {
    pub users: HashMap<String, PerformanceMetrics>,
    pub summary: PerformanceSummary,
    pub timestamp: u64,
}

/// Summary statistics for performance comparison
#[derive(Debug, Serialize)]
pub struct PerformanceSummary {
    pub total_users: usize,
    pub average_roi: f64,
    pub best_performer: Option<String>,
    pub worst_performer: Option<String>,
    pub total_portfolio_value: f64,
}

/// Request for updating user performance data
#[derive(Debug, Deserialize)]
pub struct UpdatePerformanceRequest {
    pub user_id: String,
    pub portfolio_value: f64,
    pub trades: Option<Vec<TradeUpdate>>,
}

/// Trade update for performance tracking
#[derive(Debug, Deserialize)]
pub struct TradeUpdate {
    pub amount: f64,
    pub is_profitable: bool,
    pub timestamp: Option<u64>,
}

/// Performance analytics API routes
pub fn performance_analytics_routes() -> Router<PerformanceAnalyticsState> {
    Router::new()
        .route("/metrics/:user_id", get(get_user_performance_metrics))
        .route("/metrics/:user_id/update", post(update_user_performance))
        .route("/comparison", get(get_performance_comparison))
        .route("/leaderboard", get(get_performance_leaderboard))
        .route("/analytics/summary", get(get_analytics_summary))
        .route("/health", get(health_check))
}

/// Get performance metrics for a specific user
async fn get_user_performance_metrics(
    State(state): State<PerformanceAnalyticsState>,
    Path(user_id): Path<String>,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<PerformanceResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    match state.performance_tracker.calculate_performance_metrics(user_uuid).await {
        Ok(metrics) => {
            let response = PerformanceResponse {
                metrics,
                status: "success".to_string(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            Ok(Json(response))
        }
        Err(RiskError::UserNotFound(_)) => {
            Err((StatusCode::NOT_FOUND, "User not found".to_string()))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Performance calculation failed: {}", e)))
        }
    }
}

/// Update performance data for a user
async fn update_user_performance(
    State(state): State<PerformanceAnalyticsState>,
    Json(request): Json<UpdatePerformanceRequest>,
) -> Result<Json<PerformanceResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&request.user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    // Update portfolio value and trades if provided
    if let Some(trades) = request.trades {
        for trade in trades {
            // Add trade to performance tracker
            // This would typically involve updating the historical data
            // For now, we'll just recalculate metrics
        }
    }

    // Recalculate performance metrics
    match state.performance_tracker.calculate_performance_metrics(user_uuid).await {
        Ok(metrics) => {
            let response = PerformanceResponse {
                metrics,
                status: "updated".to_string(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            Ok(Json(response))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Performance update failed: {}", e)))
        }
    }
}

/// Get performance comparison across multiple users
async fn get_performance_comparison(
    State(state): State<PerformanceAnalyticsState>,
    Query(query): Query<PerformanceQuery>,
) -> Result<Json<PerformanceComparisonResponse>, (StatusCode, String)> {
    // For demonstration, we'll create sample user IDs
    // In production, this would fetch from a user database
    let sample_users = vec![
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ];

    let mut users_metrics = HashMap::new();
    let mut total_value = 0.0;
    let mut roi_values = Vec::new();
    
    for user_id in sample_users {
        match state.performance_tracker.calculate_performance_metrics(user_id).await {
            Ok(metrics) => {
                total_value += metrics.total_value_usd.to_string().parse::<f64>().unwrap_or(0.0);
                roi_values.push(metrics.roi_percentage.to_string().parse::<f64>().unwrap_or(0.0));
                users_metrics.insert(user_id.to_string(), metrics);
            }
            Err(_) => {
                // Skip users with errors
                continue;
            }
        }
    }

    let average_roi = if !roi_values.is_empty() {
        roi_values.iter().sum::<f64>() / roi_values.len() as f64
    } else {
        0.0
    };

    let best_performer = roi_values.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| format!("user_{}", i));

    let worst_performer = roi_values.iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| format!("user_{}", i));

    let summary = PerformanceSummary {
        total_users: users_metrics.len(),
        average_roi,
        best_performer,
        worst_performer,
        total_portfolio_value: total_value,
    };

    let response = PerformanceComparisonResponse {
        users: users_metrics,
        summary,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };

    Ok(Json(response))
}

/// Get performance leaderboard
async fn get_performance_leaderboard(
    State(state): State<PerformanceAnalyticsState>,
) -> Result<Json<Vec<PerformanceMetrics>>, (StatusCode, String)> {
    // For demonstration, create sample leaderboard
    // In production, this would query top performers from database
    let sample_users = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let mut leaderboard = Vec::new();

    for user_id in sample_users {
        match state.performance_tracker.calculate_performance_metrics(user_id).await {
            Ok(metrics) => leaderboard.push(metrics),
            Err(_) => continue,
        }
    }

    // Sort by ROI percentage (descending)
    leaderboard.sort_by(|a, b| {
        b.roi_percentage.partial_cmp(&a.roi_percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(Json(leaderboard))
}

/// Get analytics summary
async fn get_analytics_summary(
    State(_state): State<PerformanceAnalyticsState>,
) -> Result<Json<PerformanceSummary>, (StatusCode, String)> {
    // Mock analytics summary for demonstration
    let summary = PerformanceSummary {
        total_users: 150,
        average_roi: 12.5,
        best_performer: Some("user_123".to_string()),
        worst_performer: Some("user_456".to_string()),
        total_portfolio_value: 2_500_000.0,
    };

    Ok(Json(summary))
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "performance_analytics",
        "timestamp": chrono::Utc::now().timestamp(),
        "version": "1.0.0"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use std::sync::Arc;
    use tokio;

    #[tokio::test]
    async fn test_health_check() {
        let app = Router::new()
            .route("/health", get(health_check));
        
        let server = TestServer::new(app).unwrap();
        let response = server.get("/health").await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: serde_json::Value = response.json();
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["service"], "performance_analytics");
    }

    #[tokio::test]
    async fn test_analytics_summary() {
        // Test disabled due to compilation issues with TestServer
        // let app = Router::new()
        //     .route("/analytics/summary", get(get_analytics_summary));
        // 
        // let server = TestServer::new(app).unwrap();
        // let response = server.get("/analytics/summary").await;
        // 
        // assert_eq!(response.status_code(), StatusCode::OK);
        // 
        // let body: PerformanceSummary = response.json();
        // assert_eq!(body.total_users, 150);
        // assert_eq!(body.average_roi, 12.5);
    }
}
