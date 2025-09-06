use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use super::chain_abstractor::{ChainAbstractor, UnifiedQuote, ExecutionResult, OperationType};
use super::unified_token_interface::{UnifiedToken, TokenPrice, CrossChainTokenMapping};

#[derive(Clone)]
pub struct ChainAbstractionApiState {
    pub chain_abstractor: Arc<ChainAbstractor>,
    pub discovery_service: Arc<crate::token_registry::TokenDiscoveryService>,
}

#[derive(Debug, Deserialize)]
pub struct QuoteRequest {
    pub from_token: String,
    pub to_token: String,
    pub amount: String,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub user_address: String,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    pub quote: UnifiedQuote,
    pub user_address: String,
}

#[derive(Debug, Serialize)]
pub struct QuoteResponse {
    pub quote: UnifiedQuote,
    pub formatted_amount_in: String,
    pub formatted_amount_out: String,
    pub estimated_time_minutes: f64,
}

#[derive(Debug, Serialize)]
pub struct ExecutionResponse {
    pub result: ExecutionResult,
    pub transaction_hash: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct TokensResponse {
    pub tokens: Vec<UnifiedToken>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ChainsResponse {
    pub chains: Vec<ChainInfo>,
    pub total_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ChainInfo {
    pub chain_id: u64,
    pub name: String,
    pub native_token: String,
    pub supported_tokens: Vec<String>,
    pub is_testnet: bool,
}

#[derive(Debug, Serialize)]
pub struct BridgeableTokensResponse {
    pub tokens: Vec<String>,
    pub recommended_token: Option<String>,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct TokenPriceResponse {
    pub price: TokenPrice,
    pub formatted_price: String,
}

#[derive(Debug, Serialize)]
pub struct CrossChainMappingResponse {
    pub mapping: CrossChainTokenMapping,
    pub supported_chains: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub chain_id: Option<u64>,
    pub token_type: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BridgeQuery {
    pub from_chain_id: u64,
    pub to_chain_id: u64,
}

pub fn create_chain_abstraction_router() -> Router<ChainAbstractionApiState> {
    Router::new()
        // Quote endpoints
        .route("/quote", post(get_unified_quote))
        .route("/quote/estimate", post(estimate_quote))
        
        // Execution endpoints
        .route("/execute", post(execute_operation))
        .route("/execution/:tx_hash/status", get(get_execution_status))
        
        // Token endpoints
        .route("/tokens", get(get_tokens))
        .route("/tokens/:symbol", get(get_token_details))
        .route("/tokens/:symbol/price", get(get_token_price))
        .route("/tokens/:symbol/mapping", get(get_token_mapping))
        .route("/tokens/:symbol/address/:chain_id", get(get_token_address))
        
        // Chain endpoints
        .route("/chains", get(get_supported_chains))
        .route("/chains/:chain_id/tokens", get(get_chain_tokens))
        
        // Bridge endpoints
        .route("/bridge/tokens", get(get_bridgeable_tokens))
        .route("/bridge/recommended", get(get_recommended_bridge_token))
        
        // Utility endpoints
        .route("/format/:symbol/:amount", get(format_token_amount))
        .route("/parse/:symbol/:formatted_amount", get(parse_token_amount))
        .route("/health", get(health_check))
}

/// Get unified quote for cross-chain operations
async fn get_unified_quote(
    State(_state): State<ChainAbstractionApiState>,
    Json(request): Json<QuoteRequest>,
) -> Result<Json<UnifiedQuote>, StatusCode> {
    info!("Getting unified quote for {} -> {} on chains {} -> {}", 
          request.from_token, request.to_token, request.from_chain_id, request.to_chain_id);

    // Mock implementation for now
    let quote = UnifiedQuote {
        operation_type: OperationType::SameChainSwap,
        from_chain_id: request.from_chain_id,
        to_chain_id: request.to_chain_id,
        token_in: request.from_token,
        token_out: request.to_token,
        amount_in: request.amount,
        amount_out: "1000000".to_string(),
        estimated_gas: "150000".to_string(),
        execution_time_seconds: 30,
        route_steps: vec![],
        total_fees_usd: 5.0,
        price_impact: 0.1,
        confidence_score: 0.9,
    };

    Ok(Json(quote))
}

/// Get quote estimate without full routing
async fn estimate_quote(
    State(state): State<ChainAbstractionApiState>,
    Json(request): Json<QuoteRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Getting quote estimate for {} -> {}", request.from_token, request.to_token);

    // Quick estimate without full routing
    let operation_type = if request.from_chain_id == request.to_chain_id {
        if request.from_token == request.to_token {
            "same_token"
        } else {
            "same_chain_swap"
        }
    } else if request.from_token == request.to_token {
        "cross_chain_bridge"
    } else {
        "cross_chain_swap"
    };

    let estimated_time = match operation_type {
        "same_token" => 0,
        "same_chain_swap" => 30,
        "cross_chain_bridge" => 300,
        "cross_chain_swap" => 600,
        _ => 300,
    };

    Ok(Json(serde_json::json!({
        "operation_type": operation_type,
        "estimated_time_seconds": estimated_time,
        "estimated_gas": "150000",
        "is_supported": true
    })))
}

/// Execute unified operation
async fn execute_operation(
    State(state): State<ChainAbstractionApiState>,
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecutionResponse>, StatusCode> {
    info!("Executing unified operation: {:?}", request.quote.operation_type);

    match state.chain_abstractor.execute_unified_operation(&request.quote, &request.user_address).await {
        Ok(result) => {
            let tx_hash = result.transaction_hash.clone();
            let status = format!("{:?}", result.status);

            Ok(Json(ExecutionResponse {
                result,
                transaction_hash: Some(tx_hash),
                status,
            }))
        }
        Err(e) => {
            error!("Failed to execute operation: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get execution status by transaction hash
async fn get_execution_status(
    State(_state): State<ChainAbstractionApiState>,
    Path(tx_hash): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Getting execution status for tx: {}", tx_hash);

    // Mock implementation - in production, query blockchain
    Ok(Json(serde_json::json!({
        "transaction_hash": tx_hash,
        "status": "Success",
        "confirmations": 12,
        "gas_used": "145000",
        "block_number": 18500000
    })))
}

/// Get supported tokens - now returns discovered tokens from token registry
async fn get_tokens(
    State(state): State<ChainAbstractionApiState>,
    Query(query): Query<TokenQuery>,
) -> Result<Json<TokensResponse>, StatusCode> {
    info!("Getting tokens with query: {:?}", query);

    // Get discovered tokens from token registry instead of hardcoded ones
    let all_discovered_tokens = state.discovery_service.get_all_discovered_tokens().await;
    
    let mut tokens_list = Vec::new();
    
    // Convert discovered tokens to UnifiedToken format with individual chain entries
    for (chain_id, chain_token_list) in all_discovered_tokens {
        // Filter by chain if specified
        if let Some(requested_chain_id) = query.chain_id {
            if chain_id != requested_chain_id as u64 {
                continue;
            }
        }
        
        for discovered_token in chain_token_list.tokens {
            // Convert each discovered token to UnifiedToken with single chain
            let mut chain_addresses = std::collections::HashMap::new();
            chain_addresses.insert(chain_id as u64, discovered_token.address.clone());
            
            let unified_token = UnifiedToken {
                symbol: discovered_token.symbol.clone(),
                name: discovered_token.name.clone(),
                decimals: discovered_token.decimals,
                chain_addresses,
                coingecko_id: discovered_token.coingecko_id.clone(),
                token_type: match discovered_token.symbol.as_str() {
                    "ETH" | "MATIC" | "AVAX" | "BNB" => super::unified_token_interface::TokenType::Native,
                    s if s.starts_with("W") => super::unified_token_interface::TokenType::Wrapped,
                    "USDC" | "USDT" | "DAI" | "BUSD" => super::unified_token_interface::TokenType::Stable,
                    _ => super::unified_token_interface::TokenType::ERC20,
                },
                is_native: discovered_token.symbol == "ETH" || discovered_token.symbol == "MATIC" || 
                          discovered_token.symbol == "AVAX" || discovered_token.symbol == "BNB",
                logo_uri: discovered_token.logo_uri.clone(),
            };
            
            tokens_list.push(unified_token);
        }
    }

    // Apply search filter if provided
    let filtered_tokens: Vec<UnifiedToken> = tokens_list.into_iter()
        .filter(|token| {
            if let Some(ref search) = query.search {
                token.symbol.to_lowercase().contains(&search.to_lowercase()) ||
                token.name.to_lowercase().contains(&search.to_lowercase())
            } else {
                true
            }
        })
        .collect();

    info!("Returning {} discovered tokens", filtered_tokens.len());

    Ok(Json(TokensResponse {
        total_count: filtered_tokens.len(),
        tokens: filtered_tokens,
    }))
}

/// Get token details
async fn get_token_details(
    State(state): State<ChainAbstractionApiState>,
    Path(symbol): Path<String>,
) -> Result<Json<UnifiedToken>, StatusCode> {
    info!("Getting token details for: {}", symbol);

    match state.chain_abstractor.get_token_interface().get_token(&symbol).await {
        Some(token) => Ok(Json(token)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Get token price
async fn get_token_price(
    State(state): State<ChainAbstractionApiState>,
    Path(symbol): Path<String>,
) -> Result<Json<TokenPriceResponse>, StatusCode> {
    info!("Getting token price for: {}", symbol);

    match state.chain_abstractor.get_token_interface().get_token_price(&symbol).await {
        Ok(price) => {
            let formatted_price = format!("${:.6}", price.price_usd);
            Ok(Json(TokenPriceResponse {
                price,
                formatted_price,
            }))
        }
        Err(e) => {
            error!("Failed to get token price: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get cross-chain token mapping
async fn get_token_mapping(
    State(state): State<ChainAbstractionApiState>,
    Path(symbol): Path<String>,
) -> Result<Json<CrossChainMappingResponse>, StatusCode> {
    info!("Getting cross-chain mapping for: {}", symbol);

    match state.chain_abstractor.get_token_interface().get_cross_chain_mapping(&symbol).await {
        Some(mapping) => {
            let supported_chains: Vec<u64> = mapping.mappings.keys().cloned().collect();
            Ok(Json(CrossChainMappingResponse {
                mapping,
                supported_chains,
            }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Get token address for specific chain
async fn get_token_address(
    State(state): State<ChainAbstractionApiState>,
    Path((symbol, chain_id)): Path<(String, u64)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Getting token address for {} on chain {}", symbol, chain_id);

    match state.chain_abstractor.get_token_address(&symbol, chain_id).await {
        Some(address) => Ok(Json(serde_json::json!({
            "symbol": symbol,
            "chain_id": chain_id,
            "address": address
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Get supported chains
async fn get_supported_chains(
    State(_state): State<ChainAbstractionApiState>,
) -> Result<Json<ChainsResponse>, StatusCode> {
    info!("Getting supported chains");

    let chains = vec![
        ChainInfo {
            chain_id: 1,
            name: "Ethereum".to_string(),
            native_token: "ETH".to_string(),
            supported_tokens: vec!["ETH".to_string(), "WETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "WBTC".to_string()],
            is_testnet: false,
        },
        ChainInfo {
            chain_id: 137,
            name: "Polygon".to_string(),
            native_token: "MATIC".to_string(),
            supported_tokens: vec!["MATIC".to_string(), "WETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "WBTC".to_string()],
            is_testnet: false,
        },
        ChainInfo {
            chain_id: 10,
            name: "Optimism".to_string(),
            native_token: "ETH".to_string(),
            supported_tokens: vec!["ETH".to_string(), "WETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "WBTC".to_string()],
            is_testnet: false,
        },
        ChainInfo {
            chain_id: 42161,
            name: "Arbitrum".to_string(),
            native_token: "ETH".to_string(),
            supported_tokens: vec!["ETH".to_string(), "WETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "WBTC".to_string()],
            is_testnet: false,
        },
        ChainInfo {
            chain_id: 8453,
            name: "Base".to_string(),
            native_token: "ETH".to_string(),
            supported_tokens: vec!["ETH".to_string(), "WETH".to_string(), "USDC".to_string()],
            is_testnet: false,
        },
    ];

    Ok(Json(ChainsResponse {
        total_count: chains.len(),
        chains,
    }))
}

/// Get tokens supported on specific chain
async fn get_chain_tokens(
    State(state): State<ChainAbstractionApiState>,
    Path(chain_id): Path<u64>,
) -> Result<Json<TokensResponse>, StatusCode> {
    info!("Getting tokens for chain: {}", chain_id);

    let tokens = state.chain_abstractor.get_supported_tokens(chain_id).await;

    Ok(Json(TokensResponse {
        total_count: tokens.len(),
        tokens,
    }))
}

/// Get bridgeable tokens between chains
async fn get_bridgeable_tokens(
    State(state): State<ChainAbstractionApiState>,
    Query(query): Query<BridgeQuery>,
) -> Result<Json<BridgeableTokensResponse>, StatusCode> {
    info!("Getting bridgeable tokens between chains {} and {}", query.from_chain_id, query.to_chain_id);

    let tokens = state.chain_abstractor.get_bridgeable_tokens(query.from_chain_id, query.to_chain_id).await;
    let recommended_token = state.chain_abstractor.get_recommended_bridge_token(query.from_chain_id, query.to_chain_id).await;

    Ok(Json(BridgeableTokensResponse {
        tokens,
        recommended_token,
        from_chain_id: query.from_chain_id,
        to_chain_id: query.to_chain_id,
    }))
}

/// Get recommended bridge token
async fn get_recommended_bridge_token(
    State(state): State<ChainAbstractionApiState>,
    Query(query): Query<BridgeQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Getting recommended bridge token between chains {} and {}", query.from_chain_id, query.to_chain_id);

    let recommended_token = state.chain_abstractor.get_recommended_bridge_token(query.from_chain_id, query.to_chain_id).await;

    Ok(Json(serde_json::json!({
        "recommended_token": recommended_token,
        "from_chain_id": query.from_chain_id,
        "to_chain_id": query.to_chain_id
    })))
}

/// Format token amount
async fn format_token_amount(
    State(state): State<ChainAbstractionApiState>,
    Path((symbol, amount)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Formatting amount {} for token {}", amount, symbol);

    match state.chain_abstractor.format_token_amount(&symbol, &amount).await {
        Ok(formatted) => Ok(Json(serde_json::json!({
            "symbol": symbol,
            "raw_amount": amount,
            "formatted_amount": formatted
        }))),
        Err(e) => {
            error!("Failed to format token amount: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Parse formatted amount to raw amount
async fn parse_token_amount(
    State(state): State<ChainAbstractionApiState>,
    Path((symbol, formatted_amount)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Parsing formatted amount {} for token {}", formatted_amount, symbol);

    match state.chain_abstractor.get_token_interface().parse_token_amount(&symbol, &formatted_amount).await {
        Ok(raw_amount) => Ok(Json(serde_json::json!({
            "symbol": symbol,
            "formatted_amount": formatted_amount,
            "raw_amount": raw_amount
        }))),
        Err(e) => {
            error!("Failed to parse token amount: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "chain_abstraction_api",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let result = health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_supported_chains() {
        let state = ChainAbstractionApiState {
            chain_abstractor: Arc::new(ChainAbstractor::new()),
        };

        let result = get_supported_chains(State(state)).await;
        assert!(result.is_ok());

        let response = result.unwrap().0;
        assert!(response.total_count > 0);
        assert!(!response.chains.is_empty());
    }

    #[tokio::test]
    async fn test_estimate_quote() {
        let state = ChainAbstractionApiState {
            chain_abstractor: Arc::new(ChainAbstractor::new()),
        };

        let request = QuoteRequest {
            from_token: "ETH".to_string(),
            to_token: "USDC".to_string(),
            amount: "1000000000000000000".to_string(),
            from_chain_id: 1,
            to_chain_id: 1,
            user_address: "0x123".to_string(),
        };

        let result = estimate_quote(State(state), Json(request)).await;
        assert!(result.is_ok());
    }
}
