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
use uuid::Uuid;

use crate::database::{TokenRepository, models::*};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub is_verified: bool,
    pub chain_addresses: HashMap<String, String>,
    pub logo_url: Option<String>,
    pub price_usd: Option<rust_decimal::Decimal>,
    pub market_cap_usd: Option<rust_decimal::Decimal>,
    pub volume_24h_usd: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenListResponse {
    pub tokens: Vec<TokenResponse>,
    pub total_count: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Deserialize)]
pub struct TokenQueryParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub chain_id: Option<i64>,
    pub search: Option<String>,
    pub verified_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub token_type: String,
    pub coingecko_id: Option<String>,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub twitter_handle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTokenAddressRequest {
    pub chain_id: i64,
    pub address: String,
    pub is_native: bool,
    pub is_wrapped: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenStatsResponse {
    pub total_tokens: i64,
    pub verified_tokens: i64,
    pub chain_counts: HashMap<i64, i64>,
    pub recent_additions: i64,
}

pub struct TokenApiState {
    pub token_repository: Arc<TokenRepository>,
}

impl TokenApiState {
    pub fn new(token_repository: Arc<TokenRepository>) -> Self {
        Self { token_repository }
    }
}

pub fn create_token_routes() -> Router<Arc<TokenApiState>> {
    Router::new()
        .route("/tokens", get(list_tokens).post(create_token))
        .route("/tokens/:id", get(get_token))
        .route("/tokens/:id/addresses", post(add_token_address))
        .route("/tokens/search", get(search_tokens))
        .route("/tokens/stats", get(get_token_stats))
        .route("/tokens/by-symbol/:symbol", get(get_token_by_symbol))
        .route("/tokens/by-chain/:chain_id", get(get_tokens_by_chain))
}

// List tokens with pagination and filtering
async fn list_tokens(
    State(state): State<Arc<TokenApiState>>,
    Query(params): Query<TokenQueryParams>,
) -> Result<Json<TokenListResponse>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(1000);
    let offset = params.offset.unwrap_or(0);

    match state.token_repository.get_unified_tokens(Some(limit), Some(offset)).await {
        Ok(tokens) => {
            let token_responses: Vec<TokenResponse> = tokens
                .into_iter()
                .map(|token| TokenResponse {
                    id: token.id,
                    symbol: token.symbol,
                    name: token.name,
                    decimals: token.decimals,
                    is_verified: token.is_verified,
                    chain_addresses: token.chain_addresses,
                    logo_url: token.logo_url,
                    price_usd: token.price_usd,
                    market_cap_usd: token.market_cap_usd,
                    volume_24h_usd: token.volume_24h_usd,
                })
                .collect();

            let total_count = state.token_repository.get_token_count().await.unwrap_or(0);

            Ok(Json(TokenListResponse {
                tokens: token_responses,
                total_count,
                page: offset / limit,
                limit,
            }))
        }
        Err(e) => {
            error!("Failed to list tokens: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Get token by ID
async fn get_token(
    State(state): State<Arc<TokenApiState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<TokenResponse>, StatusCode> {
    // For now, we'll implement this by getting all tokens and filtering
    // In a production system, you'd want a direct get_token_by_id method
    match state.token_repository.get_unified_tokens(Some(1000), Some(0)).await {
        Ok(tokens) => {
            if let Some(token) = tokens.into_iter().find(|t| t.id == id) {
                Ok(Json(TokenResponse {
                    id: token.id,
                    symbol: token.symbol,
                    name: token.name,
                    decimals: token.decimals,
                    is_verified: token.is_verified,
                    chain_addresses: token.chain_addresses,
                    logo_url: token.logo_url,
                    price_usd: token.price_usd,
                    market_cap_usd: token.market_cap_usd,
                    volume_24h_usd: token.volume_24h_usd,
                }))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(e) => {
            error!("Failed to get token {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Create new token
async fn create_token(
    State(state): State<Arc<TokenApiState>>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let token_type = match request.token_type.as_str() {
        "Native" => TokenType::Native,
        "ERC20" => TokenType::ERC20,
        "Wrapped" => TokenType::Wrapped,
        "Stable" => TokenType::Stable,
        _ => TokenType::ERC20,
    };

    let new_token = NewToken {
        symbol: request.symbol,
        name: request.name,
        coingecko_id: request.coingecko_id,
        token_type,
        decimals: request.decimals,
        total_supply: None,
        is_verified: false,
        verification_level: VerificationLevel::Unverified,
        description: request.description,
        website_url: request.website_url,
        twitter_handle: request.twitter_handle,
        telegram_url: None,
        discord_url: None,
    };

    match state.token_repository.create_token(new_token).await {
        Ok(token_id) => {
            info!("Created new token with ID: {}", token_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "token_id": token_id,
                "message": "Token created successfully"
            })))
        }
        Err(e) => {
            error!("Failed to create token: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Add token address for a chain
async fn add_token_address(
    State(state): State<Arc<TokenApiState>>,
    Path(token_id): Path<Uuid>,
    Json(request): Json<AddTokenAddressRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let new_address = NewTokenAddress {
        token_id,
        chain_id: request.chain_id,
        address: request.address,
        is_native: request.is_native,
        is_wrapped: request.is_wrapped,
        proxy_address: None,
        implementation_address: None,
    };

    match state.token_repository.add_token_address(new_address).await {
        Ok(address_id) => {
            info!("Added address for token {}: {}", token_id, address_id);
            Ok(Json(serde_json::json!({
                "success": true,
                "address_id": address_id,
                "message": "Token address added successfully"
            })))
        }
        Err(e) => {
            error!("Failed to add token address: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Search tokens
async fn search_tokens(
    State(state): State<Arc<TokenApiState>>,
    Query(params): Query<TokenQueryParams>,
) -> Result<Json<TokenListResponse>, StatusCode> {
    let search_query = params.search.unwrap_or_default();
    let limit = params.limit.unwrap_or(50).min(100);

    if search_query.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    match state.token_repository.search_tokens(&search_query, Some(limit)).await {
        Ok(tokens) => {
            let token_responses: Vec<TokenResponse> = tokens
                .into_iter()
                .map(|token| TokenResponse {
                    id: token.id,
                    symbol: token.symbol,
                    name: token.name,
                    decimals: token.decimals,
                    is_verified: token.is_verified,
                    chain_addresses: token.chain_addresses,
                    logo_url: token.logo_url,
                    price_usd: token.price_usd,
                    market_cap_usd: token.market_cap_usd,
                    volume_24h_usd: token.volume_24h_usd,
                })
                .collect();

            Ok(Json(TokenListResponse {
                tokens: token_responses,
                total_count: token_responses.len() as i64,
                page: 0,
                limit,
            }))
        }
        Err(e) => {
            error!("Failed to search tokens: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Get token statistics
async fn get_token_stats(
    State(state): State<Arc<TokenApiState>>,
) -> Result<Json<TokenStatsResponse>, StatusCode> {
    match state.token_repository.get_token_count().await {
        Ok(total_tokens) => {
            let chain_counts = state.token_repository.get_chain_token_counts().await.unwrap_or_default();
            
            Ok(Json(TokenStatsResponse {
                total_tokens,
                verified_tokens: 0, // TODO: Implement verified count query
                chain_counts,
                recent_additions: 0, // TODO: Implement recent additions query
            }))
        }
        Err(e) => {
            error!("Failed to get token stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Get token by symbol
async fn get_token_by_symbol(
    State(state): State<Arc<TokenApiState>>,
    Path(symbol): Path<String>,
) -> Result<Json<TokenResponse>, StatusCode> {
    match state.token_repository.get_token_by_symbol(&symbol).await {
        Ok(Some(token)) => {
            Ok(Json(TokenResponse {
                id: token.id,
                symbol: token.symbol,
                name: token.name,
                decimals: token.decimals,
                is_verified: token.is_verified,
                chain_addresses: token.chain_addresses,
                logo_url: token.logo_url,
                price_usd: token.price_usd,
                market_cap_usd: token.market_cap_usd,
                volume_24h_usd: token.volume_24h_usd,
            }))
        }
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get token by symbol {}: {}", symbol, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Get tokens by chain
async fn get_tokens_by_chain(
    State(state): State<Arc<TokenApiState>>,
    Path(chain_id): Path<i64>,
) -> Result<Json<TokenListResponse>, StatusCode> {
    match state.token_repository.get_tokens_by_chain(chain_id).await {
        Ok(tokens) => {
            let token_responses: Vec<TokenResponse> = tokens
                .into_iter()
                .map(|token| TokenResponse {
                    id: token.id,
                    symbol: token.symbol,
                    name: token.name,
                    decimals: token.decimals,
                    is_verified: token.is_verified,
                    chain_addresses: token.chain_addresses,
                    logo_url: token.logo_url,
                    price_usd: token.price_usd,
                    market_cap_usd: token.market_cap_usd,
                    volume_24h_usd: token.volume_24h_usd,
                })
                .collect();

            Ok(Json(TokenListResponse {
                tokens: token_responses,
                total_count: token_responses.len() as i64,
                page: 0,
                limit: token_responses.len() as i64,
            }))
        }
        Err(e) => {
            error!("Failed to get tokens for chain {}: {}", chain_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
