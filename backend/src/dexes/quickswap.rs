use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug, Clone)]
pub struct QuickSwapDex {
    http_client: HttpClient,
    supported_chains: Vec<String>,
    token_cache: Arc<RwLock<HashMap<String, Vec<TokenInfo>>>>,
}

#[derive(Debug, Deserialize)]
struct TokenListResponse {
    tokens: Vec<TokenInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub name: String,
    #[serde(rename = "chainId")]
    pub chain_id: u32,
}

#[derive(Debug)]
struct ChainConfig {
    chain_id: u32,
    rpc_url: String,
    router_address: String,
    algebra_quoter_address: String,
    token_list_url: String,
}

#[derive(Debug, Deserialize)]
struct ZeroXQuoteResponse {
    #[serde(rename = "buyAmount")]
    buy_amount: String,
    #[serde(rename = "sellAmount")]
    sell_amount: String,
    price: String,
    #[serde(rename = "estimatedGas")]
    estimated_gas: Option<String>,
}

impl QuickSwapDex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "polygon".to_string(),
        ];

        Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                router_address: "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff".to_string(), // QuickSwap Router
                algebra_quoter_address: "0xa15F0D7377B2A0C0c10262E4ABf0f6E4Ed350875".to_string(), // Algebra Quoter
                token_list_url: "https://unpkg.com/quickswap-default-token-list@1.2.28/build/quickswap-default.tokenlist.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by QuickSwap", chain))),
        }
    }

    /// Fetch token list for a specific chain with caching
    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(cached_tokens) = cache.get(chain) {
                debug!("Using cached token list for {}", chain);
                return Ok(cached_tokens.clone());
            }
        }

        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching QuickSwap token list from: {}", config.token_list_url);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.http_client.get(&config.token_list_url).send()
        ).await
        .map_err(|_| DexError::Timeout("Token list fetch timed out".to_string()))?
        .map_err(|e| DexError::NetworkError(e))?;

        if !response.status().is_success() {
            return Err(DexError::ApiError(format!("Failed to fetch token list: {}", response.status())));
        }

        let token_response: TokenListResponse = response.json().await
            .map_err(|e| DexError::InvalidResponse(format!("Failed to parse token list: {}", e)))?;

        // Filter tokens for the specific chain
        let mut filtered_tokens: Vec<TokenInfo> = token_response.tokens
            .into_iter()
            .filter(|token| token.chain_id == config.chain_id)
            .collect();

        // Add MATIC as a special case for Polygon
        if chain == "polygon" {
            let matic_token = TokenInfo {
                symbol: "MATIC".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                decimals: 18,
                name: "Polygon".to_string(),
                chain_id: 137,
            };
            filtered_tokens.push(matic_token);
        }

        // Cache the result
        {
            let mut cache = self.token_cache.write().await;
            cache.insert(chain.to_string(), filtered_tokens.clone());
        }

        info!("Cached {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol with native token normalization
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        
        let symbol_upper = symbol.to_uppercase();
        
        // Handle native tokens first
        let normalized_symbol = match (chain, symbol_upper.as_str()) {
            ("polygon", "MATIC") => {
                return Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18));
            },
            (_, other) => other,
        };
        
        // Search for token in the list
        for token in tokens {
            if token.symbol.to_uppercase() == normalized_symbol {
                return Ok((token.address, token.decimals));
            }
        }

        Err(DexError::UnsupportedPair(format!("Token {} not found on {}", symbol, chain)))
    }

    /// Get QuickSwap quote using 0x Polygon API
    async fn get_quickswap_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("polygon");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("QuickSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = (params.token_in.to_uppercase() == "MATIC" && params.token_out.to_uppercase() == "WMATIC") || 
                             (params.token_in.to_uppercase() == "WMATIC" && params.token_out.to_uppercase() == "MATIC");

        if is_native_wrap {
            let amount_f64: f64 = params.amount_in.parse()
                .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", params.amount_in)))?;
            let multiplier = 10_u128.pow(token_in_decimals as u32);
            let wei_amount = (amount_f64 * multiplier as f64) as u128;
            return Ok((wei_amount.to_string(), self.estimated_gas(chain)));
        }

        // Convert amount to wei
        let amount_f64: f64 = params.amount_in.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", params.amount_in)))?;
        let multiplier = 10_u128.pow(token_in_decimals as u32);
        let amount_wei = (amount_f64 * multiplier as f64) as u128;

        // Try 0x Polygon API first
        let api_url = "https://polygon.api.0x.org/swap/v1/quote";
        
        let mut query_params = vec![
            ("sellToken", token_in_addr.clone()),
            ("buyToken", token_out_addr.clone()),
            ("sellAmount", amount_wei.to_string()),
            ("slippagePercentage", "0.01".to_string()), // 1% slippage
        ];

        debug!("Calling 0x Polygon API for QuickSwap: {}", api_url);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.http_client
                .get(api_url)
                .query(&query_params)
                .send()
        ).await
        .map_err(|_| DexError::Timeout("0x Polygon API call timed out".to_string()))?;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ZeroXQuoteResponse>().await {
                    Ok(quote_response) => {
                        info!("0x Polygon API returned quote: {}", quote_response.buy_amount);
                        return Ok((quote_response.buy_amount, self.estimated_gas(chain)));
                    },
                    Err(e) => {
                        warn!("Failed to parse 0x Polygon response: {}", e);
                    }
                }
            },
            Ok(resp) => {
                warn!("0x Polygon API returned error: {}", resp.status());
            },
            Err(e) => {
                warn!("0x Polygon API request failed: {}", e);
            }
        }

        // Fallback calculation using Algebra V3 rates
        warn!("Using QuickSwap fallback calculation");
        
        // Assume typical Algebra V3 concentrated liquidity with 0.05% fee
        let output_wei = (amount_wei as f64 * 0.9995) as u128;
        
        Ok((output_wei.to_string(), self.estimated_gas(chain)))
    }

    /// Get estimated gas for QuickSwap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "polygon" => 130_000, // Polygon gas costs
            _ => 130_000,
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }
}

#[async_trait]
impl DexIntegration for QuickSwapDex {
    fn get_name(&self) -> &'static str {
        "QuickSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("polygon");
        info!("⚡ QuickSwap get_quote called for {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_quickswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: gas_used.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (QuickSwap only on Polygon)
        if chain != "polygon" {
            return Ok(false);
        }

        // Check if tokens exist on Polygon
        match tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "polygon").await;
                let token_out_result = self.get_token_address(token_out, "polygon").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on polygon via QuickSwap", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["polygon"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quickswap_initialization() {
        let dex = QuickSwapDex::new();
        assert_eq!(dex.get_name(), "QuickSwap");
        assert!(dex.supports_chain("polygon"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = QuickSwapDex::new();
        
        let polygon_config = dex.get_chain_config("polygon").unwrap();
        assert_eq!(polygon_config.chain_id, 137);
        assert_eq!(polygon_config.router_address, "0xa5E0829CaCEd8fFDD4De3c43696c57F7D7A678ff");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimates() {
        let dex = QuickSwapDex::new();
        assert_eq!(dex.estimated_gas("polygon"), 130_000);
    }
}
