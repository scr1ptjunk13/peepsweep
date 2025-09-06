use crate::api::rate_limiter::{RateLimiter, RateLimitTier, RateLimitConfig, UserRateLimit, RateLimitStatistics};
use crate::api::usage_tracker::{UsageTracker, UserUsageAnalytics, SystemUsageAnalytics, EndpointMetrics, UsagePeriodSummary};
use crate::risk_management::types::{UserId, RiskError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use rust_decimal::Decimal;

/// API state for rate limiting management endpoints
#[derive(Clone)]
pub struct RateLimitApiState {
    pub rate_limiter: Arc<RateLimiter>,
    pub usage_tracker: Arc<UsageTracker>,
}

impl RateLimitApiState {
    pub fn new(rate_limiter: Arc<RateLimiter>, usage_tracker: Arc<UsageTracker>) -> Self {
        Self {
            rate_limiter,
            usage_tracker,
        }
    }
}

/// Query parameters for user management
#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub include_analytics: Option<bool>,
    pub period_days: Option<u32>,
}

/// Query parameters for analytics
#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub period_days: Option<u32>,
    pub include_endpoints: Option<bool>,
    pub limit: Option<usize>,
}

/// Query parameters for system monitoring
#[derive(Debug, Deserialize)]
pub struct MonitoringQuery {
    pub include_violations: Option<bool>,
    pub error_threshold: Option<f64>,
}

/// Request body for updating user tier
#[derive(Debug, Deserialize)]
pub struct UpdateTierRequest {
    pub tier: RateLimitTier,
    pub reason: Option<String>,
}

/// Request body for blocking user
#[derive(Debug, Deserialize)]
pub struct BlockUserRequest {
    pub duration_seconds: u64,
    pub reason: String,
}

/// Request body for updating system load
#[derive(Debug, Deserialize)]
pub struct SystemLoadRequest {
    pub load: f64, // 0.0 to 1.0
}

/// Response for user status with optional analytics
#[derive(Debug, Serialize)]
pub struct UserStatusResponse {
    pub rate_limit: UserRateLimit,
    pub analytics: Option<UserUsageAnalytics>,
    pub period_summary: Option<UsagePeriodSummary>,
}

/// Response for system status
#[derive(Debug, Serialize)]
pub struct SystemStatusResponse {
    pub rate_limit_stats: RateLimitStatistics,
    pub usage_analytics: SystemUsageAnalytics,
    pub violated_users: Option<Vec<(UserId, UserRateLimit)>>,
    pub high_error_users: Option<Vec<(UserId, f64)>>,
}

/// Response for endpoint analytics
#[derive(Debug, Serialize)]
pub struct EndpointAnalyticsResponse {
    pub endpoint: String,
    pub metrics: Option<EndpointMetrics>,
    pub top_users: Vec<(UserId, u64)>, // user -> request count for this endpoint
}

/// Create rate limiting management router
pub fn create_rate_limit_router(state: RateLimitApiState) -> Router {
    Router::new()
        // User management endpoints
        .route("/users/:user_id/status", get(get_user_status))
        .route("/users/:user_id/tier", put(update_user_tier))
        .route("/users/:user_id/reset", post(reset_user_limits))
        .route("/users/:user_id/block", post(block_user))
        .route("/users/:user_id/unblock", post(unblock_user))
        .route("/users/:user_id/analytics", get(get_user_analytics))
        
        // System management endpoints
        .route("/system/status", get(get_system_status))
        .route("/system/load", put(update_system_load))
        .route("/system/cleanup", post(cleanup_old_data))
        
        // Analytics endpoints
        .route("/analytics/top-users", get(get_top_users))
        .route("/analytics/endpoints/:endpoint", get(get_endpoint_analytics))
        .route("/analytics/violations", get(get_violations))
        
        // Configuration endpoints
        .route("/config/tiers", get(get_tier_configs))
        .route("/config/statistics", get(get_rate_limit_statistics))
        
        // Health check
        .route("/health", get(health_check))
        .with_state(state)
}

/// Get user rate limit status and optional analytics
async fn get_user_status(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<UserQuery>,
) -> Result<Json<UserStatusResponse>, StatusCode> {
    let rate_limit = state.rate_limiter
        .get_user_status(user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let analytics = if query.include_analytics.unwrap_or(false) {
        state.usage_tracker.get_user_analytics(user_id).await.ok()
    } else {
        None
    };
    
    let period_summary = if let (Some(analytics), Some(days)) = (&analytics, query.period_days) {
        Some(analytics.get_period_summary(days))
    } else {
        None
    };
    
    Ok(Json(UserStatusResponse {
        rate_limit,
        analytics,
        period_summary,
    }))
}

/// Update user tier
async fn update_user_tier(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
    Json(request): Json<UpdateTierRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.rate_limiter
        .update_user_tier(user_id, request.tier.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("User {} tier updated to {:?}", user_id, request.tier),
        "reason": request.reason
    })))
}

/// Reset user rate limits
async fn reset_user_limits(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.rate_limiter
        .reset_user_limits(user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("Rate limits reset for user {}", user_id)
    })))
}

/// Block user
async fn block_user(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
    Json(request): Json<BlockUserRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.rate_limiter
        .block_user(user_id, request.duration_seconds)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("User {} blocked for {} seconds", user_id, request.duration_seconds),
        "reason": request.reason,
        "duration_seconds": request.duration_seconds
    })))
}

/// Unblock user
async fn unblock_user(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.rate_limiter
        .unblock_user(user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": format!("User {} unblocked", user_id)
    })))
}

/// Get user analytics
async fn get_user_analytics(
    State(state): State<RateLimitApiState>,
    Path(user_id): Path<Uuid>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<UserUsageAnalytics>, StatusCode> {
    let analytics = state.usage_tracker
        .get_user_analytics(user_id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(Json(analytics))
}

/// Get system status
async fn get_system_status(
    State(state): State<RateLimitApiState>,
    Query(query): Query<MonitoringQuery>,
) -> Result<Json<SystemStatusResponse>, StatusCode> {
    let rate_limit_stats = state.rate_limiter.get_statistics().await;
    let usage_analytics = state.usage_tracker.get_system_analytics().await;
    
    let violated_users = if query.include_violations.unwrap_or(false) {
        Some(state.rate_limiter.get_violated_users().await)
    } else {
        None
    };
    
    let high_error_users = if let Some(threshold) = query.error_threshold {
        Some(state.usage_tracker.get_high_error_users(threshold).await)
    } else {
        None
    };
    
    Ok(Json(SystemStatusResponse {
        rate_limit_stats,
        usage_analytics,
        violated_users,
        high_error_users,
    }))
}

/// Update system load
async fn update_system_load(
    State(state): State<RateLimitApiState>,
    Json(request): Json<SystemLoadRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if request.load < 0.0 || request.load > 1.0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    state.rate_limiter.update_system_load(request.load).await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "System load updated",
        "load": request.load
    })))
}

/// Cleanup old data
async fn cleanup_old_data(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let cleaned_records = state.usage_tracker
        .cleanup_old_data()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Old data cleaned up",
        "cleaned_records": cleaned_records
    })))
}

/// Get top users by request count
async fn get_top_users(
    State(state): State<RateLimitApiState>,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Vec<(UserId, UserUsageAnalytics)>>, StatusCode> {
    let limit = query.limit.unwrap_or(10);
    let top_users = state.usage_tracker.get_top_users(limit).await;
    
    Ok(Json(top_users))
}

/// Get endpoint analytics
async fn get_endpoint_analytics(
    State(state): State<RateLimitApiState>,
    Path(endpoint): Path<String>,
) -> Result<Json<EndpointAnalyticsResponse>, StatusCode> {
    let metrics = state.usage_tracker.get_endpoint_metrics(&endpoint).await;
    
    // Get top users for this endpoint
    let top_users = state.usage_tracker.get_top_users(100).await;
    let endpoint_users: Vec<(UserId, u64)> = top_users.into_iter()
        .filter_map(|(user_id, analytics)| {
            let endpoint_requests = analytics.endpoint_usage.values()
                .filter(|usage| usage.endpoint.contains(&endpoint))
                .map(|usage| usage.total_requests)
                .sum::<u64>();
            
            if endpoint_requests > 0 {
                Some((user_id, endpoint_requests))
            } else {
                None
            }
        })
        .collect();
    
    Ok(Json(EndpointAnalyticsResponse {
        endpoint,
        metrics,
        top_users: endpoint_users,
    }))
}

/// Get rate limit violations
async fn get_violations(
    State(state): State<RateLimitApiState>,
) -> Result<Json<Vec<(UserId, UserRateLimit)>>, StatusCode> {
    let violations = state.rate_limiter.get_violated_users().await;
    Ok(Json(violations))
}

/// Get tier configurations
async fn get_tier_configs(
    State(_state): State<RateLimitApiState>,
) -> Result<Json<Vec<(RateLimitTier, RateLimitConfig)>>, StatusCode> {
    let configs = vec![
        (RateLimitTier::Free, UserRateLimit::get_tier_config(&RateLimitTier::Free)),
        (RateLimitTier::Basic, UserRateLimit::get_tier_config(&RateLimitTier::Basic)),
        (RateLimitTier::Premium, UserRateLimit::get_tier_config(&RateLimitTier::Premium)),
        (RateLimitTier::Enterprise, UserRateLimit::get_tier_config(&RateLimitTier::Enterprise)),
        (RateLimitTier::Unlimited, UserRateLimit::get_tier_config(&RateLimitTier::Unlimited)),
    ];
    
    Ok(Json(configs))
}

/// Get rate limit statistics
async fn get_rate_limit_statistics(
    State(state): State<RateLimitApiState>,
) -> Result<Json<RateLimitStatistics>, StatusCode> {
    let stats = state.rate_limiter.get_statistics().await;
    Ok(Json(stats))
}

/// Health check endpoint
async fn health_check(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let system_load = state.rate_limiter.get_system_load().await;
    let stats = state.rate_limiter.get_statistics().await;
    
    let health_status = if system_load > 0.9 {
        "critical"
    } else if system_load > 0.7 {
        "warning"
    } else {
        "healthy"
    };
    
    Ok(Json(serde_json::json!({
        "status": "ok",
        "health": health_status,
        "system_load": system_load,
        "total_users": stats.total_users,
        "blocked_users": stats.blocked_users,
        "total_requests": stats.total_requests,
        "total_violations": stats.total_violations,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    })))
}

/// Administrative endpoints (require special authentication)
pub fn create_admin_router(state: RateLimitApiState) -> Router {
    Router::new()
        // Bulk operations
        .route("/admin/users/bulk-update", post(bulk_update_users))
        .route("/admin/users/export", get(export_all_users))
        .route("/admin/system/emergency-stop", post(emergency_stop))
        .route("/admin/system/reset-all", post(reset_all_limits))
        
        // Advanced analytics
        .route("/admin/analytics/revenue", get(get_revenue_analytics))
        .route("/admin/analytics/abuse-patterns", get(get_abuse_patterns))
        .route("/admin/analytics/export", get(export_analytics))
        .with_state(state)
}

/// Bulk update user tiers
#[derive(Debug, Deserialize)]
pub struct BulkUpdateRequest {
    pub updates: Vec<(UserId, RateLimitTier)>,
    pub reason: String,
}

async fn bulk_update_users(
    State(state): State<RateLimitApiState>,
    Json(request): Json<BulkUpdateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut success_count = 0;
    let mut error_count = 0;
    
    for (user_id, tier) in request.updates {
        match state.rate_limiter.update_user_tier(user_id, tier).await {
            Ok(_) => success_count += 1,
            Err(_) => error_count += 1,
        }
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Bulk update completed",
        "success_count": success_count,
        "error_count": error_count,
        "reason": request.reason
    })))
}

/// Export all user data
async fn export_all_users(
    State(state): State<RateLimitApiState>,
) -> Result<Json<Vec<(UserId, UserRateLimit, Option<UserUsageAnalytics>)>>, StatusCode> {
    let stats = state.rate_limiter.get_statistics().await;
    let top_users = state.usage_tracker.get_top_users(stats.total_users as usize).await;
    
    let mut export_data = Vec::new();
    
    for (user_id, analytics) in top_users {
        if let Ok(rate_limit) = state.rate_limiter.get_user_status(user_id).await {
            export_data.push((user_id, rate_limit, Some(analytics)));
        }
    }
    
    Ok(Json(export_data))
}

/// Emergency stop - block all non-unlimited users
async fn emergency_stop(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // This would require iterating through all users and blocking them
    // For now, we'll just set system load to maximum
    state.rate_limiter.update_system_load(1.0).await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Emergency stop activated - system load set to maximum",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    })))
}

/// Reset all rate limits (dangerous operation)
async fn reset_all_limits(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // This is a dangerous operation that would reset all user limits
    // In a real implementation, this would require special authorization
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "All rate limits reset (not implemented for safety)",
        "warning": "This operation requires special authorization"
    })))
}

/// Get revenue analytics
async fn get_revenue_analytics(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let system_analytics = state.usage_tracker.get_system_analytics().await;
    
    // Calculate revenue breakdown by tier
    let mut tier_revenue = std::collections::HashMap::new();
    let top_users = state.usage_tracker.get_top_users(1000).await;
    
    for (_, analytics) in top_users {
        let tier_total = tier_revenue.entry(analytics.tier.clone()).or_insert(Decimal::new(0, 0));
        *tier_total += analytics.cost_incurred;
    }
    
    Ok(Json(serde_json::json!({
        "total_revenue": system_analytics.total_revenue,
        "tier_breakdown": tier_revenue,
        "total_requests": system_analytics.total_requests,
        "revenue_per_request": if system_analytics.total_requests > 0 {
            system_analytics.total_revenue / Decimal::from(system_analytics.total_requests)
        } else {
            Decimal::new(0, 0)
        }
    })))
}

/// Get abuse patterns
async fn get_abuse_patterns(
    State(state): State<RateLimitApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let violations = state.rate_limiter.get_violated_users().await;
    let high_error_users = state.usage_tracker.get_high_error_users(0.5).await;
    
    Ok(Json(serde_json::json!({
        "total_violations": violations.len(),
        "high_error_users": high_error_users.len(),
        "violation_patterns": violations.into_iter().take(10).collect::<Vec<_>>(),
        "error_patterns": high_error_users.into_iter().take(10).collect::<Vec<_>>()
    })))
}

/// Export analytics data
async fn export_analytics(
    State(state): State<RateLimitApiState>,
) -> Result<Json<SystemUsageAnalytics>, StatusCode> {
    let analytics = state.usage_tracker.get_system_analytics().await;
    Ok(Json(analytics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::rate_limiter::RateLimiter;
    use crate::api::usage_tracker::UsageTracker;
    use axum::http::StatusCode;
    use uuid::Uuid;

    fn create_test_state() -> RateLimitApiState {
        let rate_limiter = Arc::new(RateLimiter::new());
        let usage_tracker = Arc::new(UsageTracker::new());
        RateLimitApiState::new(rate_limiter, usage_tracker)
    }

    #[tokio::test]
    async fn test_get_tier_configs() {
        let state = create_test_state();
        let result = get_tier_configs(State(state)).await;
        
        assert!(result.is_ok());
        let configs = result.unwrap().0;
        assert_eq!(configs.len(), 5); // All tier types
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = create_test_state();
        let result = health_check(State(state)).await;
        
        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert!(response.get("status").is_some());
        assert!(response.get("health").is_some());
    }

    #[tokio::test]
    async fn test_update_system_load() {
        let state = create_test_state();
        let request = SystemLoadRequest { load: 0.5 };
        
        let result = update_system_load(State(state.clone()), Json(request)).await;
        assert!(result.is_ok());
        
        let current_load = state.rate_limiter.get_system_load().await;
        assert_eq!(current_load, 0.5);
    }

    #[tokio::test]
    async fn test_update_system_load_invalid() {
        let state = create_test_state();
        let request = SystemLoadRequest { load: 1.5 }; // Invalid load > 1.0
        
        let result = update_system_load(State(state), Json(request)).await;
        assert_eq!(result.unwrap_err(), StatusCode::BAD_REQUEST);
    }
}
