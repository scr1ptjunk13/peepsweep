use crate::risk_management::metrics_aggregation::{
    aggregator::MetricsAggregator, types::*
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
pub struct MetricsApiState {
    pub aggregator: Arc<MetricsAggregator>,
}

pub fn create_metrics_router(state: MetricsApiState) -> Router {
    Router::new()
        .route("/metrics/snapshot", get(get_latest_snapshot))
        .route("/metrics/snapshot/:id", get(get_snapshot_by_id))
        .route("/metrics/query", post(query_metrics))
        .route("/metrics/health", get(get_health_status))
        .route("/metrics/performance", get(get_performance_metrics))
        .route("/metrics/dex-liquidity", get(get_dex_liquidity_metrics))
        .route("/metrics/bridge-status", get(get_bridge_status_metrics))
        .route("/metrics/system-health", get(get_system_health_metrics))
        .route("/metrics/anomalies", get(get_anomalies))
        .route("/metrics/insights", get(get_insights))
        .with_state(state)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub metric_type: Option<String>,
    pub aggregation: Option<String>,
    pub interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyQueryParams {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub severity: Option<String>,
}

async fn get_latest_snapshot(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsResponse<MetricsSnapshot>>, StatusCode> {
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot),
            message: "Latest metrics snapshot retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        None => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "No metrics snapshots available".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn get_snapshot_by_id(
    State(state): State<MetricsApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MetricsResponse<MetricsSnapshot>>, StatusCode> {
    // For now, return the latest snapshot if ID matches
    // In a real implementation, this would query by specific ID
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) if snapshot.id == id => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot),
            message: "Metrics snapshot retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        _ => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "Metrics snapshot not found".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn query_metrics(
    State(state): State<MetricsApiState>,
    Json(query): Json<MetricsQuery>,
) -> Result<Json<MetricsResponse<AggregatedMetrics>>, StatusCode> {
    match state.aggregator.query_metrics(query).await {
        Ok(aggregated) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(aggregated),
            message: "Metrics query executed successfully".to_string(),
            timestamp: Utc::now(),
        })),
        Err(e) => {
            eprintln!("Failed to query metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_health_status(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsResponse<HealthStatus>>, StatusCode> {
    let health_status = state.aggregator.get_health_status().await;
    
    Ok(Json(MetricsResponse {
        success: true,
        data: Some(health_status),
        message: "Health status retrieved successfully".to_string(),
        timestamp: Utc::now(),
    }))
}

async fn get_performance_metrics(
    State(state): State<MetricsApiState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<MetricsResponse<PerformanceMetrics>>, StatusCode> {
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot.performance_metrics),
            message: "Performance metrics retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        None => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "No performance metrics available".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn get_dex_liquidity_metrics(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsResponse<DexLiquidityMetrics>>, StatusCode> {
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot.dex_liquidity_metrics),
            message: "DEX liquidity metrics retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        None => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "No DEX liquidity metrics available".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn get_bridge_status_metrics(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsResponse<BridgeStatusMetrics>>, StatusCode> {
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot.bridge_status_metrics),
            message: "Bridge status metrics retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        None => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "No bridge status metrics available".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn get_system_health_metrics(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsResponse<SystemHealthMetrics>>, StatusCode> {
    match state.aggregator.get_latest_snapshot().await {
        Some(snapshot) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(snapshot.system_health_metrics),
            message: "System health metrics retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        None => Ok(Json(MetricsResponse {
            success: false,
            data: None,
            message: "No system health metrics available".to_string(),
            timestamp: Utc::now(),
        })),
    }
}

async fn get_anomalies(
    State(state): State<MetricsApiState>,
    Query(params): Query<AnomalyQueryParams>,
) -> Result<Json<MetricsResponse<Vec<MetricsAnomaly>>>, StatusCode> {
    let start_time = params.start_time.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
    let end_time = params.end_time.unwrap_or_else(|| Utc::now());
    
    // Create a query to get anomalies
    let query = MetricsQuery {
        start_time: Some(start_time),
        end_time: Some(end_time),
        metric_types: vec![MetricType::All],
        aggregation: AggregationType::Raw,
        interval: None,
    };
    
    match state.aggregator.query_metrics(query).await {
        Ok(aggregated) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(aggregated.summary.anomalies_detected),
            message: "Anomalies retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        Err(e) => {
            eprintln!("Failed to get anomalies: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_insights(
    State(state): State<MetricsApiState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<MetricsResponse<Vec<String>>>, StatusCode> {
    let start_time = params.start_time.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
    let end_time = params.end_time.unwrap_or_else(|| Utc::now());
    
    let query = MetricsQuery {
        start_time: Some(start_time),
        end_time: Some(end_time),
        metric_types: vec![MetricType::All],
        aggregation: AggregationType::Raw,
        interval: None,
    };
    
    match state.aggregator.query_metrics(query).await {
        Ok(aggregated) => Ok(Json(MetricsResponse {
            success: true,
            data: Some(aggregated.summary.key_insights),
            message: "Insights retrieved successfully".to_string(),
            timestamp: Utc::now(),
        })),
        Err(e) => {
            eprintln!("Failed to get insights: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(disabled_test)]
mod tests {
    use super::*;
    use crate::risk_management::performance_tracker::PortfolioPerformanceTracker;
    use crate::routing::liquidity_tracker::LiquidityTracker;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use std::sync::Arc;

    async fn create_test_state() -> MetricsApiState {
        // Create mock dependencies for testing - simplified for testing
        let config = MetricsAggregationConfig::default();
        
        // Create a minimal aggregator for testing without complex dependencies
        let aggregator = Arc::new(MetricsAggregator::new_for_testing(config));
        
        MetricsApiState { aggregator }
    }

    #[tokio::test]
    async fn test_get_health_status() {
        let state = create_test_state().await;
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/metrics/health").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<HealthStatus> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());
    }

    #[tokio::test]
    async fn test_get_latest_snapshot() {
        let state = create_test_state().await;
        
        // Collect a snapshot first
        let _ = state.aggregator.collect_snapshot().await;
        
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/metrics/snapshot").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<MetricsSnapshot> = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_get_performance_metrics() {
        let state = create_test_state().await;
        
        // Collect a snapshot first
        let _ = state.aggregator.collect_snapshot().await;
        
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/metrics/performance").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<PerformanceMetrics> = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_get_dex_liquidity_metrics() {
        let state = create_test_state().await;
        
        // Collect a snapshot first
        let _ = state.aggregator.collect_snapshot().await;
        
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/metrics/dex-liquidity").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<DexLiquidityMetrics> = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_query_metrics() {
        let state = create_test_state().await;
        
        // Collect a snapshot first
        let _ = state.aggregator.collect_snapshot().await;
        
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let query = MetricsQuery {
            start_time: Some(Utc::now() - chrono::Duration::hours(1)),
            end_time: Some(Utc::now()),
            metric_types: vec![MetricType::Performance],
            aggregation: AggregationType::Raw,
            interval: None,
        };
        
        let response = server.post("/metrics/query").json(&query).await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<AggregatedMetrics> = response.json();
        assert!(body.success);
    }

    #[tokio::test]
    async fn test_get_insights() {
        let state = create_test_state().await;
        
        // Collect a snapshot first
        let _ = state.aggregator.collect_snapshot().await;
        
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/metrics/insights").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: MetricsResponse<Vec<String>> = response.json();
        assert!(body.success);
    }
}
