use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::database::{TokenRepository, UnifiedToken};

#[derive(Clone)]
pub struct TokensApiState {
    pub token_repository: Arc<TokenRepository>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub token_type: String,
    pub decimals: i32,
    pub is_verified: bool,
    pub verification_level: String,
    pub logo_url: Option<String>,
    pub price_usd: Option<String>,
    pub market_cap_usd: Option<String>,
    pub volume_24h_usd: Option<String>,
    pub chain_addresses: Vec<ChainAddress>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainAddress {
    pub chain_id: i64,
    pub address: String,
    pub is_native: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokensListResponse {
    pub success: bool,
    pub tokens: Vec<TokenResponse>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Deserialize)]
pub struct TokensQueryParams {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub search: Option<String>,
    pub chain_id: Option<i64>,
    pub verified_only: Option<bool>,
}

impl TokensApiState {
    pub fn new(token_repository: Arc<TokenRepository>) -> Self {
        Self { token_repository }
    }
}

pub fn create_tokens_router() -> Router<TokensApiState> {
    Router::new()
        .route("/", get(get_tokens))
        .route("/search", get(search_tokens))
}

/// Get all tokens with pagination and filtering
async fn get_tokens(
    State(state): State<TokensApiState>,
    Query(params): Query<TokensQueryParams>,
) -> Result<Json<TokensListResponse>, StatusCode> {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50).min(1000); // Max 1000 tokens per request
    
    info!("Fetching tokens - page: {}, per_page: {}", page, per_page);
    
    match state.token_repository.get_unified_tokens(Some(1000), Some(0)).await {
        Ok(mut tokens) => {
            // Apply filters
            if let Some(search) = &params.search {
                let search_lower = search.to_lowercase();
                tokens.retain(|token| {
                    token.symbol.to_lowercase().contains(&search_lower) ||
                    token.name.to_lowercase().contains(&search_lower)
                });
            }
            
            if params.verified_only.unwrap_or(false) {
                tokens.retain(|token| token.is_verified);
            }
            
            // Apply chain filter if specified
            if let Some(chain_id) = params.chain_id {
                tokens.retain(|token| {
                    token.chain_addresses.contains_key(&chain_id)
                });
            }
            
            let total_count = tokens.len();
            
            // Apply pagination
            let start = (page - 1) * per_page;
            let end = start + per_page;
            
            if start < tokens.len() {
                tokens = tokens.into_iter().skip(start).take(per_page).collect();
            } else {
                tokens.clear();
            }
            
            // Convert to response format
            let token_responses: Vec<TokenResponse> = tokens.into_iter().map(|token| {
                TokenResponse {
                    id: token.id.to_string(),
                    symbol: token.symbol,
                    name: token.name,
                    token_type: format!("{:?}", token.token_type),
                    decimals: token.decimals,
                    is_verified: token.is_verified,
                    verification_level: format!("{:?}", token.verification_level),
                    logo_url: token.logo_url,
                    price_usd: token.price_usd.map(|p| p.to_string()),
                    market_cap_usd: token.market_cap_usd.map(|p| p.to_string()),
                    volume_24h_usd: token.volume_24h_usd.map(|p| p.to_string()),
                    chain_addresses: token.chain_addresses.into_iter().map(|(chain_id, address)| {
                        ChainAddress {
                            chain_id,
                            address,
                            is_native: false, // Default for now
                        }
                    }).collect(),
                }
            }).collect();
            
            info!("Returning {} tokens out of {} total", token_responses.len(), total_count);
            
            Ok(Json(TokensListResponse {
                success: true,
                tokens: token_responses,
                total_count,
                page,
                per_page,
            }))
        }
        Err(e) => {
            error!("Failed to fetch tokens: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Search tokens by symbol or name
async fn search_tokens(
    State(state): State<TokensApiState>,
    Query(params): Query<TokensQueryParams>,
) -> Result<Json<TokensListResponse>, StatusCode> {
    // Reuse the get_tokens function with search parameter
    get_tokens(State(state), Query(params)).await
}
