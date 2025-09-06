use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Duration;

use crate::user_retention::trading_insights::{
    DashboardService, MarketIntelligenceEngine, PersonalizationEngine, PredictiveAnalytics,
    DashboardData, DashboardConfiguration, MarketOverview, PersonalizedInsight,
    MarketOpportunity, TimingRecommendation, RiskAdjustedRecommendation,
    PricePrediction, LiquidityForecast, MarketSentimentAnalysis,
    ChartType, DashboardComponent, ChartData,
};
use crate::websocket::insights_websocket::InsightsWebSocketServer;

#[derive(Clone)]
pub struct InsightsApiState {
    pub dashboard_service: Arc<DashboardService>,
    pub market_intelligence: Arc<MarketIntelligenceEngine>,
    pub personalization_engine: Arc<PersonalizationEngine>,
    pub predictive_analytics: Arc<PredictiveAnalytics>,
    pub websocket_server: Arc<InsightsWebSocketServer>,
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub refresh: Option<bool>,
    pub components: Option<String>, // comma-separated component names
}

#[derive(Debug, Deserialize)]
pub struct OpportunityQuery {
    pub limit: Option<u32>,
    pub risk_level: Option<String>,
    pub confidence_min: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct PredictionQuery {
    pub tokens: String, // comma-separated token symbols
    pub timeframe_hours: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TimingQuery {
    pub pairs: String, // comma-separated token pairs
}

#[derive(Debug, Deserialize)]
pub struct ChartQuery {
    pub chart_type: String,
    pub time_range_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            timestamp: chrono::Utc::now(),
        }
    }
}

pub fn create_insights_router() -> Router<InsightsApiState> {
    Router::new()
        .route("/dashboard/:user_id", get(get_dashboard_data))
        .route("/dashboard/:user_id/config", get(get_dashboard_config))
        .route("/dashboard/:user_id/config", put(update_dashboard_config))
        .route("/market/overview", get(get_market_overview))
        .route("/opportunities/:user_id", get(get_market_opportunities))
        .route("/insights/:user_id", get(get_personalized_insights))
        .route("/timing/:user_id", get(get_timing_recommendations))
        .route("/risk/:user_id", get(get_risk_recommendations))
        .route("/predictions/price", get(get_price_predictions))
        .route("/predictions/timing", get(get_timing_predictions))
        .route("/predictions/liquidity", get(get_liquidity_forecasts))
        .route("/sentiment", get(get_market_sentiment))
        .route("/charts/:user_id", get(get_chart_data))
        .route("/dashboard/:user_id/refresh", post(refresh_dashboard_component))
        .route("/ws/:client_id", get(websocket_handler))
        .route("/health", get(health_check))
}

/// Get complete dashboard data for a user
async fn get_dashboard_data(
    Path(user_id): Path<Uuid>,
    Query(query): Query<DashboardQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<DashboardData>>, StatusCode> {
    match query.refresh.unwrap_or(false) {
        true => {
            // Generate fresh dashboard data
            match state.dashboard_service.generate_dashboard_data(user_id).await {
                Ok(data) => Ok(Json(ApiResponse::success(data))),
                Err(e) => {
                    eprintln!("Error generating dashboard data: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        false => {
            // Try to get cached data first
            match state.dashboard_service.get_cached_dashboard(user_id).await {
                Some(data) => Ok(Json(ApiResponse::success(data))),
                None => {
                    // Generate fresh data if no cache
                    match state.dashboard_service.generate_dashboard_data(user_id).await {
                        Ok(data) => Ok(Json(ApiResponse::success(data))),
                        Err(e) => {
                            eprintln!("Error generating dashboard data: {}", e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
            }
        }
    }
}

/// Get dashboard configuration for a user
async fn get_dashboard_config(
    Path(user_id): Path<Uuid>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<DashboardConfiguration>>, StatusCode> {
    let config = state.dashboard_service.get_dashboard_config(user_id).await;
    Ok(Json(ApiResponse::success(config)))
}

/// Update dashboard configuration for a user
async fn update_dashboard_config(
    Path(user_id): Path<Uuid>,
    State(state): State<InsightsApiState>,
    Json(config): Json<DashboardConfiguration>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    match state.dashboard_service.update_dashboard_config(user_id, config).await {
        Ok(_) => Ok(Json(ApiResponse::success("Configuration updated successfully".to_string()))),
        Err(e) => {
            eprintln!("Error updating dashboard config: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get market overview data
async fn get_market_overview(
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<MarketOverview>>, StatusCode> {
    match state.dashboard_service.get_market_overview().await {
        Ok(overview) => Ok(Json(ApiResponse::success(overview))),
        Err(e) => {
            eprintln!("Error getting market overview: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get personalized market opportunities for a user
async fn get_opportunities(
    Path(user_id): Path<Uuid>,
    Query(query): Query<OpportunityQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<MarketOpportunity>>>, StatusCode> {
    match state.dashboard_service.get_opportunity_feed(user_id, query.limit).await {
        Ok(mut opportunities) => {
            // Filter by confidence if specified
            if let Some(min_confidence) = query.confidence_min {
                opportunities.retain(|opp| opp.confidence >= min_confidence);
            }
            
            Ok(Json(ApiResponse::success(opportunities)))
        }
        Err(e) => {
            eprintln!("Error getting opportunities: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get personalized insights for a user
async fn get_personalized_insights(
    Path(user_id): Path<Uuid>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<PersonalizedInsight>>>, StatusCode> {
    match state.personalization_engine.generate_personalized_insights(user_id).await {
        Ok(insights) => Ok(Json(ApiResponse::success(insights))),
        Err(e) => {
            eprintln!("Error getting personalized insights: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get timing recommendations for a user
async fn get_timing_recommendations(
    Path(user_id): Path<Uuid>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<TimingRecommendation>>>, StatusCode> {
    match state.personalization_engine.generate_timing_recommendations(user_id).await {
        Ok(recommendations) => Ok(Json(ApiResponse::success(recommendations))),
        Err(e) => {
            eprintln!("Error getting timing recommendations: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get risk-adjusted recommendations for a user
async fn get_risk_recommendations(
    Path(user_id): Path<Uuid>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<RiskAdjustedRecommendation>>, StatusCode> {
    match state.personalization_engine.generate_risk_adjusted_recommendations(user_id).await {
        Ok(recommendation) => Ok(Json(ApiResponse::success(recommendation))),
        Err(e) => {
            eprintln!("Error getting risk recommendations: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get price predictions for specified tokens
async fn get_price_predictions(
    Query(query): Query<PredictionQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<PricePrediction>>>, StatusCode> {
    let tokens: Vec<String> = query.tokens
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();
    
    let timeframe = Duration::hours(query.timeframe_hours.unwrap_or(24));
    
    match state.predictive_analytics.predict_price_trends(tokens, timeframe).await {
        Ok(predictions) => Ok(Json(ApiResponse::success(predictions))),
        Err(e) => {
            eprintln!("Error getting price predictions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get timing predictions for specified token pairs
async fn get_timing_predictions(
    Query(query): Query<TimingQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<crate::user_retention::trading_insights::predictive_analytics::TimingPrediction>>>, StatusCode> {
    let pairs: Vec<String> = query.pairs
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();
    
    match state.predictive_analytics.predict_optimal_timing(pairs).await {
        Ok(predictions) => Ok(Json(ApiResponse::success(predictions))),
        Err(e) => {
            eprintln!("Error getting timing predictions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get liquidity forecasts for specified token pairs
async fn get_liquidity_forecasts(
    Query(query): Query<TimingQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<Vec<LiquidityForecast>>>, StatusCode> {
    let pairs: Vec<String> = query.pairs
        .split(',')
        .map(|s| s.trim().to_uppercase())
        .collect();
    
    let timeframe = Duration::hours(24); // Default 24 hour forecast
    
    match state.predictive_analytics.forecast_liquidity(pairs, timeframe).await {
        Ok(forecasts) => Ok(Json(ApiResponse::success(forecasts))),
        Err(e) => {
            eprintln!("Error getting liquidity forecasts: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get market sentiment analysis
async fn get_market_sentiment(
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<MarketSentimentAnalysis>>, StatusCode> {
    match state.predictive_analytics.analyze_market_sentiment().await {
        Ok(sentiment) => Ok(Json(ApiResponse::success(sentiment))),
        Err(e) => {
            eprintln!("Error getting market sentiment: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get chart data for visualization
async fn get_chart_data(
    Path(user_id): Path<Uuid>,
    Query(query): Query<ChartQuery>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<ChartData>>, StatusCode> {
    let chart_type = match query.chart_type.as_str() {
        "portfolio" => ChartType::PortfolioPerformance,
        "market" => ChartType::MarketOverview,
        "gas" => ChartType::GasPrices,
        "liquidity" => ChartType::LiquidityTrends,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    let time_range = Duration::hours(query.time_range_hours.unwrap_or(24));
    let user_id_opt = if matches!(chart_type, ChartType::PortfolioPerformance) {
        Some(user_id)
    } else {
        None
    };
    
    match state.dashboard_service.get_chart_data(chart_type, user_id_opt, time_range).await {
        Ok(data) => Ok(Json(ApiResponse::success(data))),
        Err(e) => {
            eprintln!("Error getting chart data: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Refresh specific dashboard component
async fn refresh_dashboard_component(
    Path(user_id): Path<Uuid>,
    Query(component_query): Query<HashMap<String, String>>,
    State(state): State<InsightsApiState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let component_name = component_query.get("component")
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let component = match component_name.as_str() {
        "market_overview" => DashboardComponent::MarketOverview,
        "personalized_feed" => DashboardComponent::PersonalizedFeed,
        "performance_summary" => DashboardComponent::PerformanceSummary,
        "gas_optimization" => DashboardComponent::GasOptimization,
        "liquidity_insights" => DashboardComponent::LiquidityInsights,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    match state.dashboard_service.refresh_component(user_id, component).await {
        Ok(data) => Ok(Json(ApiResponse::success(data))),
        Err(e) => {
            eprintln!("Error refreshing component: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// WebSocket handler for real-time insights
async fn websocket_handler(
    Path(client_id): Path<String>,
    ws: WebSocketUpgrade,
    State(state): State<InsightsApiState>,
) -> Response {
    state.websocket_server.handle_websocket(ws, client_id).await
}

/// Health check endpoint
async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("Insights API is healthy".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;
    use axum::body::Body;
    use axum::http::Request;

    // Helper function to create test state
    fn create_test_state() -> InsightsApiState {
        // This would need to be implemented with proper test dependencies
        todo!("Implement test state creation")
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert!(response.0.success);
    }

    #[tokio::test]
    async fn test_api_response_creation() {
        let success_response: ApiResponse<String> = ApiResponse::success("test".to_string());
        assert!(success_response.success);
        assert_eq!(success_response.data, Some("test".to_string()));
        assert!(success_response.error.is_none());

        let error_response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert_eq!(error_response.error, Some("test error".to_string()));
    }
}
