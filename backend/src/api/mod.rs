use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Method},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router, Json,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{CorsLayer, Any},
    trace::TraceLayer,
    compression::CompressionLayer,
    timeout::TimeoutLayer,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::{ApiResult, ApiError};

pub mod calculations;
pub mod positions;
pub mod admin;
pub mod test;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub cache_manager: Arc<crate::cache::CacheManager>,
    pub pricing_engine: Arc<crate::calculations::pricing::PricingEngine>,
    pub calculation_engine: Arc<crate::calculations::CalculationEngine>,
    pub indexer: Arc<crate::indexer::Indexer>,
    pub provider: Arc<dyn alloy::providers::Provider>,
    // pub position_orchestrator: Arc<crate::fetchers::orchestrator::PositionOrchestrator>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("db_pool", &"PgPool")
            .field("cache_manager", &"Arc<CacheManager>")
            .field("pricing_engine", &"Arc<PricingEngine>")
            .field("calculation_engine", &"Arc<CalculationEngine>")
            .field("indexer", &"Arc<Indexer>")
            .field("provider", &"Arc<dyn Provider>")
            .field("position_orchestrator", &"Arc<PositionOrchestrator>")
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub request_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, request_id: String) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            request_id,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(error: String, request_id: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
            request_id,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
    pub cache: String,
    pub indexer: String,
    pub uptime_seconds: u64,
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .nest("/api/v1", api_routes())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                // Rate limiting can be added later with tower-governor or similar
                .layer(CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                    .allow_headers(Any))
                .layer(middleware::from_fn(request_id_middleware))
                .layer(middleware::from_fn(auth_middleware))
                .layer(middleware::from_fn(logging_middleware))
        )
        .with_state(state)
}

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .nest("/positions", positions::positions_router())
        .nest("/calculations", calculations::routes())
        .nest("/analytics", analytics_routes())
        .nest("/admin", admin_routes())
        .nest("/test", test::test_routes())
}

fn analytics_routes() -> Router<AppState> {
    Router::new()
        .route("/top-pools", get(analytics::get_top_pools))
        .route("/il-leaderboard", get(analytics::get_il_leaderboard))
        .route("/volume-stats", get(analytics::get_volume_stats))
}

fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/refresh-views", post(admin::refresh_materialized_views))
        .route("/backfill/:address", post(admin::trigger_backfill))
        .route("/cache/clear", post(admin::clear_cache))
        .layer(middleware::from_fn(admin_auth_middleware))
}

/// Admin authentication middleware
async fn admin_auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check for admin API key in headers
    if let Some(api_key) = headers.get("x-admin-key") {
        if let Ok(key_str) = api_key.to_str() {
            // TODO: Replace with actual admin key validation
            if key_str == "admin-secret-key" {
                let _result: Result<(), ApiError> = Ok(());
                next.run(request).await;
            }
        }
    }
    
    Err(StatusCode::UNAUTHORIZED)
}

// Health check endpoint
async fn health_check(State(state): State<AppState>) -> ApiResult<Json<HealthResponse>> {
    let start_time = std::time::Instant::now();
    
    // Check database health
    let db_status = match crate::database::Database::health_check(&state.db_pool).await {
        Ok(_) => "healthy".to_string(),
        Err(e) => format!("unhealthy: {}", e),
    };

    // Check cache health
    let cache_status = match state.cache_manager.health_check().await {
        Ok(_) => "healthy".to_string(),
        Err(e) => format!("unhealthy: {}", e),
    };

    // Check indexer health
    let indexer_status = "running".to_string(); // Placeholder since get_status method doesn't exist

    let response = HealthResponse {
        status: if db_status == "healthy" && cache_status == "healthy" { 
            "healthy".to_string() 
        } else { 
            "degraded".to_string() 
        },
        database: db_status,
        cache: cache_status,
        indexer: indexer_status,
        uptime_seconds: start_time.elapsed().as_secs(),
    };

    Ok(Json(response))
}

// Metrics endpoint
async fn get_metrics(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let mut metrics = serde_json::Map::new();
    
    // Database metrics
    if let Ok(db_stats) = crate::database::Database::get_statistics(&state.db_pool).await {
        metrics.insert("database".to_string(), serde_json::to_value(db_stats).unwrap_or_default());
    }

    // Cache metrics
    let cache_metrics = state.cache_manager.get_metrics().await;
    metrics.insert("cache".to_string(), serde_json::to_value(cache_metrics).unwrap_or_default());

    // Pricing metrics
    let pricing_metrics = state.pricing_engine.get_metrics().await;
    metrics.insert("pricing".to_string(), serde_json::to_value(pricing_metrics).unwrap_or_default());

    // Indexer metrics
    match Ok::<serde_json::Value, serde_json::Error>(serde_json::json!({"placeholder": "metrics"})) {
        Ok(indexer_metrics) => {
            metrics.insert("indexer".to_string(), serde_json::to_value(indexer_metrics).unwrap_or_default());
        }
        Err(_) => {
            // Handle error case
        }
    }

    Ok(Json(serde_json::Value::Object(metrics)))
}

// Request ID middleware
async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    
    response
}

// Authentication middleware
async fn auth_middleware(headers: HeaderMap, request: Request, next: Next) -> Result<Response, ApiError> {
    // For now, we'll implement a simple API key authentication
    // In production, you'd want JWT or OAuth2
    
    if let Some(api_key) = headers.get("x-api-key") {
        let api_key_str = api_key.to_str().map_err(|_| ApiError::Unauthorized)?;
        
        // Validate API key (in production, check against database or external service)
        if !is_valid_api_key(api_key_str).await {
            return Err(ApiError::Unauthorized);
        }
    } else if requires_auth(&request) {
        return Err(ApiError::Unauthorized);
    }

    Ok(next.run(request).await)
}


// Logging middleware
async fn logging_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start_time = std::time::Instant::now();
    
    let response = next.run(request).await;
    
    let duration = start_time.elapsed();
    let status = response.status();
    
    match status.as_u16() {
        200..=299 => info!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = duration.as_millis(),
            "Request completed successfully"
        ),
        400..=499 => warn!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = duration.as_millis(),
            "Client error"
        ),
        500..=599 => error!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = duration.as_millis(),
            "Server error"
        ),
        _ => info!(
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = duration.as_millis(),
            "Request completed"
        ),
    }
    
    response
}

// Helper functions
async fn is_valid_api_key(api_key: &str) -> bool {
    // In production, validate against database or external service
    // For now, accept any non-empty key
    !api_key.is_empty()
}

async fn is_valid_admin_key(admin_key: &str) -> bool {
    // In production, validate against secure admin key storage
    // For now, check against environment variable
    if let Ok(expected_key) = std::env::var("ADMIN_API_KEY") {
        admin_key == expected_key
    } else {
        false
    }
}

fn requires_auth(request: &Request) -> bool {
    // Define which endpoints require authentication
    let path = request.uri().path();
    
    // Public endpoints that don't require auth
    let public_paths = ["/health", "/metrics"];
    
    !public_paths.iter().any(|&public_path| path.starts_with(public_path))
}

// Analytics module
pub mod analytics {
    use super::*;
    use axum::extract::{Query, State};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct TopPoolsQuery {
        pub limit: Option<i64>,
        pub timeframe: Option<String>,
    }

    pub async fn get_top_pools(
        State(state): State<AppState>,
        Query(params): Query<TopPoolsQuery>,
    ) -> ApiResult<Json<serde_json::Value>> {
        let limit = params.limit.unwrap_or(10);
        let timeframe = params.timeframe.unwrap_or_else(|| "24h".to_string());

        let pools = crate::database::queries::get_top_pools_by_volume(&state.db_pool, limit as i32).await
            .map_err(|e| ApiError::DatabaseError(crate::DatabaseError::QueryError(format!("Database error: {}", e))))?;

        Ok(Json(serde_json::to_value(pools).unwrap_or_default()))
    }

    pub async fn get_il_leaderboard(
        State(state): State<AppState>,
        Query(params): Query<TopPoolsQuery>,
    ) -> ApiResult<Json<serde_json::Value>> {
        let limit = params.limit.unwrap_or(10);

        let leaderboard = crate::database::queries::get_il_leaderboard(&state.db_pool, limit as i64).await
            .map_err(ApiError::DatabaseError)?;

        Ok(Json(serde_json::to_value(leaderboard).unwrap_or_default()))
    }

    pub async fn get_volume_stats(
        State(state): State<AppState>,
    ) -> ApiResult<Json<serde_json::Value>> {
        // Implementation for volume statistics
        let stats = serde_json::json!({
            "total_volume_24h": 0,
            "total_volume_7d": 0,
            "total_volume_30d": 0,
            "active_positions": 0
        });

        Ok(Json(stats))
    }
}

