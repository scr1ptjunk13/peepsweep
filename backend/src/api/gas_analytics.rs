use crate::analytics::{GasUsageTracker, GasUsageRecord, GasEfficiencyMetrics, GasOptimizationAnalyzer, GasOptimizationInsights, GasReportsGenerator, GasUsageReport, ReportPeriod, ExportFormat};
use crate::risk_management::types::{UserId, RiskError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Gas analytics API state
#[derive(Clone)]
pub struct GasAnalyticsState {
    pub usage_tracker: Arc<GasUsageTracker>,
    pub optimization_analyzer: Arc<GasOptimizationAnalyzer>,
    pub reports_generator: Arc<GasReportsGenerator>,
}

/// Request/Response types for gas analytics API
#[derive(Debug, Deserialize)]
pub struct TrackTransactionRequest {
    pub transaction_hash: String,
    pub user_id: UserId,
    pub trade_id: String,
    pub expected_gas_limit: u64,
    pub gas_price_gwei: Decimal,
    pub trade_value_usd: Decimal,
    pub dex_name: String,
    pub route_type: String,
    pub token_pair: String,
}

#[derive(Debug, Serialize)]
pub struct TrackTransactionResponse {
    pub success: bool,
    pub message: String,
    pub transaction_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct GasUsageQuery {
    pub from_date: Option<String>, // ISO 8601 format
    pub to_date: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct GasUsageResponse {
    pub user_id: UserId,
    pub period: String,
    pub total_transactions: u64,
    pub total_gas_spent_usd: Decimal,
    pub average_efficiency_ratio: Decimal,
    pub records: Vec<GasRecordSummary>,
}

#[derive(Debug, Serialize)]
pub struct GasRecordSummary {
    pub transaction_hash: String,
    pub gas_cost_usd: Decimal,
    pub gas_efficiency: Decimal,
    pub dex_name: String,
    pub token_pair: String,
    pub timestamp: DateTime<Utc>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct OptimizationQuery {
    pub analysis_days: Option<u32>,
    pub include_recommendations: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct OptimizationResponse {
    pub user_id: UserId,
    pub efficiency_score: Decimal,
    pub potential_savings_usd: Decimal,
    pub recommendations_count: u32,
    pub insights: Option<GasOptimizationInsights>,
}

#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub report_type: String, // "daily", "weekly", "monthly", "custom"
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub include_charts: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ReportResponse {
    pub report_id: String,
    pub download_url: String,
    pub generated_at: DateTime<Utc>,
    pub report_summary: ReportSummary,
}

#[derive(Debug, Serialize)]
pub struct ReportSummary {
    pub total_transactions: u64,
    pub total_gas_spent: Decimal,
    pub efficiency_score: Decimal,
    pub top_recommendation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GasHealthResponse {
    pub status: String,
    pub pending_transactions: u64,
    pub total_tracked: u64,
    pub last_update: DateTime<Utc>,
    pub system_operational: bool,
}

#[derive(Debug, Deserialize)]
pub struct GasEstimateRequest {
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub user_id: UserId,
    pub dex_preference: Option<String>,
    pub route_type: Option<String>, // "direct", "multi_hop", "complex"
}

#[derive(Debug, Serialize)]
pub struct GasEstimateResponse {
    pub estimated_gas: u64,
    pub gas_price_gwei: Decimal,
    pub estimated_cost_usd: Decimal,
    pub estimated_cost_eth: Decimal,
    pub route_info: RouteGasInfo,
    pub confidence_level: String, // "high", "medium", "low"
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RouteGasInfo {
    pub route_type: String,
    pub dex_name: String,
    pub complexity_score: u32,
    pub gas_breakdown: GasBreakdown,
}

#[derive(Debug, Serialize)]
pub struct GasBreakdown {
    pub base_gas: u64,
    pub swap_gas: u64,
    pub bridge_gas: Option<u64>,
    pub approval_gas: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub timestamp: DateTime<Utc>,
}

/// Gas analytics API routes
pub fn gas_analytics_routes() -> Router<GasAnalyticsState> {
    Router::new()
        .route("/health", get(gas_health_check))
        .route("/track", post(track_transaction))
        .route("/usage/:user_id", get(get_user_gas_usage))
        .route("/efficiency/:user_id", get(get_gas_efficiency_metrics))
        .route("/optimization/:user_id", get(get_optimization_insights))
        .route("/recommendations/:user_id", get(get_immediate_recommendations))
        .route("/reports/generate/:user_id", post(generate_gas_report))
        .route("/reports/download/:report_id", get(download_gas_report))
        .route("/comparison", get(get_dex_comparison))
        .route("/price-recommendations", get(get_gas_price_recommendations))
        .route("/process-pending", post(process_pending_transactions))
        .route("/estimate", post(estimate_gas_cost))
}

/// Health check for gas analytics system
async fn gas_health_check(
    State(state): State<GasAnalyticsState>,
) -> Result<Json<GasHealthResponse>, (StatusCode, Json<ErrorResponse>)> {
    let health = state.usage_tracker.get_health_status().await;
    
    Ok(Json(GasHealthResponse {
        status: if health.is_operational { "healthy".to_string() } else { "degraded".to_string() },
        pending_transactions: health.pending_transactions,
        total_tracked: health.total_tracked_transactions,
        last_update: health.last_update,
        system_operational: health.is_operational,
    }))
}

/// Track a new transaction for gas analysis
async fn track_transaction(
    State(state): State<GasAnalyticsState>,
    Json(request): Json<TrackTransactionRequest>,
) -> Result<Json<TrackTransactionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let trade_id = Uuid::parse_str(&request.trade_id)
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid trade_id format".to_string(),
                code: "INVALID_TRADE_ID".to_string(),
                timestamp: Utc::now(),
            })
        ))?;

    match state.usage_tracker.track_transaction(
        request.transaction_hash.clone(),
        request.user_id,
        trade_id,
        request.expected_gas_limit,
        request.gas_price_gwei,
        request.trade_value_usd,
        request.dex_name,
        request.route_type,
        request.token_pair,
    ).await {
        Ok(_) => Ok(Json(TrackTransactionResponse {
            success: true,
            message: "Transaction tracking initiated".to_string(),
            transaction_hash: request.transaction_hash,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to track transaction: {}", e),
                code: "TRACKING_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Get gas usage data for a user
async fn get_user_gas_usage(
    Path(user_id): Path<UserId>,
    Query(query): Query<GasUsageQuery>,
    State(state): State<GasAnalyticsState>,
) -> Result<Json<GasUsageResponse>, (StatusCode, Json<ErrorResponse>)> {
    let end_date = if let Some(to_str) = query.to_date {
        DateTime::parse_from_rfc3339(&to_str)
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid to_date format".to_string(),
                    code: "INVALID_DATE".to_string(),
                    timestamp: Utc::now(),
                })
            ))?
            .with_timezone(&Utc)
    } else {
        Utc::now()
    };

    let start_date = if let Some(from_str) = query.from_date {
        DateTime::parse_from_rfc3339(&from_str)
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid from_date format".to_string(),
                    code: "INVALID_DATE".to_string(),
                    timestamp: Utc::now(),
                })
            ))?
            .with_timezone(&Utc)
    } else {
        end_date - Duration::days(30) // Default to last 30 days
    };

    match state.usage_tracker.get_user_gas_usage(user_id, start_date, end_date).await {
        Ok(records) => {
            let total_gas_spent: Decimal = records.iter().map(|r| r.gas_cost_usd).sum();
            let avg_efficiency: Decimal = if !records.is_empty() {
                records.iter().map(|r| r.gas_efficiency).sum::<Decimal>() / Decimal::from(records.len())
            } else {
                Decimal::ZERO
            };

            let mut record_summaries: Vec<GasRecordSummary> = records.into_iter().map(|r| {
                GasRecordSummary {
                    transaction_hash: r.transaction_hash,
                    gas_cost_usd: r.gas_cost_usd,
                    gas_efficiency: r.gas_efficiency,
                    dex_name: r.dex_name,
                    token_pair: r.token_pair,
                    timestamp: r.timestamp,
                    status: format!("{:?}", r.transaction_status),
                }
            }).collect();

            // Apply limit if specified
            if let Some(limit) = query.limit {
                record_summaries.truncate(limit as usize);
            }

            Ok(Json(GasUsageResponse {
                user_id,
                period: format!("{} to {}", start_date.format("%Y-%m-%d"), end_date.format("%Y-%m-%d")),
                total_transactions: record_summaries.len() as u64,
                total_gas_spent_usd: total_gas_spent,
                average_efficiency_ratio: avg_efficiency,
                records: record_summaries,
            }))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to retrieve gas usage: {}", e),
                code: "RETRIEVAL_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Get gas efficiency metrics for a user
async fn get_gas_efficiency_metrics(
    Path(user_id): Path<UserId>,
    Query(query): Query<GasUsageQuery>,
    State(state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(30);

    match state.usage_tracker.calculate_gas_efficiency_metrics(user_id, start_date, end_date).await {
        Ok(metrics) => Ok(Json(serde_json::to_value(metrics).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to calculate metrics: {}", e),
                code: "CALCULATION_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Get optimization insights for a user
async fn get_optimization_insights(
    Path(user_id): Path<UserId>,
    Query(query): Query<OptimizationQuery>,
    State(state): State<GasAnalyticsState>,
) -> Result<Json<OptimizationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let analysis_days = query.analysis_days.unwrap_or(30);
    let include_full_insights = query.include_recommendations.unwrap_or(false);

    match state.optimization_analyzer.generate_optimization_insights(user_id, analysis_days).await {
        Ok(insights) => {
            Ok(Json(OptimizationResponse {
                user_id,
                efficiency_score: insights.current_efficiency_score,
                potential_savings_usd: insights.potential_savings_usd,
                recommendations_count: insights.recommendations.len() as u32,
                insights: if include_full_insights { Some(insights) } else { None },
            }))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to generate insights: {}", e),
                code: "INSIGHTS_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Get immediate actionable recommendations
async fn get_immediate_recommendations(
    Path(user_id): Path<UserId>,
    State(state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.optimization_analyzer.get_immediate_recommendations(user_id).await {
        Ok(recommendations) => Ok(Json(serde_json::to_value(recommendations).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get recommendations: {}", e),
                code: "RECOMMENDATIONS_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Generate a gas usage report
async fn generate_gas_report(
    State(state): State<GasAnalyticsState>,
    Path(user_id): Path<UserId>,
    Json(request): Json<ReportRequest>,
) -> Result<Json<ReportResponse>, (StatusCode, Json<ErrorResponse>)> {
    let report_result = match request.report_type.as_str() {
        "daily" => {
            let date = Utc::now();
            state.reports_generator.generate_daily_report(user_id, date).await
        },
        "weekly" => {
            let week_start = Utc::now() - Duration::days(7);
            state.reports_generator.generate_weekly_report(user_id, week_start).await
        },
        "monthly" => {
            let month = Utc::now();
            state.reports_generator.generate_monthly_report(user_id, month).await
        },
        "custom" => {
            let start = request.start_date
                .ok_or_else(|| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "start_date required for custom reports".to_string(),
                        code: "MISSING_START_DATE".to_string(),
                        timestamp: Utc::now(),
                    })
                ))?;
            let end = request.end_date
                .ok_or_else(|| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "end_date required for custom reports".to_string(),
                        code: "MISSING_END_DATE".to_string(),
                        timestamp: Utc::now(),
                    })
                ))?;
            
            let start_dt = DateTime::parse_from_rfc3339(&start)
                .map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Invalid start_date format".to_string(),
                        code: "INVALID_DATE".to_string(),
                        timestamp: Utc::now(),
                    })
                ))?
                .with_timezone(&Utc);
            
            let end_dt = DateTime::parse_from_rfc3339(&end)
                .map_err(|_| (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "Invalid end_date format".to_string(),
                        code: "INVALID_DATE".to_string(),
                        timestamp: Utc::now(),
                    })
                ))?
                .with_timezone(&Utc);

            state.reports_generator.generate_custom_report(user_id, start_dt, end_dt).await
        },
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid report_type. Must be: daily, weekly, monthly, or custom".to_string(),
                code: "INVALID_REPORT_TYPE".to_string(),
                timestamp: Utc::now(),
            })
        ))
    };

    match report_result {
        Ok(report) => {
            let top_recommendation = report.recommendations.first().cloned();
            
            Ok(Json(ReportResponse {
                report_id: report.report_id.clone(),
                download_url: format!("/api/gas/reports/{}", report.report_id),
                generated_at: report.generated_at,
                report_summary: ReportSummary {
                    total_transactions: report.summary.total_transactions,
                    total_gas_spent: report.summary.total_gas_spent_usd,
                    efficiency_score: Decimal::from(85), // Mock efficiency score
                    top_recommendation,
                },
            }))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to generate report: {}", e),
                code: "REPORT_GENERATION_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Download a generated report
async fn download_gas_report(
    Path(report_id): Path<String>,
    State(_state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Mock implementation - in production, retrieve from storage
    Ok(Json(serde_json::json!({
        "report_id": report_id,
        "status": "ready",
        "download_url": format!("/downloads/gas-report-{}.json", report_id),
        "message": "Report ready for download"
    })))
}

/// Get DEX efficiency comparison
async fn get_dex_comparison(
    Query(query): Query<HashMap<String, String>>,
    State(state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let user_id_str = query.get("user_id")
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "user_id parameter required".to_string(),
                code: "MISSING_USER_ID".to_string(),
                timestamp: Utc::now(),
            })
        ))?;

    let user_id = Uuid::parse_str(user_id_str)
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid user_id format".to_string(),
                code: "INVALID_USER_ID".to_string(),
                timestamp: Utc::now(),
            })
        ))?;

    let end_date = Utc::now();
    let start_date = end_date - Duration::days(30);

    match state.usage_tracker.get_dex_gas_comparison(user_id, start_date, end_date).await {
        Ok(comparison) => Ok(Json(serde_json::to_value(comparison).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get DEX comparison: {}", e),
                code: "COMPARISON_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Get current gas price recommendations
async fn get_gas_price_recommendations(
    State(state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.usage_tracker.get_gas_price_recommendations().await {
        Ok(recommendations) => Ok(Json(serde_json::to_value(recommendations).unwrap())),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get gas price recommendations: {}", e),
                code: "PRICE_RECOMMENDATIONS_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Process pending transactions
async fn process_pending_transactions(
    State(state): State<GasAnalyticsState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.usage_tracker.process_pending_transactions().await {
        Ok(processed_count) => Ok(Json(serde_json::json!({
            "processed_transactions": processed_count,
            "timestamp": Utc::now(),
            "status": "success"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to process pending transactions: {}", e),
                code: "PROCESSING_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    }
}

/// Estimate gas cost for a pre-trade transaction
async fn estimate_gas_cost(
    State(state): State<GasAnalyticsState>,
    Json(request): Json<GasEstimateRequest>,
) -> Result<Json<GasEstimateResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse amount_in
    let _amount_in = Decimal::from_str_exact(&request.amount_in)
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid amount_in format".to_string(),
                code: "INVALID_AMOUNT".to_string(),
                timestamp: Utc::now(),
            })
        ))?;

    // Get current gas price recommendations
    let gas_price_data = match state.usage_tracker.get_gas_price_recommendations().await {
        Ok(data) => data,
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to get gas prices: {}", e),
                code: "GAS_PRICE_FAILED".to_string(),
                timestamp: Utc::now(),
            })
        ))
    };

    // Determine route type and estimate gas
    let route_type = request.route_type.as_deref().unwrap_or("direct");
    let dex_name = request.dex_preference.as_deref().unwrap_or("Uniswap");
    
    // Base gas estimates by route type
    let (base_gas, swap_gas, complexity_score) = match route_type {
        "direct" => (21000u64, 120000u64, 1u32),
        "multi_hop" => (21000u64, 180000u64, 2u32),
        "complex" => (21000u64, 250000u64, 3u32),
        _ => (21000u64, 150000u64, 2u32),
    };

    // DEX-specific gas adjustments
    let dex_multiplier = match dex_name {
        "Uniswap" => 1.0,
        "Curve" => 1.2,
        "Balancer" => 1.3,
        "dYdX" => 0.9,
        "CoW" => 0.0, // Gasless
        _ => 1.1,
    };

    let adjusted_swap_gas = (swap_gas as f64 * dex_multiplier) as u64;
    let total_gas = base_gas + adjusted_swap_gas;

    // Check if approval is needed (assume needed for ERC20 tokens)
    let approval_gas = if request.token_in.to_lowercase() != "eth" {
        Some(45000u64)
    } else {
        None
    };

    let final_gas = total_gas + approval_gas.unwrap_or(0);

    // Calculate costs
    let gas_price_gwei = gas_price_data.standard;
    let gas_price_wei = gas_price_gwei * Decimal::from(1_000_000_000); // Convert gwei to wei
    let cost_wei = Decimal::from(final_gas) * gas_price_wei;
    let cost_eth = cost_wei / Decimal::from(1_000_000_000_000_000_000u64); // Convert wei to ETH

    // Estimate ETH price (mock - in production, get from price oracle)
    let eth_price_usd = Decimal::from(3200); // Mock ETH price
    let cost_usd = cost_eth * eth_price_usd;

    // Determine confidence level based on route complexity and historical data
    let confidence_level = match complexity_score {
        1 => "high",
        2 => "medium",
        _ => "low",
    };

    // Get user's historical efficiency for this route type if available
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(7);
    
    // Try to get user's historical data to improve estimate accuracy
    if let Ok(user_records) = state.usage_tracker.get_user_gas_usage(request.user_id, start_date, end_date).await {
        // Filter records for similar trades
        let similar_trades: Vec<_> = user_records.into_iter()
            .filter(|r| r.dex_name == dex_name && r.token_pair.contains(&request.token_in))
            .collect();
        
        if !similar_trades.is_empty() {
            // Adjust estimate based on user's historical performance
            let avg_actual_gas: u64 = similar_trades.iter()
                .map(|r| r.gas_used)
                .sum::<u64>() / similar_trades.len() as u64;
            
            // Use historical average if significantly different
            let adjusted_gas = if avg_actual_gas > 0 && 
                (avg_actual_gas as f64 - final_gas as f64).abs() / final_gas as f64 > 0.1 {
                avg_actual_gas
            } else {
                final_gas
            };
            
            let adjusted_cost_wei = Decimal::from(adjusted_gas) * gas_price_wei;
            let adjusted_cost_eth = adjusted_cost_wei / Decimal::from(1_000_000_000_000_000_000u64);
            let adjusted_cost_usd = adjusted_cost_eth * eth_price_usd;
            
            return Ok(Json(GasEstimateResponse {
                estimated_gas: adjusted_gas,
                gas_price_gwei,
                estimated_cost_usd: adjusted_cost_usd,
                estimated_cost_eth: adjusted_cost_eth,
                route_info: RouteGasInfo {
                    route_type: route_type.to_string(),
                    dex_name: dex_name.to_string(),
                    complexity_score,
                    gas_breakdown: GasBreakdown {
                        base_gas,
                        swap_gas: adjusted_gas - base_gas - approval_gas.unwrap_or(0),
                        bridge_gas: None,
                        approval_gas,
                    },
                },
                confidence_level: "high".to_string(), // Higher confidence with historical data
                timestamp: Utc::now(),
            }));
        }
    }

    // Return standard estimate if no historical data available
    Ok(Json(GasEstimateResponse {
        estimated_gas: final_gas,
        gas_price_gwei,
        estimated_cost_usd: cost_usd,
        estimated_cost_eth: cost_eth,
        route_info: RouteGasInfo {
            route_type: route_type.to_string(),
            dex_name: dex_name.to_string(),
            complexity_score,
            gas_breakdown: GasBreakdown {
                base_gas,
                swap_gas: adjusted_swap_gas,
                bridge_gas: None,
                approval_gas,
            },
        },
        confidence_level: confidence_level.to_string(),
        timestamp: Utc::now(),
    }))
}
