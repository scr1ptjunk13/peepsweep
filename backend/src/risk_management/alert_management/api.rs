use crate::risk_management::alert_management::{
    AlertCategory, AlertManager, AlertManagerConfig, AlertSeverity, AlertStatistics, NotificationConfig, NotificationManager, RiskAlert, RiskAlertIntegration, ThresholdConfig
};
use crate::risk_management::position_tracker::{PositionTracker, PositionTrackerConfig};
use crate::risk_management::risk_engine::{RiskEngineConfig, RiskProcessingEngine};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AlertApiState {
    pub alert_manager: Arc<AlertManager>,
    pub risk_integration: Arc<RiskAlertIntegration>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAlertRequest {
    pub category: AlertCategory,
    pub current_value: f64,
    pub user_id: Option<Uuid>,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AcknowledgeAlertRequest {
    pub acknowledged_by: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateThresholdRequest {
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub threshold_value: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertQueryParams {
    pub user_id: Option<Uuid>,
    pub category: Option<AlertCategory>,
    pub severity: Option<AlertSeverity>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

pub fn create_alert_api_router(state: AlertApiState) -> Router {
    Router::new()
        .route("/alerts", get(get_alerts))
        .route("/alerts", post(create_alert))
        .route("/alerts/:alert_id", get(get_alert))
        .route("/alerts/:alert_id/acknowledge", put(acknowledge_alert))
        .route("/alerts/:alert_id/resolve", put(resolve_alert))
        .route("/alerts/statistics", get(get_alert_statistics))
        .route("/alerts/active", get(get_active_alerts))
        .route("/alerts/user/:user_id", get(get_user_alerts))
        .route("/alerts/category/:category", get(get_category_alerts))
        .route("/thresholds", put(update_threshold))
        .route("/health", get(health_check))
        .with_state(state)
}

async fn get_alerts(
    State(state): State<AlertApiState>,
    Query(params): Query<AlertQueryParams>,
) -> Result<Json<AlertResponse>, StatusCode> {
    // For now, return active alerts (can be extended with filtering)
    let alerts = state.alert_manager.get_active_alerts().await;
    
    let filtered_alerts: Vec<RiskAlert> = alerts
        .into_iter()
        .filter(|alert| {
            if let Some(user_id) = params.user_id {
                if alert.user_id != Some(user_id) {
                    return false;
                }
            }
            if let Some(category) = &params.category {
                if &alert.category != category {
                    return false;
                }
            }
            if let Some(severity) = &params.severity {
                if &alert.severity != severity {
                    return false;
                }
            }
            true
        })
        .take(params.limit.unwrap_or(100))
        .skip(params.offset.unwrap_or(0))
        .collect();

    Ok(Json(AlertResponse {
        success: true,
        message: format!("Retrieved {} alerts", filtered_alerts.len()),
        data: Some(serde_json::to_value(filtered_alerts).unwrap()),
    }))
}

async fn create_alert(
    State(state): State<AlertApiState>,
    Json(request): Json<CreateAlertRequest>,
) -> Result<Json<AlertResponse>, StatusCode> {
    match state.alert_manager.check_and_create_alert(
        request.category,
        request.current_value,
        request.user_id,
        request.metadata,
    ).await {
        Ok(Some(alert)) => Ok(Json(AlertResponse {
            success: true,
            message: "Alert created successfully".to_string(),
            data: Some(serde_json::to_value(alert).unwrap()),
        })),
        Ok(None) => Ok(Json(AlertResponse {
            success: false,
            message: "No alert threshold exceeded".to_string(),
            data: None,
        })),
        Err(e) => {
            eprintln!("Error creating alert: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_alert(
    State(state): State<AlertApiState>,
    Path(alert_id): Path<Uuid>,
) -> Result<Json<AlertResponse>, StatusCode> {
    match state.alert_manager.get_alert(alert_id).await {
        Some(alert) => Ok(Json(AlertResponse {
            success: true,
            message: "Alert retrieved successfully".to_string(),
            data: Some(serde_json::to_value(alert).unwrap()),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn acknowledge_alert(
    State(state): State<AlertApiState>,
    Path(alert_id): Path<Uuid>,
    Json(request): Json<AcknowledgeAlertRequest>,
) -> Result<Json<AlertResponse>, StatusCode> {
    match state.alert_manager.acknowledge_alert(alert_id, request.acknowledged_by).await {
        Ok(()) => Ok(Json(AlertResponse {
            success: true,
            message: "Alert acknowledged successfully".to_string(),
            data: None,
        })),
        Err(e) => {
            eprintln!("Error acknowledging alert: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn resolve_alert(
    State(state): State<AlertApiState>,
    Path(alert_id): Path<Uuid>,
) -> Result<Json<AlertResponse>, StatusCode> {
    match state.alert_manager.resolve_alert(alert_id).await {
        Ok(()) => Ok(Json(AlertResponse {
            success: true,
            message: "Alert resolved successfully".to_string(),
            data: None,
        })),
        Err(e) => {
            eprintln!("Error resolving alert: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_alert_statistics(
    State(state): State<AlertApiState>,
) -> Result<Json<AlertResponse>, StatusCode> {
    let stats = state.alert_manager.get_alert_statistics().await;
    
    Ok(Json(AlertResponse {
        success: true,
        message: "Statistics retrieved successfully".to_string(),
        data: Some(serde_json::to_value(stats).unwrap()),
    }))
}

async fn get_active_alerts(
    State(state): State<AlertApiState>,
) -> Result<Json<AlertResponse>, StatusCode> {
    let alerts = state.alert_manager.get_active_alerts().await;
    
    Ok(Json(AlertResponse {
        success: true,
        message: format!("Retrieved {} active alerts", alerts.len()),
        data: Some(serde_json::to_value(alerts).unwrap()),
    }))
}

async fn get_user_alerts(
    State(state): State<AlertApiState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<AlertResponse>, StatusCode> {
    let alerts = state.alert_manager.get_alerts_by_user(user_id).await;
    
    Ok(Json(AlertResponse {
        success: true,
        message: format!("Retrieved {} alerts for user", alerts.len()),
        data: Some(serde_json::to_value(alerts).unwrap()),
    }))
}

async fn get_category_alerts(
    State(state): State<AlertApiState>,
    Path(category): Path<String>,
) -> Result<Json<AlertResponse>, StatusCode> {
    let alert_category = match category.as_str() {
        "risk_threshold" => AlertCategory::RiskThreshold,
        "position_limit" => AlertCategory::PositionLimit,
        "liquidity_risk" => AlertCategory::LiquidityRisk,
        "price_impact" => AlertCategory::PriceImpact,
        "gas_price" => AlertCategory::GasPrice,
        "slippage_exceeded" => AlertCategory::SlippageExceeded,
        "failed_transaction" => AlertCategory::FailedTransaction,
        "system_health" => AlertCategory::SystemHealth,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    
    let alerts = state.alert_manager.get_alerts_by_category(alert_category).await;
    
    Ok(Json(AlertResponse {
        success: true,
        message: format!("Retrieved {} alerts for category", alerts.len()),
        data: Some(serde_json::to_value(alerts).unwrap()),
    }))
}

async fn update_threshold(
    State(state): State<AlertApiState>,
    Json(request): Json<UpdateThresholdRequest>,
) -> Result<Json<AlertResponse>, StatusCode> {
    match state.alert_manager.update_threshold(
        request.category,
        request.severity,
        request.threshold_value,
    ).await {
        Ok(()) => Ok(Json(AlertResponse {
            success: true,
            message: "Threshold updated successfully".to_string(),
            data: None,
        })),
        Err(e) => {
            eprintln!("Error updating threshold: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn health_check(
    State(state): State<AlertApiState>,
) -> Result<Json<AlertResponse>, StatusCode> {
    let stats = state.alert_manager.get_alert_statistics().await;
    
    let health_data = serde_json::json!({
        "status": "healthy",
        "active_alerts": stats.active_alerts,
        "total_alerts": stats.total_alerts,
        "failed_notifications": stats.failed_notifications,
        "uptime": "running"
    });
    
    Ok(Json(AlertResponse {
        success: true,
        message: "Alert management system is healthy".to_string(),
        data: Some(health_data),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::alert_management::{
        AlertManagerConfig, NotificationConfig, NotificationManager, ThresholdConfig
    };
    use crate::risk_management::risk_engine::RiskProcessingEngine;
    // use crate::trade_streaming::TradeEventStreamer;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    async fn create_test_state() -> AlertApiState {
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        let alert_config = AlertManagerConfig::default();
        let alert_manager = Arc::new(AlertManager::new(
            alert_config,
            threshold_config,
            notification_manager,
        ));

        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let config = RiskEngineConfig::default();
        let risk_engine = Arc::new(RwLock::new(RiskProcessingEngine::new(config, position_tracker)));
        let risk_integration = Arc::new(RiskAlertIntegration::new(
            alert_manager.clone(),
            risk_engine,
        ));

        AlertApiState {
            alert_manager,
            risk_integration,
        }
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_active_alerts_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/alerts/active")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_alert_statistics_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/alerts/statistics")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_create_alert_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let create_request = CreateAlertRequest {
            category: AlertCategory::RiskThreshold,
            current_value: 0.08, // 8% risk, should trigger high severity alert
            user_id: None,
            metadata: None,
        };

        let request = Request::builder()
            .method(Method::POST)
            .uri("/alerts")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&create_request).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_update_threshold_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let update_request = UpdateThresholdRequest {
            category: AlertCategory::RiskThreshold,
            severity: AlertSeverity::High,
            threshold_value: 0.06, // Update to 6%
        };

        let request = Request::builder()
            .method(Method::PUT)
            .uri("/thresholds")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&update_request).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_category_alerts_endpoint() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/alerts/category/risk_threshold")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_category_returns_bad_request() {
        let state = create_test_state().await;
        let app = create_alert_api_router(state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/alerts/category/invalid_category")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
