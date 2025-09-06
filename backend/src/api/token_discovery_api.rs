use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

use crate::token_registry::{
    TokenDiscoveryService, TokenDiscoveryScheduler, TokenDiscoveryResult,
    TokenDiscoveryStats, ChainTokenList, DiscoveredToken, TokenRegistryConfig,
};

#[derive(Clone)]
pub struct TokenDiscoveryApiState {
    pub discovery_service: Arc<TokenDiscoveryService>,
    pub scheduler: Arc<TokenDiscoveryScheduler>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryTriggerRequest {
    pub chain_ids: Option<Vec<u64>>,
    pub immediate: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub success: bool,
    pub message: String,
    pub result: Option<TokenDiscoveryResult>,
    pub stats: Option<TokenDiscoveryStats>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainTokensResponse {
    pub success: bool,
    pub chain_id: u64,
    pub chain_name: String,
    pub tokens: Vec<DiscoveredToken>,
    pub total_count: usize,
    pub last_updated: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AllTokensResponse {
    pub success: bool,
    pub chains: HashMap<u64, ChainTokenList>,
    pub total_tokens: usize,
    pub total_chains: usize,
}

#[derive(Debug, Deserialize)]
pub struct TokenQueryParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub verified_only: Option<bool>,
    pub min_volume: Option<f64>,
}

impl TokenDiscoveryApiState {
    pub fn new(
        discovery_service: Arc<TokenDiscoveryService>,
        scheduler: Arc<TokenDiscoveryScheduler>,
    ) -> Self {
        Self {
            discovery_service,
            scheduler,
        }
    }
}

pub fn create_token_discovery_router() -> Router<TokenDiscoveryApiState> {
    Router::new()
        .route("/discovery/trigger", post(trigger_discovery))
        .route("/discovery/stats", get(get_discovery_stats))
        .route("/discovery/status", get(get_discovery_status))
        .route("/tokens/chain/:chain_id", get(get_chain_tokens))
        .route("/tokens/all", get(get_all_tokens))
        .route("/scheduler/start", post(start_scheduler))
        .route("/scheduler/stop", post(stop_scheduler))
        .route("/scheduler/status", get(get_scheduler_status))
        .route("/supported-chains", get(get_supported_chains))
}

/// Trigger token discovery manually
async fn trigger_discovery(
    State(state): State<TokenDiscoveryApiState>,
    Json(request): Json<DiscoveryTriggerRequest>,
) -> Result<Json<DiscoveryResponse>, StatusCode> {
    info!("Manual token discovery triggered: {:?}", request);

    let result = if request.immediate.unwrap_or(false) {
        // Trigger immediate discovery via scheduler
        state.scheduler.trigger_immediate_run().await
    } else if let Some(chain_ids) = request.chain_ids {
        // Trigger discovery for specific chains
        let mut total_discovered = 0;
        let mut total_added = 0;
        let mut total_updated = 0;
        let mut errors = Vec::new();

        for chain_id in chain_ids {
            match state.discovery_service.trigger_chain_discovery(chain_id).await {
                Ok(chain_result) => {
                    total_discovered += chain_result.tokens_discovered;
                    total_added += chain_result.tokens_added;
                    total_updated += chain_result.tokens_updated;
                }
                Err(e) => {
                    errors.push(format!("Chain {}: {}", chain_id, e));
                }
            }
        }

        TokenDiscoveryResult {
            success: errors.is_empty(),
            tokens_discovered: total_discovered,
            tokens_added: total_added,
            tokens_updated: total_updated,
            chains_processed: vec![], // Would need to track this
            errors,
            duration_seconds: 0, // Would need to track this
        }
    } else {
        // Trigger full discovery
        state.discovery_service.discover_all_tokens().await
    };

    let stats = state.discovery_service.get_stats().await;

    Ok(Json(DiscoveryResponse {
        success: result.success,
        message: if result.success {
            format!("Discovery completed: {} tokens discovered, {} added, {} updated", 
                   result.tokens_discovered, result.tokens_added, result.tokens_updated)
        } else {
            format!("Discovery completed with errors: {}", result.errors.join(", "))
        },
        result: Some(result),
        stats: Some(stats),
    }))
}

/// Get current discovery statistics
async fn get_discovery_stats(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<TokenDiscoveryStats>, StatusCode> {
    let stats = state.discovery_service.get_stats().await;
    Ok(Json(stats))
}

/// Get discovery service status
async fn get_discovery_status(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<DiscoveryResponse>, StatusCode> {
    let stats = state.discovery_service.get_stats().await;
    let scheduler_running = state.scheduler.is_running().await;
    
    Ok(Json(DiscoveryResponse {
        success: true,
        message: format!("Discovery service active. Scheduler: {}. Last run: {} tokens", 
                        if scheduler_running { "running" } else { "stopped" },
                        stats.total_tokens),
        result: None,
        stats: Some(stats),
    }))
}

/// Get tokens for a specific chain
async fn get_chain_tokens(
    State(state): State<TokenDiscoveryApiState>,
    Path(chain_id): Path<u64>,
    Query(params): Query<TokenQueryParams>,
) -> Result<Json<ChainTokensResponse>, StatusCode> {
    match state.discovery_service.get_chain_tokens(chain_id).await {
        Some(chain_tokens) => {
            let mut tokens = chain_tokens.tokens;
            
            // Apply filters
            if params.verified_only.unwrap_or(false) {
                tokens.retain(|t| t.verified);
            }
            
            if let Some(min_volume) = params.min_volume {
                tokens.retain(|t| t.trading_volume_24h.unwrap_or(0.0) >= min_volume);
            }
            
            // Apply pagination
            let total_count = tokens.len();
            let offset = params.offset.unwrap_or(0);
            let limit = params.limit.unwrap_or(100).min(1000); // Max 1000 tokens per request
            
            if offset < tokens.len() {
                tokens = tokens.into_iter().skip(offset).take(limit).collect();
            } else {
                tokens.clear();
            }
            
            Ok(Json(ChainTokensResponse {
                success: true,
                chain_id,
                chain_name: chain_tokens.chain_name,
                tokens,
                total_count,
                last_updated: chain_tokens.last_updated,
            }))
        }
        None => {
            Ok(Json(ChainTokensResponse {
                success: false,
                chain_id,
                chain_name: format!("Chain {}", chain_id),
                tokens: Vec::new(),
                total_count: 0,
                last_updated: 0,
            }))
        }
    }
}

/// Get all discovered tokens across all chains
async fn get_all_tokens(
    State(state): State<TokenDiscoveryApiState>,
    Query(params): Query<TokenQueryParams>,
) -> Result<Json<AllTokensResponse>, StatusCode> {
    let all_tokens = state.discovery_service.get_all_discovered_tokens().await;
    
    let mut filtered_chains = HashMap::new();
    let mut total_tokens = 0;
    
    for (chain_id, mut chain_tokens) in all_tokens {
        // Apply filters
        if params.verified_only.unwrap_or(false) {
            chain_tokens.tokens.retain(|t| t.verified);
        }
        
        if let Some(min_volume) = params.min_volume {
            chain_tokens.tokens.retain(|t| t.trading_volume_24h.unwrap_or(0.0) >= min_volume);
        }
        
        // Apply pagination per chain
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(100);
        
        if offset < chain_tokens.tokens.len() {
            chain_tokens.tokens = chain_tokens.tokens.into_iter().skip(offset).take(limit).collect();
        } else {
            chain_tokens.tokens.clear();
        }
        
        total_tokens += chain_tokens.tokens.len();
        filtered_chains.insert(chain_id, chain_tokens);
    }
    
    let total_chains = filtered_chains.len();
    Ok(Json(AllTokensResponse {
        success: true,
        chains: filtered_chains,
        total_tokens,
        total_chains,
    }))
}

/// Start the discovery scheduler
async fn start_scheduler(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<DiscoveryResponse>, StatusCode> {
    if state.scheduler.is_running().await {
        return Ok(Json(DiscoveryResponse {
            success: false,
            message: "Scheduler is already running".to_string(),
            result: None,
            stats: None,
        }));
    }
    
    state.scheduler.start().await;
    info!("Token discovery scheduler started via API");
    
    Ok(Json(DiscoveryResponse {
        success: true,
        message: "Token discovery scheduler started successfully".to_string(),
        result: None,
        stats: None,
    }))
}

/// Stop the discovery scheduler
async fn stop_scheduler(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<DiscoveryResponse>, StatusCode> {
    if !state.scheduler.is_running().await {
        return Ok(Json(DiscoveryResponse {
            success: false,
            message: "Scheduler is not running".to_string(),
            result: None,
            stats: None,
        }));
    }
    
    state.scheduler.stop().await;
    info!("Token discovery scheduler stopped via API");
    
    Ok(Json(DiscoveryResponse {
        success: true,
        message: "Token discovery scheduler stopped successfully".to_string(),
        result: None,
        stats: None,
    }))
}

/// Get scheduler status
async fn get_scheduler_status(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    use tokio::time::Instant;
    let is_running = state.scheduler.is_running().await;
    let last_run = state.scheduler.last_run_time().await;
    let next_run = state.scheduler.next_run_time().await;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "scheduler_running": is_running,
        "last_run": last_run.map(|t| t.elapsed().as_secs()),
        "next_run": next_run.map(|t| {
            let now = Instant::now();
            if t.elapsed().as_secs() == 0 {
                0
            } else {
                0
            }
        }),
        "status": if is_running { "running" } else { "stopped" }
    })))
}

/// Get supported chains
async fn get_supported_chains(
    State(state): State<TokenDiscoveryApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let supported_chains = state.discovery_service.get_supported_chains();
    
    let chain_info: Vec<serde_json::Value> = supported_chains.into_iter().map(|chain_id| {
        let chain_name = match chain_id {
            1 => "Ethereum",
            137 => "Polygon",
            43114 => "Avalanche",
            42161 => "Arbitrum",
            10 => "Optimism",
            8453 => "Base",
            56 => "BNB Chain",
            250 => "Fantom",
            100 => "Gnosis",
            _ => "Unknown",
        };
        
        serde_json::json!({
            "chain_id": chain_id,
            "name": chain_name,
            "supported": true
        })
    }).collect();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "supported_chains": chain_info,
        "total_chains": chain_info.len()
    })))
}
