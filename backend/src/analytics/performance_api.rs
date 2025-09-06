use crate::analytics::performance_metrics::{PerformanceMetricsCalculator, PerformanceMetrics, TimePeriod, PerformanceComparison, BenchmarkData};
use crate::analytics::performance_comparison::{PerformanceComparator, LeaderboardConfig, LeaderboardMetric, PerformanceCategory, AnonymizedPerformance};
use crate::analytics::benchmark_integration::{BenchmarkDataManager, BenchmarkConfig};
use crate::risk_management::types::RiskError;
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
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Performance API state
#[derive(Debug, Clone)]
pub struct PerformanceApiState {
    pub metrics_calculator: Arc<PerformanceMetricsCalculator>,
    pub performance_comparator: Arc<PerformanceComparator>,
    pub benchmark_manager: Arc<BenchmarkDataManager>,
}

/// Performance metrics request parameters
#[derive(Debug, Deserialize)]
pub struct PerformanceMetricsQuery {
    pub time_period: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

/// Benchmark comparison request
#[derive(Debug, Deserialize)]
pub struct BenchmarkComparisonRequest {
    pub benchmark_symbols: Vec<String>,
    pub time_period: Option<String>,
}

/// Leaderboard query parameters
#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub metric: Option<String>,
    pub category: Option<String>,
    pub min_trades: Option<u64>,
    pub min_portfolio_value: Option<f64>,
    pub limit: Option<usize>,
    pub time_period: Option<String>,
}

/// Performance metrics response
#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub success: bool,
    pub data: Option<PerformanceMetrics>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Benchmark comparison response
#[derive(Debug, Serialize)]
pub struct BenchmarkComparisonResponse {
    pub success: bool,
    pub data: Option<PerformanceComparison>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Leaderboard response
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub success: bool,
    pub data: Option<Vec<AnonymizedPerformance>>,
    pub total_users: Option<u64>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Benchmark list response
#[derive(Debug, Serialize)]
pub struct BenchmarkListResponse {
    pub success: bool,
    pub data: Option<Vec<BenchmarkConfig>>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Performance analytics summary
#[derive(Debug, Serialize)]
pub struct PerformanceAnalyticsSummary {
    pub total_users: u64,
    pub active_traders: u64,
    pub average_return: Decimal,
    pub median_return: Decimal,
    pub top_performer_return: Decimal,
    pub total_volume_traded: Decimal,
    pub total_fees_paid: Decimal,
    pub average_sharpe_ratio: Decimal,
    pub market_correlation: Decimal,
    pub risk_metrics: RiskMetricsSummary,
}

#[derive(Debug, Serialize)]
pub struct RiskMetricsSummary {
    pub average_max_drawdown: Decimal,
    pub average_volatility: Decimal,
    pub var_95_average: Decimal,
    pub high_risk_users: u64,
    pub conservative_users: u64,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub metrics_calculator_status: String,
    pub benchmark_manager_status: String,
    pub performance_comparator_status: String,
    pub cache_hit_rate: Option<f64>,
    pub active_users: u64,
}

impl PerformanceApiState {
    pub fn new(
        metrics_calculator: Arc<PerformanceMetricsCalculator>,
        performance_comparator: Arc<PerformanceComparator>,
        benchmark_manager: Arc<BenchmarkDataManager>,
    ) -> Self {
        Self {
            metrics_calculator,
            performance_comparator,
            benchmark_manager,
        }
    }
}

/// Create performance API router
pub fn create_performance_api_router() -> Router<PerformanceApiState> {
    Router::new()
        .route("/metrics/:user_id", get(get_user_performance_metrics))
        .route("/comparison/:user_id", post(compare_user_performance))
        .route("/leaderboard", get(get_performance_leaderboard))
        .route("/analytics/summary", get(get_performance_analytics_summary))
        .route("/benchmarks", get(get_available_benchmarks))
        .route("/benchmarks/:symbol/data", get(get_benchmark_data))
        .route("/health", get(get_performance_health))
}

/// Get performance metrics for a specific user
async fn get_user_performance_metrics(
    State(state): State<PerformanceApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<PerformanceMetricsQuery>,
) -> Result<Json<PerformanceMetricsResponse>, StatusCode> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(PerformanceMetricsResponse {
                success: false,
                data: None,
                error: Some("Invalid user ID format".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    // Parse time period
    let time_period = match params.time_period.as_deref() {
        Some("daily") => TimePeriod::Daily,
        Some("weekly") => TimePeriod::Weekly,
        Some("monthly") => TimePeriod::Monthly,
        Some("quarterly") => TimePeriod::Quarterly,
        Some("yearly") => TimePeriod::Yearly,
        Some("all_time") => TimePeriod::AllTime,
        Some("custom") => {
            if let (Some(start), Some(end)) = (params.start_date, params.end_date) {
                TimePeriod::Custom { start, end }
            } else {
                return Ok(Json(PerformanceMetricsResponse {
                    success: false,
                    data: None,
                    error: Some("Custom time period requires start_date and end_date".to_string()),
                    timestamp: Utc::now(),
                }));
            }
        }
        _ => TimePeriod::AllTime,
    };

    match state.metrics_calculator.calculate_performance_metrics(&user_uuid, time_period).await {
        Ok(metrics) => {
            debug!("Retrieved performance metrics for user {}", user_uuid);
            Ok(Json(PerformanceMetricsResponse {
                success: true,
                data: Some(metrics),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to get performance metrics for user {}: {}", user_uuid, e);
            Ok(Json(PerformanceMetricsResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Compare user performance against benchmarks
async fn compare_user_performance(
    State(state): State<PerformanceApiState>,
    Path(user_id): Path<String>,
    Json(request): Json<BenchmarkComparisonRequest>,
) -> Result<Json<BenchmarkComparisonResponse>, StatusCode> {
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(_) => {
            return Ok(Json(BenchmarkComparisonResponse {
                success: false,
                data: None,
                error: Some("Invalid user ID format".to_string()),
                timestamp: Utc::now(),
            }));
        }
    };

    // Parse time period
    let time_period = match request.time_period.as_deref() {
        Some("daily") => TimePeriod::Daily,
        Some("weekly") => TimePeriod::Weekly,
        Some("monthly") => TimePeriod::Monthly,
        Some("quarterly") => TimePeriod::Quarterly,
        Some("yearly") => TimePeriod::Yearly,
        Some("all_time") => TimePeriod::AllTime,
        _ => TimePeriod::AllTime,
    };

    match state.performance_comparator
        .compare_against_benchmarks(&user_uuid, &request.benchmark_symbols, time_period)
        .await
    {
        Ok(comparison) => {
            debug!("Generated benchmark comparison for user {}", user_uuid);
            Ok(Json(BenchmarkComparisonResponse {
                success: true,
                data: Some(comparison),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to compare user {} against benchmarks: {}", user_uuid, e);
            Ok(Json(BenchmarkComparisonResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get performance leaderboard
async fn get_performance_leaderboard(
    State(state): State<PerformanceApiState>,
    Query(params): Query<LeaderboardQuery>,
) -> Result<Json<LeaderboardResponse>, StatusCode> {
    // Parse leaderboard metric
    let metric = match params.metric.as_deref() {
        Some("total_return") => LeaderboardMetric::TotalReturn,
        Some("sharpe_ratio") => LeaderboardMetric::SharpeRatio,
        Some("sortino_ratio") => LeaderboardMetric::SortinoRatio,
        Some("max_drawdown") => LeaderboardMetric::MaxDrawdown,
        Some("win_rate") => LeaderboardMetric::WinRate,
        Some("profit_factor") => LeaderboardMetric::ProfitFactor,
        Some("risk_adjusted_return") => LeaderboardMetric::RiskAdjustedReturn,
        _ => LeaderboardMetric::TotalReturn,
    };

    // Parse category filter
    let category_filter = match params.category.as_deref() {
        Some("conservative") => Some(PerformanceCategory::Conservative),
        Some("moderate") => Some(PerformanceCategory::Moderate),
        Some("aggressive") => Some(PerformanceCategory::Aggressive),
        Some("high_frequency") => Some(PerformanceCategory::HighFrequency),
        Some("long_term") => Some(PerformanceCategory::LongTerm),
        _ => None,
    };

    // Parse time period
    let time_period = match params.time_period.as_deref() {
        Some("daily") => TimePeriod::Daily,
        Some("weekly") => TimePeriod::Weekly,
        Some("monthly") => TimePeriod::Monthly,
        Some("quarterly") => TimePeriod::Quarterly,
        Some("yearly") => TimePeriod::Yearly,
        Some("all_time") => TimePeriod::AllTime,
        _ => TimePeriod::AllTime,
    };

    let min_portfolio_value = params.min_portfolio_value.map(Decimal::try_from).transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let config = LeaderboardConfig {
        time_period,
        metric,
        category_filter,
        min_trades: params.min_trades,
        min_portfolio_value,
        limit: params.limit.unwrap_or(50),
    };

    match state.performance_comparator.generate_leaderboard(config).await {
        Ok(leaderboard) => {
            let total_users = leaderboard.len() as u64;
            debug!("Generated leaderboard with {} users", total_users);
            Ok(Json(LeaderboardResponse {
                success: true,
                data: Some(leaderboard),
                total_users: Some(total_users),
                error: None,
                timestamp: Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to generate leaderboard: {}", e);
            Ok(Json(LeaderboardResponse {
                success: false,
                data: None,
                total_users: None,
                error: Some(e.to_string()),
                timestamp: Utc::now(),
            }))
        }
    }
}

/// Get performance analytics summary
async fn get_performance_analytics_summary(
    State(_state): State<PerformanceApiState>,
) -> Result<Json<PerformanceAnalyticsSummary>, StatusCode> {
    // In a real implementation, this would aggregate data from all users
    // For now, return mock data
    let summary = PerformanceAnalyticsSummary {
        total_users: 1250,
        active_traders: 890,
        average_return: Decimal::try_from(12.5).unwrap(),
        median_return: Decimal::try_from(8.3).unwrap(),
        top_performer_return: Decimal::try_from(156.7).unwrap(),
        total_volume_traded: Decimal::try_from(45_678_900.0).unwrap(),
        total_fees_paid: Decimal::try_from(123_456.78).unwrap(),
        average_sharpe_ratio: Decimal::try_from(1.34).unwrap(),
        market_correlation: Decimal::try_from(0.67).unwrap(),
        risk_metrics: RiskMetricsSummary {
            average_max_drawdown: Decimal::try_from(15.2).unwrap(),
            average_volatility: Decimal::try_from(23.4).unwrap(),
            var_95_average: Decimal::try_from(4.8).unwrap(),
            high_risk_users: 156,
            conservative_users: 423,
        },
    };

    Ok(Json(summary))
}

/// Get available benchmarks
async fn get_available_benchmarks(
    State(state): State<PerformanceApiState>,
) -> Json<BenchmarkListResponse> {
    let benchmarks = state.benchmark_manager.get_supported_benchmarks();
    Json(BenchmarkListResponse {
        success: true,
        data: Some(benchmarks),
        error: None,
        timestamp: Utc::now(),
    })
}

/// Get benchmark data for a specific symbol
async fn get_benchmark_data(
    State(state): State<PerformanceApiState>,
    Path(symbol): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<BenchmarkData>, StatusCode> {
    let days = params.get("days")
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(365);

    match state.benchmark_manager.get_benchmark_data(&symbol, days).await {
        Ok(data) => {
            debug!("Retrieved benchmark data for {}", symbol);
            Ok(Json(data))
        }
        Err(e) => {
            error!("Failed to get benchmark data for {}: {}", symbol, e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Get performance API health status
async fn get_performance_health(
    State(_state): State<PerformanceApiState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    // In a real implementation, this would check the actual health of components
    let health = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        metrics_calculator_status: "operational".to_string(),
        benchmark_manager_status: "operational".to_string(),
        performance_comparator_status: "operational".to_string(),
        cache_hit_rate: Some(0.85),
        active_users: 890,
    };

    Ok(Json(health))
}

/// Performance API error handling
impl From<RiskError> for StatusCode {
    fn from(error: RiskError) -> Self {
        match error {
            RiskError::UserNotFound(_) => StatusCode::NOT_FOUND,
            RiskError::InsufficientData(_) => StatusCode::BAD_REQUEST,
            RiskError::ExternalApiError(_) => StatusCode::SERVICE_UNAVAILABLE,
            RiskError::ServiceAlreadyRunning(_) => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

