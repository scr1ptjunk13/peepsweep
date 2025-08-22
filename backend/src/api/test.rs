// src/api/test.rs - Test endpoint for modular position fetcher
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

use crate::{ApiResult, ApiError, Address};
use crate::api::AppState;

#[derive(Debug, Serialize)]
pub struct TestPositionResponse {
    pub user_address: String,
    pub total_positions: usize,
    pub protocols: Vec<String>,
    pub positions: Vec<TestPosition>,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
pub struct TestPosition {
    pub protocol: String,
    pub pool_address: String,
    pub token0_symbol: String,
    pub token1_symbol: String,
    pub position_value_usd: f64,
}

/// Test endpoint to verify modular position fetcher works
pub async fn test_positions(
    Path(user_address): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<TestPositionResponse>> {
    info!("Testing modular position fetcher for address: {}", user_address);
    
    // Parse address
    let address: Address = user_address.parse()
        .map_err(|_| ApiError::ValidationError("Invalid Ethereum address".to_string()))?;
    
    // Test on Ethereum mainnet (chain_id = 1)
    let chain_id = 1u32;
    
    match state.position_orchestrator.get_user_positions(chain_id, address).await {
        Ok(summary) => {
            info!("Successfully fetched positions: {} total", summary.positions.len());
            
            let test_positions: Vec<TestPosition> = summary.positions.iter().map(|pos| {
                TestPosition {
                    protocol: pos.protocol.clone(),
                    pool_address: format!("{:?}", pos.pool_address),
                    token0_symbol: pos.token0.symbol.clone(),
                    token1_symbol: pos.token1.symbol.clone(),
                    position_value_usd: pos.value_usd,
                }
            }).collect();
            
            let protocols: Vec<String> = summary.protocol_stats.keys().cloned().collect();
            
            let response = TestPositionResponse {
                user_address,
                total_positions: summary.positions.len(),
                protocols,
                positions: test_positions,
                fetched_at: summary.fetched_at.to_rfc3339(),
            };
            
            Ok(Json(response))
        },
        Err(e) => {
            error!("Failed to fetch positions: {}", e);
            Err(ApiError::InternalError(format!("Position fetch failed: {}", e)))
        }
    }
}

/// Test endpoint to check loaded protocol configs
pub async fn test_protocols(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<String>>> {
    info!("Testing loaded protocol configurations");
    
    let protocol_names = state.position_orchestrator.get_protocol_names();
    info!("Loaded protocols: {:?}", protocol_names);
    
    Ok(Json(protocol_names))
}

pub fn test_routes() -> Router<AppState> {
    Router::new()
        .route("/test/positions/:address", get(test_positions))
        .route("/test/protocols", get(test_protocols))
}
