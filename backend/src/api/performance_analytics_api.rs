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
use chrono::Duration;

use crate::user_retention::performance_analytics::{
    UserPerformanceAnalyzer, ComparativeAnalytics, InsightsGenerator, PerformanceReporter,
};
use crate::user_retention::performance_analytics::user_analyzer::{UserPerformanceMetrics, TradingPattern, UserGrowthMetrics};
use crate::user_retention::performance_analytics::comparative_analytics::{MarketComparison, CohortCriteria};
use crate::user_retention::performance_analytics::insights_generator::{TradingInsight, PerformanceRecommendation};
use crate::user_retention::performance_analytics::reporter::{PerformanceReport, ReportType, ExportFormat};
use crate::analytics::performance_metrics::PerformanceMetricsCalculator;
use crate::analytics::benchmark_integration::BenchmarkDataManager;
use crate::position_tracking::position_tracker::PositionTracker;

#[derive(Debug, Deserialize)]
pub struct UserMetricsQuery {
    pub time_period_days: Option<i64>,
    pub include_patterns: Option<bool>,
    pub include_growth: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ComparisonQuery {
    pub benchmarks: Option<String>, // Comma-separated benchmark names
    pub time_period_days: Option<i64>,
    pub include_peers: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct InsightsQuery {
    pub include_recommendations: Option<bool>,
    pub priority_filter: Option<String>, // "critical,high,medium,low"
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub report_type: String, // "daily", "weekly", "monthly", "quarterly", "annual"
    pub export_format: Option<String>, // "json", "csv", "pdf", "excel"
    pub include_charts: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct UserMetricsResponse {
    pub metrics: UserPerformanceMetrics,
    pub trading_pattern: Option<TradingPattern>,
    pub growth_metrics: Option<UserGrowthMetrics>,
}

#[derive(Debug, Serialize)]
pub struct ComparisonResponse {
    pub market_comparison: MarketComparison,
    pub summary: ComparisonSummary,
}

#[derive(Debug, Serialize)]
pub struct ComparisonSummary {
    pub overall_rank: String,
    pub performance_category: String,
    pub key_strengths: Vec<String>,
    pub improvement_areas: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct InsightsResponse {
    pub insights: Vec<TradingInsight>,
    pub recommendations: Option<PerformanceRecommendation>,
    pub summary: InsightsSummary,
}

#[derive(Debug, Serialize)]
pub struct InsightsSummary {
    pub total_insights: usize,
    pub critical_count: usize,
    pub high_priority_count: usize,
    pub top_recommendation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub report: PerformanceReport,
    pub download_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub features_enabled: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<String>,
}

pub struct PerformanceAnalyticsState {
    pub user_analyzer: Arc<UserPerformanceAnalyzer>,
    pub comparative_analytics: Arc<ComparativeAnalytics>,
    pub insights_generator: Arc<InsightsGenerator>,
    pub performance_reporter: Arc<PerformanceReporter>,
}

impl PerformanceAnalyticsState {
    pub fn new(
        performance_calculator: Arc<PerformanceMetricsCalculator>,
        benchmark_manager: Arc<BenchmarkDataManager>,
        position_tracker: Arc<PositionTracker>,
    ) -> Self {
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(
            performance_calculator,
            position_tracker,
        ));
        let comparative_analytics = Arc::new(ComparativeAnalytics::new(benchmark_manager));
        let insights_generator = Arc::new(InsightsGenerator::new());
        let performance_reporter = Arc::new(PerformanceReporter::new());

        Self {
            user_analyzer,
            comparative_analytics,
            insights_generator,
            performance_reporter,
        }
    }
}

pub fn create_performance_analytics_router() -> Router<Arc<PerformanceAnalyticsState>> {
    Router::new()
        .route("/users/:user_id/metrics", get(get_user_metrics))
        .route("/users/:user_id/comparison", post(get_market_comparison))
        .route("/users/:user_id/insights", get(get_user_insights))
        .route("/users/:user_id/recommendations", get(get_user_recommendations))
        .route("/users/:user_id/reports", post(generate_user_report))
        .route("/reports/:report_id", get(get_report))
        .route("/reports/:report_id/export", post(export_report))
        .route("/health", get(health_check))
}

/// Get comprehensive user performance metrics
async fn get_user_metrics(
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserMetricsQuery>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<UserMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let time_period = query.time_period_days.map(Duration::days);

    // Get user performance metrics
    let metrics = state
        .user_analyzer
        .calculate_user_performance(user_id, time_period)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to calculate user metrics".to_string(),
                    code: "METRICS_CALCULATION_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Get trading patterns if requested
    let trading_pattern = if query.include_patterns.unwrap_or(false) {
        Some(
            state
                .user_analyzer
                .analyze_trading_patterns(user_id, time_period)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to analyze trading patterns".to_string(),
                            code: "PATTERN_ANALYSIS_ERROR".to_string(),
                            details: Some(e.to_string()),
                        }),
                    )
                })?,
        )
    } else {
        None
    };

    // Get growth metrics if requested
    let growth_metrics = if query.include_growth.unwrap_or(false) {
        Some(
            state
                .user_analyzer
                .calculate_growth_metrics(user_id, time_period)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to calculate growth metrics".to_string(),
                            code: "GROWTH_CALCULATION_ERROR".to_string(),
                            details: Some(e.to_string()),
                        }),
                    )
                })?,
        )
    } else {
        None
    };

    Ok(Json(UserMetricsResponse {
        metrics,
        trading_pattern,
        growth_metrics,
    }))
}

/// Get market comparison for user
async fn get_market_comparison(
    Path(user_id): Path<Uuid>,
    Query(query): Query<ComparisonQuery>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<ComparisonResponse>, (StatusCode, Json<ErrorResponse>)> {
    let time_period = Duration::days(query.time_period_days.unwrap_or(30));

    // Get user metrics and trading patterns
    let user_metrics = state
        .user_analyzer
        .calculate_user_performance(user_id, Some(time_period))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get user metrics".to_string(),
                    code: "USER_METRICS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let trading_pattern = state
        .user_analyzer
        .analyze_trading_patterns(user_id, Some(time_period))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to analyze trading patterns".to_string(),
                    code: "PATTERN_ANALYSIS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Parse benchmarks
    let benchmarks = query
        .benchmarks
        .unwrap_or_else(|| "BTC,ETH,SP500".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Generate market comparison
    let market_comparison = state
        .comparative_analytics
        .generate_market_comparison(&user_metrics, &trading_pattern, benchmarks, time_period)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate market comparison".to_string(),
                    code: "COMPARISON_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Generate summary
    let summary = generate_comparison_summary(&market_comparison);

    Ok(Json(ComparisonResponse {
        market_comparison,
        summary,
    }))
}

/// Get user insights and recommendations
async fn get_user_insights(
    Path(user_id): Path<Uuid>,
    Query(query): Query<InsightsQuery>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<InsightsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get user data
    let user_metrics = state
        .user_analyzer
        .calculate_user_performance(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get user metrics".to_string(),
                    code: "USER_METRICS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let trading_pattern = state
        .user_analyzer
        .analyze_trading_patterns(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to analyze trading patterns".to_string(),
                    code: "PATTERN_ANALYSIS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let market_comparison = state
        .comparative_analytics
        .generate_market_comparison(
            &user_metrics,
            &trading_pattern,
            vec!["BTC".to_string(), "ETH".to_string()],
            Duration::days(30),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate market comparison".to_string(),
                    code: "COMPARISON_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Generate insights
    let mut insights = state
        .insights_generator
        .generate_insights(&user_metrics, &trading_pattern, &market_comparison)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate insights".to_string(),
                    code: "INSIGHTS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Apply filters
    if let Some(priority_filter) = &query.priority_filter {
        let priorities: Vec<&str> = priority_filter.split(',').collect();
        insights.retain(|insight| {
            let priority_str = match insight.priority {
                crate::user_retention::performance_analytics::insights_generator::InsightPriority::Critical => "critical",
                crate::user_retention::performance_analytics::insights_generator::InsightPriority::High => "high",
                crate::user_retention::performance_analytics::insights_generator::InsightPriority::Medium => "medium",
                crate::user_retention::performance_analytics::insights_generator::InsightPriority::Low => "low",
            };
            priorities.contains(&priority_str)
        });
    }

    // Apply limit
    if let Some(limit) = query.limit {
        insights.truncate(limit);
    }

    // Get recommendations if requested
    let recommendations = if query.include_recommendations.unwrap_or(false) {
        Some(
            state
                .insights_generator
                .generate_recommendations(&user_metrics, &trading_pattern, &market_comparison)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to generate recommendations".to_string(),
                            code: "RECOMMENDATIONS_ERROR".to_string(),
                            details: Some(e.to_string()),
                        }),
                    )
                })?,
        )
    } else {
        None
    };

    // Generate summary
    let summary = generate_insights_summary(&insights, &recommendations);

    Ok(Json(InsightsResponse {
        insights,
        recommendations,
        summary,
    }))
}

/// Get user recommendations only
async fn get_user_recommendations(
    Path(user_id): Path<Uuid>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<PerformanceRecommendation>, (StatusCode, Json<ErrorResponse>)> {
    // Get user data
    let user_metrics = state
        .user_analyzer
        .calculate_user_performance(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get user metrics".to_string(),
                    code: "USER_METRICS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let trading_pattern = state
        .user_analyzer
        .analyze_trading_patterns(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to analyze trading patterns".to_string(),
                    code: "PATTERN_ANALYSIS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let market_comparison = state
        .comparative_analytics
        .generate_market_comparison(
            &user_metrics,
            &trading_pattern,
            vec!["BTC".to_string(), "ETH".to_string()],
            Duration::days(30),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate market comparison".to_string(),
                    code: "COMPARISON_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let recommendations = state
        .insights_generator
        .generate_recommendations(&user_metrics, &trading_pattern, &market_comparison)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate recommendations".to_string(),
                    code: "RECOMMENDATIONS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    Ok(Json(recommendations))
}

/// Generate performance report
async fn generate_user_report(
    Path(user_id): Path<Uuid>,
    Query(request): Query<ReportRequest>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<ReportResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse report type
    let report_type = match request.report_type.as_str() {
        "daily" => ReportType::Daily,
        "weekly" => ReportType::Weekly,
        "monthly" => ReportType::Monthly,
        "quarterly" => ReportType::Quarterly,
        "annual" => ReportType::Annual,
        _ => ReportType::Monthly,
    };

    // Get all required data
    let user_metrics = state
        .user_analyzer
        .calculate_user_performance(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get user metrics".to_string(),
                    code: "USER_METRICS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let growth_metrics = state
        .user_analyzer
        .calculate_growth_metrics(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to calculate growth metrics".to_string(),
                    code: "GROWTH_CALCULATION_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let trading_pattern = state
        .user_analyzer
        .analyze_trading_patterns(user_id, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to analyze trading patterns".to_string(),
                    code: "PATTERN_ANALYSIS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let market_comparison = state
        .comparative_analytics
        .generate_market_comparison(
            &user_metrics,
            &trading_pattern,
            vec!["BTC".to_string(), "ETH".to_string()],
            Duration::days(30),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate market comparison".to_string(),
                    code: "COMPARISON_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let insights = state
        .insights_generator
        .generate_insights(&user_metrics, &trading_pattern, &market_comparison)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate insights".to_string(),
                    code: "INSIGHTS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let recommendations = state
        .insights_generator
        .generate_recommendations(&user_metrics, &trading_pattern, &market_comparison)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate recommendations".to_string(),
                    code: "RECOMMENDATIONS_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Generate report
    let report = state
        .performance_reporter
        .generate_report(
            user_id,
            report_type,
            &user_metrics,
            &growth_metrics,
            &trading_pattern,
            Some(market_comparison),
            insights,
            Some(recommendations),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to generate report".to_string(),
                    code: "REPORT_GENERATION_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    Ok(Json(ReportResponse {
        report,
        download_url: None, // Would be implemented for file downloads
    }))
}

/// Get existing report
async fn get_report(
    Path(report_id): Path<Uuid>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<PerformanceReport>, (StatusCode, Json<ErrorResponse>)> {
    let report = state
        .performance_reporter
        .get_cached_report(report_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Report not found".to_string(),
                    code: "REPORT_NOT_FOUND".to_string(),
                    details: None,
                }),
            )
        })?;

    Ok(Json(report))
}

/// Export report in different formats
async fn export_report(
    Path(report_id): Path<Uuid>,
    Query(request): Query<HashMap<String, String>>,
    State(state): State<Arc<PerformanceAnalyticsState>>,
) -> Result<Json<Vec<u8>>, (StatusCode, Json<ErrorResponse>)> {
    let report = state
        .performance_reporter
        .get_cached_report(report_id)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Report not found".to_string(),
                    code: "REPORT_NOT_FOUND".to_string(),
                    details: None,
                }),
            )
        })?;

    let format = match request.get("format").map(|s| s.as_str()).unwrap_or("json") {
        "csv" => ExportFormat::CSV,
        "pdf" => ExportFormat::PDF,
        "excel" => ExportFormat::Excel,
        _ => ExportFormat::JSON,
    };

    let exported = state
        .performance_reporter
        .export_report(&report, format)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to export report".to_string(),
                    code: "EXPORT_ERROR".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    Ok(Json(exported.content))
}

/// Health check endpoint
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: "1.0.0".to_string(),
        features_enabled: vec![
            "user_metrics".to_string(),
            "market_comparison".to_string(),
            "insights_generation".to_string(),
            "performance_reports".to_string(),
        ],
    })
}

// Helper functions
fn generate_comparison_summary(market_comparison: &MarketComparison) -> ComparisonSummary {
    let performance_category = match market_comparison.performance_category {
        crate::user_retention::performance_analytics::comparative_analytics::PerformanceCategory::TopPerformer => "Top Performer",
        crate::user_retention::performance_analytics::comparative_analytics::PerformanceCategory::AboveAverage => "Above Average",
        crate::user_retention::performance_analytics::comparative_analytics::PerformanceCategory::Average => "Average",
        crate::user_retention::performance_analytics::comparative_analytics::PerformanceCategory::BelowAverage => "Below Average",
        crate::user_retention::performance_analytics::comparative_analytics::PerformanceCategory::Underperformer => "Underperformer",
    };

    let overall_rank = format!("{}th percentile", market_comparison.peer_comparison.user_percentile as u32);

    let key_strengths = vec![
        if market_comparison.user_metrics.win_rate > 60.0 {
            Some("High win rate".to_string())
        } else {
            None
        },
        if market_comparison.user_metrics.sharpe_ratio > 1.0 {
            Some("Good risk-adjusted returns".to_string())
        } else {
            None
        },
        if market_comparison.user_metrics.total_return > Decimal::from(10) {
            Some("Strong returns".to_string())
        } else {
            None
        },
    ]
    .into_iter()
    .flatten()
    .collect();

    let improvement_areas = vec![
        if market_comparison.user_metrics.win_rate < 50.0 {
            Some("Improve win rate".to_string())
        } else {
            None
        },
        if market_comparison.user_metrics.max_drawdown > rust_decimal::Decimal::from_str("0.15").unwrap() {
            Some("Reduce drawdown risk".to_string())
        } else {
            None
        },
        if market_comparison.dex_performance.optimization_opportunities.len() > 0 {
            Some("Optimize DEX usage".to_string())
        } else {
            None
        },
    ]
    .into_iter()
    .flatten()
    .collect();

    ComparisonSummary {
        overall_rank,
        performance_category: performance_category.to_string(),
        key_strengths,
        improvement_areas,
    }
}

fn generate_insights_summary(
    insights: &[TradingInsight],
    recommendations: &Option<PerformanceRecommendation>,
) -> InsightsSummary {
    let critical_count = insights
        .iter()
        .filter(|i| matches!(i.priority, crate::user_retention::performance_analytics::insights_generator::InsightPriority::Critical))
        .count();

    let high_priority_count = insights
        .iter()
        .filter(|i| matches!(i.priority, crate::user_retention::performance_analytics::insights_generator::InsightPriority::High))
        .count();

    let top_recommendation = recommendations
        .as_ref()
        .and_then(|r| r.strategy_adjustments.first())
        .map(|adj| adj.description.clone());

    InsightsSummary {
        total_insights: insights.len(),
        critical_count,
        high_priority_count,
        top_recommendation,
    }
}
