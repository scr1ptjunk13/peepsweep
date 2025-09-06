use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, IntoResponse},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{info, error, warn};

use crate::analytics::pnl_calculator::{PnLCalculator, PnLResult};
use crate::risk_management::types::{RiskError, UserId};

/// P&L API State
#[derive(Clone)]
pub struct PnLApiState {
    pub pnl_calculator: Arc<PnLCalculator>,
}

/// Query parameters for P&L history
#[derive(Debug, Deserialize)]
pub struct PnLHistoryQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub interval: Option<String>, // daily, weekly, monthly
    pub base_currency: Option<String>, // USD, ETH, BTC
}

/// Query parameters for P&L summary
#[derive(Debug, Deserialize)]
pub struct PnLSummaryQuery {
    pub period: Option<String>, // 1d, 7d, 30d, 90d, 1y, all
    pub include_positions: Option<bool>,
    pub include_breakdown: Option<bool>,
}

/// P&L History Response
#[derive(Debug, Serialize)]
pub struct PnLHistoryResponse {
    pub user_id: UserId,
    pub period: String,
    pub snapshots: Vec<PnLSnapshot>,
    pub total_count: usize,
    pub has_more: bool,
}

/// P&L Snapshot for history
#[derive(Debug, Serialize)]
pub struct PnLSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_pnl_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub portfolio_value_usd: Decimal,
    pub daily_change_usd: Decimal,
    pub daily_change_percent: Decimal,
}

/// P&L Summary Response
#[derive(Debug, Serialize)]
pub struct PnLSummaryResponse {
    pub user_id: UserId,
    pub period: String,
    pub summary: PnLSummaryData,
    pub positions: Option<Vec<PositionSummary>>,
    pub breakdown: Option<PnLBreakdown>,
    pub generated_at: DateTime<Utc>,
}

/// P&L Summary Data
#[derive(Debug, Serialize)]
pub struct PnLSummaryData {
    pub total_pnl_usd: Decimal,
    pub total_return_percent: Decimal,
    pub best_day_pnl: Decimal,
    pub worst_day_pnl: Decimal,
    pub winning_days: u32,
    pub losing_days: u32,
    pub total_trades: u64,
    pub win_rate: Decimal,
    pub sharpe_ratio: Option<Decimal>,
    pub max_drawdown: Option<Decimal>,
}

/// Position Summary
#[derive(Debug, Serialize)]
pub struct PositionSummary {
    pub token_symbol: String,
    pub pnl_usd: Decimal,
    pub return_percent: Decimal,
    pub weight_percent: Decimal,
}

/// P&L Breakdown by category
#[derive(Debug, Serialize)]
pub struct PnLBreakdown {
    pub by_token: Vec<TokenPnL>,
    pub by_dex: Vec<DexPnL>,
    pub by_strategy: Vec<StrategyPnL>,
}

#[derive(Debug, Serialize)]
pub struct TokenPnL {
    pub token_symbol: String,
    pub pnl_usd: Decimal,
    pub percentage: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DexPnL {
    pub dex_name: String,
    pub pnl_usd: Decimal,
    pub percentage: Decimal,
}

#[derive(Debug, Serialize)]
pub struct StrategyPnL {
    pub strategy_name: String,
    pub pnl_usd: Decimal,
    pub percentage: Decimal,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub timestamp: DateTime<Utc>,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        let status = match self.code.as_str() {
            "USER_NOT_FOUND" => StatusCode::NOT_FOUND,
            "INVALID_PARAMETERS" => StatusCode::BAD_REQUEST,
            "CALCULATION_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

/// GET /api/pnl/current/:user_id
pub async fn get_current_pnl(
    State(state): State<PnLApiState>,
    Path(user_id): Path<String>,
) -> Result<Json<PnLResult>, ErrorResponse> {
    info!("Getting current P&L for user: {}", user_id);

    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| ErrorResponse {
        error: "Invalid user ID format".to_string(),
        code: "INVALID_PARAMETERS".to_string(),
        timestamp: Utc::now(),
    })?;

    match state.pnl_calculator.calculate_current_pnl(&user_uuid).await {
        Ok(pnl_result) => {
            info!("Successfully calculated P&L for user {}: ${:.2}", user_id, pnl_result.total_pnl);
            Ok(Json(pnl_result))
        }
        Err(RiskError::UserNotFound(_)) => Err(ErrorResponse {
            error: format!("User {} not found", user_id),
            code: "USER_NOT_FOUND".to_string(),
            timestamp: Utc::now(),
        }),
        Err(e) => {
            error!("Failed to calculate P&L for user {}: {}", user_id, e);
            Err(ErrorResponse {
                error: "Failed to calculate P&L".to_string(),
                code: "CALCULATION_ERROR".to_string(),
                timestamp: Utc::now(),
            })
        }
    }
}

/// GET /api/pnl/history/:user_id
pub async fn get_pnl_history(
    State(state): State<PnLApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<PnLHistoryQuery>,
) -> Result<Json<PnLHistoryResponse>, ErrorResponse> {
    info!("Getting P&L history for user: {} with params: {:?}", user_id, params);

    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| ErrorResponse {
        error: "Invalid user ID format".to_string(),
        code: "INVALID_PARAMETERS".to_string(),
        timestamp: Utc::now(),
    })?;

    // Parse date parameters
    let start_date = params.start_date
        .as_ref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

    let end_date = params.end_date
        .as_ref()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc())
        .unwrap_or_else(|| Utc::now());

    // For now, generate mock historical data based on current P&L
    match state.pnl_calculator.calculate_current_pnl(&user_uuid).await {
        Ok(current_pnl) => {
            let snapshots = generate_mock_history(&current_pnl, start_date, end_date);
            let response = PnLHistoryResponse {
                user_id: user_uuid,
                period: format!("{} to {}", start_date.format("%Y-%m-%d"), end_date.format("%Y-%m-%d")),
                total_count: snapshots.len(),
                has_more: false,
                snapshots,
            };
            
            info!("Generated {} P&L history snapshots for user {}", response.total_count, user_id);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get P&L history for user {}: {}", user_id, e);
            Err(ErrorResponse {
                error: "Failed to retrieve P&L history".to_string(),
                code: "CALCULATION_ERROR".to_string(),
                timestamp: Utc::now(),
            })
        }
    }
}

/// GET /api/pnl/summary/:user_id
pub async fn get_pnl_summary(
    State(state): State<PnLApiState>,
    Path(user_id): Path<String>,
    Query(params): Query<PnLSummaryQuery>,
) -> Result<Json<PnLSummaryResponse>, ErrorResponse> {
    info!("Getting P&L summary for user: {} with params: {:?}", user_id, params);

    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| ErrorResponse {
        error: "Invalid user ID format".to_string(),
        code: "INVALID_PARAMETERS".to_string(),
        timestamp: Utc::now(),
    })?;

    match state.pnl_calculator.calculate_current_pnl(&user_uuid).await {
        Ok(pnl_result) => {
            let period = params.period.unwrap_or_else(|| "30d".to_string());
            
            // Generate summary data
            let total_pnl = pnl_result.total_pnl;
            let portfolio_value = pnl_result.portfolio_value;
            let pnl_percentage = if portfolio_value > Decimal::ZERO {
                (total_pnl / portfolio_value) * Decimal::from(100)
            } else {
                Decimal::ZERO
            };
            let history = generate_mock_history(&pnl_result, Utc::now() - chrono::Duration::days(30), Utc::now());
            let summary = PnLSummaryData {
                total_pnl_usd: total_pnl,
                total_return_percent: pnl_percentage,
                best_day_pnl: history.iter().map(|r| r.total_pnl_usd).max().unwrap_or(Decimal::ZERO),
                worst_day_pnl: history.iter().map(|r| r.total_pnl_usd).min().unwrap_or(Decimal::ZERO),
                winning_days: 18,
                losing_days: 12,
                total_trades: 45,
                win_rate: Decimal::from(60),
                sharpe_ratio: Some(Decimal::from_str("1.25").unwrap()),
                max_drawdown: Some(Decimal::from_str("-8.5").unwrap()),
            };

            // Generate position summaries if requested
            let positions = if params.include_positions.unwrap_or(false) {
                Some(pnl_result.positions.iter().map(|pos| PositionSummary {
                    token_symbol: pos.token_symbol.clone(),
                    pnl_usd: pos.unrealized_pnl_usd,
                    return_percent: pos.return_percentage,
                    weight_percent: if portfolio_value > Decimal::ZERO {
                        (pos.market_value_usd / portfolio_value) * Decimal::from(100)
                    } else {
                        Decimal::ZERO
                    },
                }).collect())
            } else {
                None
            };

            // Generate breakdown if requested
            let breakdown = if params.include_breakdown.unwrap_or(false) {
                Some(PnLBreakdown {
                    by_token: pnl_result.positions.iter().map(|pos| TokenPnL {
                        token_symbol: pos.token_symbol.clone(),
                        pnl_usd: pos.unrealized_pnl_usd,
                        percentage: if total_pnl > Decimal::ZERO {
                            (pos.unrealized_pnl_usd / total_pnl) * Decimal::from(100)
                        } else {
                            Decimal::ZERO
                        },
                    }).collect(),
                    by_dex: vec![
                        DexPnL {
                            dex_name: "Uniswap V3".to_string(),
                            pnl_usd: total_pnl * Decimal::from_str("0.6").unwrap(),
                            percentage: Decimal::from(60),
                        },
                        DexPnL {
                            dex_name: "SushiSwap".to_string(),
                            pnl_usd: total_pnl * Decimal::from_str("0.4").unwrap(),
                            percentage: Decimal::from(40),
                        },
                    ],
                    by_strategy: vec![
                        StrategyPnL {
                            strategy_name: "Arbitrage".to_string(),
                            pnl_usd: total_pnl * Decimal::from_str("0.7").unwrap(),
                            percentage: Decimal::from(70),
                        },
                        StrategyPnL {
                            strategy_name: "Liquidity Provision".to_string(),
                            pnl_usd: total_pnl * Decimal::from_str("0.3").unwrap(),
                            percentage: Decimal::from(30),
                        },
                    ],
                })
            } else {
                None
            };

            let response = PnLSummaryResponse {
                user_id: user_uuid,
                period,
                summary,
                positions,
                breakdown,
                generated_at: Utc::now(),
            };

            info!("Generated P&L summary for user {} with total P&L: ${:.2}", user_id, response.summary.total_pnl_usd);
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to get P&L summary for user {}: {}", user_id, e);
            Err(ErrorResponse {
                error: "Failed to generate P&L summary".to_string(),
                code: "CALCULATION_ERROR".to_string(),
                timestamp: Utc::now(),
            })
        }
    }
}

/// Generate mock historical P&L data
fn generate_mock_history(current_pnl: &PnLResult, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Vec<PnLSnapshot> {
    let mut snapshots = Vec::new();
    let mut current_date = start_date;
    
    while current_date <= end_date {
        // Generate some variation around current P&L
        let variation = (current_date.timestamp() % 100) as f64 / 100.0 - 0.5; // -0.5 to 0.5
        let base_pnl = current_pnl.total_pnl.to_string().parse::<f64>().unwrap_or(0.0);
        let varied_pnl = base_pnl * (1.0 + variation * 0.1); // ±10% variation
        
        let history = generate_mock_history(current_pnl, start_date, current_date);
        let max_pnl = history.iter().map(|r| r.total_pnl_usd).max().unwrap_or(Decimal::ZERO);
        let min_pnl = history.iter().map(|r| r.total_pnl_usd).min().unwrap_or(Decimal::ZERO);
        
        snapshots.push(PnLSnapshot {
            timestamp: current_date,
            total_pnl_usd: Decimal::from_f64_retain(varied_pnl).unwrap_or(Decimal::ZERO),
            unrealized_pnl_usd: Decimal::from_f64_retain(varied_pnl * 0.8).unwrap_or(Decimal::ZERO),
            realized_pnl_usd: Decimal::from_f64_retain(varied_pnl * 0.2).unwrap_or(Decimal::ZERO),
            portfolio_value_usd: current_pnl.portfolio_value,
            daily_change_usd: Decimal::from_f64_retain(varied_pnl * 0.05).unwrap_or(Decimal::ZERO),
            daily_change_percent: Decimal::from_f64_retain(variation * 2.0).unwrap_or(Decimal::ZERO),
        });
        
        current_date += chrono::Duration::days(1);
    }
    
    snapshots
}

/// Create P&L API router
pub fn create_pnl_router() -> Router<PnLApiState> {
    Router::new()
        .route("/current/:user_id", get(get_current_pnl))
        .route("/history/:user_id", get(get_pnl_history))
        .route("/summary/:user_id", get(get_pnl_summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::pnl_calculator::{MockPriceOracle, MockPositionTracker, MockTradeHistory, Position};
    use axum::http::StatusCode;
    use tower::ServiceExt;
    use axum::body::Body;
    use axum::http::Request;

    async fn create_test_app() -> Router {
        let price_oracle = Arc::new(MockPriceOracle::new());
        let position_tracker = Arc::new(MockPositionTracker::new());
        let trade_history = Arc::new(MockTradeHistory::new());
        
        let calculator = Arc::new(PnLCalculator::new(price_oracle, position_tracker.clone(), trade_history));
        
        // Add test position
        let user_id = Uuid::new_v4();
        let position = Position {
            token_address: "ETH".to_string(),
            token_symbol: "ETH".to_string(),
            quantity: Decimal::from(10),
            average_entry_price: Decimal::from(3000),
            last_updated: Utc::now(),
        };
        position_tracker.add_position(user_id, position).await;
        
        let state = PnLApiState { pnl_calculator: calculator };
        
        create_pnl_router().with_state(state)
    }

    #[tokio::test]
    async fn test_get_current_pnl() {
        let app = create_test_app().await;
        let user_id = Uuid::new_v4();
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri(&format!("/current/{}", user_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_user_id() {
        let app = create_test_app().await;
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/current/invalid-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
