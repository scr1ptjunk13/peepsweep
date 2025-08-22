use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde_json::{json, Value};
use crate::{api::AppState, ApiResult};

/// Refresh materialized views
pub async fn refresh_materialized_views(
    State(state): State<AppState>,
) -> ApiResult<Json<Value>> {
    // Refresh materialized views in the database
    sqlx::query("REFRESH MATERIALIZED VIEW user_positions_summary")
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to refresh materialized views: {}", e);
            crate::ApiError::CacheError(crate::CacheError::OperationError(format!("Cache clear failed: {}", e)))
        })?;

    Ok(Json(json!({"message": "Materialized views refreshed successfully"})))
}

/// Trigger backfill for a specific address
pub async fn trigger_backfill(
    Path(address): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<Value>> {
    // Validate address format
    if !address.starts_with("0x") || address.len() != 42 {
        return Err(crate::ApiError::ValidationError("Invalid address format".to_string()));
    }

    // TODO: Implement actual backfill logic
    tracing::info!("Triggering backfill for address: {}", address);
    
    Ok(Json(json!({
        "message": format!("Backfill triggered for address: {}", address),
        "address": address
    })))
}

/// Clear cache
pub async fn clear_cache(
    State(state): State<AppState>,
) -> ApiResult<Json<Value>> {
    // Clear the cache
    state.cache_manager.clear_all().await
        .map_err(|e| crate::ApiError::CacheError(crate::CacheError::OperationError(format!("Failed to flush cache: {}", e))))?;

    Ok(Json(json!({"message": "Cache cleared successfully"})))
}
