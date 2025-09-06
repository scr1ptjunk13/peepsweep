use crate::routing::user_preferences::{RoutingPreferences, RoutingPreferencesUpdate, UserPreferenceManager};
use crate::routing::strategy_templates::{StrategyTemplateManager, StrategyTemplate, RiskLevel};
use crate::routing::preference_router::{PreferenceRouter, PreferenceOptimizedRoute};
use crate::types::RouteRequest;
use crate::risk_management::types::{UserId, RiskError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use rust_decimal::Decimal;
use std::str::FromStr;

/// API state for routing preferences endpoints
#[derive(Clone)]
pub struct RoutingPreferencesState {
    pub preference_manager: Arc<UserPreferenceManager>,
    pub strategy_manager: Arc<StrategyTemplateManager>,
    pub preference_router: Arc<PreferenceRouter>,
}

impl RoutingPreferencesState {
    pub fn new(
        preference_manager: Arc<UserPreferenceManager>,
        strategy_manager: Arc<StrategyTemplateManager>,
        preference_router: Arc<PreferenceRouter>,
    ) -> Self {
        Self {
            preference_manager,
            strategy_manager,
            preference_router,
        }
    }
}

/// Query parameters for preferences
#[derive(Debug, Deserialize)]
pub struct PreferencesQuery {
    pub include_templates: Option<bool>,
    pub risk_level: Option<String>,
}

/// Query parameters for custom quotes
#[derive(Debug, Deserialize)]
pub struct CustomQuoteQuery {
    pub from_token: String,
    pub to_token: String,
    pub amount: String,
    pub strategy_template: Option<String>,
}

/// Response for preferences endpoint
#[derive(Debug, Serialize)]
pub struct PreferencesResponse {
    pub preferences: RoutingPreferences,
    pub available_templates: Option<Vec<StrategyTemplate>>,
    pub status: String,
    pub timestamp: u64,
}

/// Response for strategy templates endpoint
#[derive(Debug, Serialize)]
pub struct StrategyTemplatesResponse {
    pub templates: Vec<StrategyTemplate>,
    pub total_count: usize,
    pub filtered_by: Option<String>,
    pub timestamp: u64,
}

/// Response for custom quote endpoint
#[derive(Debug, Serialize)]
pub struct CustomQuoteResponse {
    pub route: PreferenceOptimizedRoute,
    pub applied_preferences: RoutingPreferences,
    pub execution_summary: ExecutionSummary,
    pub timestamp: u64,
}

/// Execution summary for transparency
#[derive(Debug, Serialize)]
pub struct ExecutionSummary {
    pub total_routes_evaluated: usize,
    pub routes_filtered_out: usize,
    pub optimization_time_ms: u64,
    pub confidence_score: Decimal,
}

/// Request for updating preferences
#[derive(Debug, Deserialize)]
pub struct UpdatePreferencesRequest {
    pub preferences: RoutingPreferencesUpdate,
    pub apply_template: Option<String>,
}

/// Request for applying strategy template
#[derive(Debug, Deserialize)]
pub struct ApplyTemplateRequest {
    pub template_name: String,
    pub override_settings: Option<RoutingPreferencesUpdate>,
}

/// Routing preferences API routes
pub fn routing_preferences_routes() -> Router<RoutingPreferencesState> {
    Router::new()
        .route("/preferences/:user_id", get(get_user_preferences))
        .route("/preferences/:user_id", put(update_user_preferences))
        .route("/preferences/:user_id/template", post(apply_strategy_template))
        .route("/strategies", get(get_strategy_templates))
        .route("/strategies/recommend", post(get_strategy_recommendations))
        .route("/custom-quote/:user_id", post(get_custom_quote))
        .route("/preferences/:user_id/reset", post(reset_user_preferences))
        .route("/health", get(health_check))
}

/// Get user routing preferences
async fn get_user_preferences(
    State(state): State<RoutingPreferencesState>,
    Path(user_id): Path<String>,
    Query(query): Query<PreferencesQuery>,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    match state.preference_manager.get_preferences(user_uuid).await {
        Ok(preferences) => {
            let available_templates = if query.include_templates.unwrap_or(false) {
                let templates = if let Some(risk_level_str) = query.risk_level {
                    let risk_level = parse_risk_level(&risk_level_str)?;
                    state.strategy_manager.get_templates_by_risk_level(&risk_level)
                        .into_iter().cloned().collect()
                } else {
                    state.strategy_manager.get_all_templates().values().cloned().collect()
                };
                Some(templates)
            } else {
                None
            };

            let response = PreferencesResponse {
                preferences,
                available_templates,
                status: "success".to_string(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            Ok(Json(response))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get preferences: {}", e)))
        }
    }
}

/// Update user routing preferences
async fn update_user_preferences(
    State(state): State<RoutingPreferencesState>,
    Path(user_id): Path<String>,
    Json(request): Json<UpdatePreferencesRequest>,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    // Apply template first if specified
    if let Some(template_name) = request.apply_template {
        if let Some(template) = state.strategy_manager.get_template(&template_name) {
            let template_preferences = template_to_preferences(user_uuid, template);
            state.preference_manager.set_preferences(user_uuid, template_preferences).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to apply template: {}", e)))?;
        } else {
            return Err((StatusCode::BAD_REQUEST, "Template not found".to_string()));
        }
    }

    // Apply preference updates
    match state.preference_manager.update_preferences(user_uuid, request.preferences).await {
        Ok(updated_preferences) => {
            let response = PreferencesResponse {
                preferences: updated_preferences,
                available_templates: None,
                status: "updated".to_string(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            Ok(Json(response))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update preferences: {}", e)))
        }
    }
}

/// Apply strategy template to user preferences
async fn apply_strategy_template(
    State(state): State<RoutingPreferencesState>,
    Path(user_id): Path<String>,
    Json(request): Json<ApplyTemplateRequest>,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    let template = state.strategy_manager.get_template(&request.template_name)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Strategy template not found".to_string()))?;

    // Convert template to preferences
    let mut preferences = template_to_preferences(user_uuid, template);

    // Apply any override settings
    if let Some(overrides) = request.override_settings {
        preferences.update(overrides);
    }

    match state.preference_manager.set_preferences(user_uuid, preferences).await {
        Ok(_) => {
            let updated_preferences = state.preference_manager.get_preferences(user_uuid).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to retrieve updated preferences: {}", e)))?;

            let response = PreferencesResponse {
                preferences: updated_preferences,
                available_templates: None,
                status: "template_applied".to_string(),
                timestamp: chrono::Utc::now().timestamp() as u64,
            };
            Ok(Json(response))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to apply template: {}", e)))
        }
    }
}

/// Get available strategy templates
async fn get_strategy_templates(
    State(state): State<RoutingPreferencesState>,
    Query(query): Query<PreferencesQuery>,
) -> Result<Json<StrategyTemplatesResponse>, (StatusCode, String)> {
    let (templates, filtered_by) = if let Some(risk_level_str) = query.risk_level {
        let risk_level = parse_risk_level(&risk_level_str)?;
        let filtered_templates: Vec<StrategyTemplate> = state.strategy_manager.get_templates_by_risk_level(&risk_level)
            .into_iter().cloned().collect();
        (filtered_templates, Some(format!("risk_level: {}", risk_level_str)))
    } else {
        let all_templates = state.strategy_manager.get_all_templates().values().cloned().collect();
        (all_templates, None)
    };

    let response = StrategyTemplatesResponse {
        total_count: templates.len(),
        templates,
        filtered_by,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };

    Ok(Json(response))
}

/// Get strategy recommendations based on trade characteristics
async fn get_strategy_recommendations(
    State(state): State<RoutingPreferencesState>,
    Json(request): Json<RecommendationRequest>,
) -> Result<Json<RecommendationResponse>, (StatusCode, String)> {
    let trade_amount = Decimal::from_str(&request.trade_amount_usd)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid trade amount".to_string()))?;

    let risk_level = parse_risk_level(&request.risk_tolerance)?;

    let recommendations = state.strategy_manager.recommend_templates(
        trade_amount,
        request.is_time_sensitive,
        &risk_level,
    );

    let recommended_templates: Vec<StrategyTemplate> = recommendations.iter()
        .filter_map(|name| state.strategy_manager.get_template(name))
        .cloned()
        .collect();

    let response = RecommendationResponse {
        recommended_templates,
        recommendation_reasons: generate_recommendation_reasons(&request, &recommendations),
        timestamp: chrono::Utc::now().timestamp() as u64,
    };

    Ok(Json(response))
}

/// Get custom quote with user preferences
async fn get_custom_quote(
    State(state): State<RoutingPreferencesState>,
    Path(user_id): Path<String>,
    Query(query): Query<CustomQuoteQuery>,
) -> Result<Json<CustomQuoteResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    let amount = Decimal::from_str(&query.amount)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid amount format".to_string()))?;

    // Apply strategy template if specified
    if let Some(template_name) = query.strategy_template {
        if let Some(template) = state.strategy_manager.get_template(&template_name) {
            let template_preferences = template_to_preferences(user_uuid, template);
            state.preference_manager.set_preferences(user_uuid, template_preferences).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to apply template: {}", e)))?;
        }
    }

    let route_request = RouteRequest {
        from_token: query.from_token,
        to_token: query.to_token,
        amount,
        user_id: Some(user_uuid),
    };

    let start_time = std::time::Instant::now();

    match state.preference_router.get_preference_optimized_route(user_uuid, route_request).await {
        Ok(optimized_route) => {
            let execution_time = start_time.elapsed().as_millis() as u64;
            
            let preferences = state.preference_manager.get_preferences(user_uuid).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get preferences: {}", e)))?;

            let execution_summary = ExecutionSummary {
                total_routes_evaluated: 10, // This would be tracked in the router
                routes_filtered_out: 5,     // This would be tracked in the router
                optimization_time_ms: execution_time,
                confidence_score: optimized_route.preference_score,
            };

            let response = CustomQuoteResponse {
                route: optimized_route,
                applied_preferences: preferences,
                execution_summary,
                timestamp: chrono::Utc::now().timestamp() as u64,
            };

            Ok(Json(response))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get custom quote: {}", e)))
        }
    }
}

/// Reset user preferences to default
async fn reset_user_preferences(
    State(state): State<RoutingPreferencesState>,
    Path(user_id): Path<String>,
) -> Result<Json<PreferencesResponse>, (StatusCode, String)> {
    let user_uuid = Uuid::parse_str(&user_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user ID format".to_string()))?;

    // Delete existing preferences
    state.preference_manager.delete_preferences(user_uuid).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to delete preferences: {}", e)))?;

    // Get new default preferences
    let default_preferences = state.preference_manager.get_preferences(user_uuid).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create default preferences: {}", e)))?;

    let response = PreferencesResponse {
        preferences: default_preferences,
        available_templates: None,
        status: "reset".to_string(),
        timestamp: chrono::Utc::now().timestamp() as u64,
    };

    Ok(Json(response))
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "routing_preferences",
        "timestamp": chrono::Utc::now().timestamp(),
        "version": "1.0.0"
    })))
}

/// Helper function to convert strategy template to routing preferences
fn template_to_preferences(user_id: UserId, template: &StrategyTemplate) -> RoutingPreferences {
    RoutingPreferences {
        user_id,
        dex_preferences: template.recommended_dex_preferences.clone(),
        optimization_strategy: template.strategy.clone(),
        mev_protection_level: template.mev_protection.clone(),
        max_hop_count: template.max_hop_count,
        gas_vs_price_preference: template.gas_vs_price_preference,
        blacklisted_dexs: template.blacklisted_dexs.clone(),
        whitelisted_dexs: None,
        blacklisted_tokens: Vec::new(),
        max_slippage_tolerance: template.max_slippage_tolerance,
        min_liquidity_threshold: template.min_liquidity_threshold,
        created_at: chrono::Utc::now().timestamp() as u64,
        updated_at: chrono::Utc::now().timestamp() as u64,
    }
}

/// Helper function to parse risk level from string
fn parse_risk_level(risk_level_str: &str) -> Result<RiskLevel, (StatusCode, String)> {
    match risk_level_str.to_lowercase().as_str() {
        "conservative" => Ok(RiskLevel::Conservative),
        "moderate" => Ok(RiskLevel::Moderate),
        "aggressive" => Ok(RiskLevel::Aggressive),
        "custom" => Ok(RiskLevel::Custom),
        _ => Err((StatusCode::BAD_REQUEST, "Invalid risk level".to_string())),
    }
}

/// Generate recommendation reasons for transparency
fn generate_recommendation_reasons(
    request: &RecommendationRequest,
    recommendations: &[String],
) -> Vec<String> {
    let mut reasons = Vec::new();
    
    let trade_amount = Decimal::from_str(&request.trade_amount_usd).unwrap_or_default();
    
    if request.is_time_sensitive && recommendations.contains(&"speed_first".to_string()) {
        reasons.push("Speed First recommended for time-sensitive trades".to_string());
    }
    
    if trade_amount > Decimal::new(100000, 0) && recommendations.contains(&"best_price".to_string()) {
        reasons.push("Best Price recommended for large trades to minimize slippage".to_string());
    }
    
    if trade_amount > Decimal::new(100000, 0) && recommendations.contains(&"mev_protected".to_string()) {
        reasons.push("MEV Protection recommended for large trades to prevent front-running".to_string());
    }
    
    if trade_amount < Decimal::new(1000, 0) && recommendations.contains(&"gas_optimized".to_string()) {
        reasons.push("Gas Optimized recommended for small trades to minimize fees".to_string());
    }
    
    match request.risk_tolerance.to_lowercase().as_str() {
        "conservative" => {
            if recommendations.contains(&"conservative_defi".to_string()) {
                reasons.push("Conservative DeFi strategy matches your risk tolerance".to_string());
            }
        }
        "aggressive" => {
            if recommendations.contains(&"aggressive_yield".to_string()) {
                reasons.push("Aggressive Yield strategy matches your risk tolerance".to_string());
            }
        }
        _ => {}
    }
    
    if reasons.is_empty() {
        reasons.push("Balanced strategy recommended as a safe default".to_string());
    }
    
    reasons
}

/// Request for strategy recommendations
#[derive(Debug, Deserialize)]
pub struct RecommendationRequest {
    pub trade_amount_usd: String,
    pub is_time_sensitive: bool,
    pub risk_tolerance: String,
    pub trade_type: Option<String>,
}

/// Response for strategy recommendations
#[derive(Debug, Serialize)]
pub struct RecommendationResponse {
    pub recommended_templates: Vec<StrategyTemplate>,
    pub recommendation_reasons: Vec<String>,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_health_check() {
        let app = Router::new()
            .route("/health", get(health_check));
        
        let server = TestServer::new(app).unwrap();
        let response = server.get("/health").await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: serde_json::Value = response.json();
        assert_eq!(body["status"], "healthy");
        assert_eq!(body["service"], "routing_preferences");
    }

    #[test]
    fn test_risk_level_parsing() {
        assert!(matches!(parse_risk_level("conservative"), Ok(RiskLevel::Conservative)));
        assert!(matches!(parse_risk_level("moderate"), Ok(RiskLevel::Moderate)));
        assert!(matches!(parse_risk_level("aggressive"), Ok(RiskLevel::Aggressive)));
        assert!(matches!(parse_risk_level("custom"), Ok(RiskLevel::Custom)));
        assert!(parse_risk_level("invalid").is_err());
    }

    #[test]
    fn test_template_to_preferences_conversion() {
        let user_id = Uuid::new_v4();
        let template = StrategyTemplate {
            name: "Test Template".to_string(),
            description: "Test".to_string(),
            strategy: crate::routing::user_preferences::OptimizationStrategy::Balanced,
            mev_protection: crate::routing::user_preferences::MevProtectionLevel::Medium,
            max_hop_count: 3,
            gas_vs_price_preference: Decimal::new(5, 1),
            max_slippage_tolerance: Decimal::new(5, 1),
            min_liquidity_threshold: Decimal::new(50000, 0),
            recommended_dex_preferences: std::collections::HashMap::new(),
            blacklisted_dexs: Vec::new(),
            use_cases: Vec::new(),
            risk_level: crate::routing::strategy_templates::RiskLevel::Moderate,
        };
        
        let preferences = template_to_preferences(user_id, &template);
        assert_eq!(preferences.user_id, user_id);
        assert_eq!(preferences.max_hop_count, 3);
        assert_eq!(preferences.gas_vs_price_preference, Decimal::new(5, 1));
    }

    #[test]
    fn test_recommendation_reasons_generation() {
        let request = RecommendationRequest {
            trade_amount_usd: "500000".to_string(),
            is_time_sensitive: true,
            risk_tolerance: "conservative".to_string(),
            trade_type: None,
        };
        
        let recommendations = vec![
            "speed_first".to_string(),
            "best_price".to_string(),
            "mev_protected".to_string(),
        ];
        
        let reasons = generate_recommendation_reasons(&request, &recommendations);
        assert!(!reasons.is_empty());
        assert!(reasons.iter().any(|r| r.contains("time-sensitive")));
        assert!(reasons.iter().any(|r| r.contains("large trades")));
    }
}
