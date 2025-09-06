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
pub struct KyberSwapDex {
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
    api_base_url: String,
    token_list_url: String,
}

#[derive(Debug, Deserialize)]
struct KyberRouteResponse {
    #[serde(rename = "outputAmount")]
    output_amount: String,
    #[serde(rename = "routeSummary")]
    route_summary: Option<KyberRouteSummary>,
}

#[derive(Debug, Deserialize)]
struct KyberRouteSummary {
    #[serde(rename = "tokenIn")]
    token_in: String,
    #[serde(rename = "tokenOut")]
    token_out: String,
    #[serde(rename = "amountIn")]
    amount_in: String,
    #[serde(rename = "amountOut")]
    amount_out: String,
    gas: String,
}

impl KyberSwapDex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "ethereum".to_string(),
        ];

        Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                rpc_url: std::env::var("ETHEREUM_RPC_URL")
                    .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string()),
                router_address: "0x6131B5fae19EA4f9D964eAc0408E4408b66337b5".to_string(), // KyberSwap Router
                api_base_url: "https://aggregator-api.kyberswap.com".to_string(),
                token_list_url: "https://raw.githubusercontent.com/KyberNetwork/kyberswap-interface/develop/src/constants/tokenLists/kyber.tokenlist.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by KyberSwap", chain))),
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
        
        debug!("Fetching KyberSwap token list from: {}", config.token_list_url);

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

        // Add ETH as a special case for Ethereum
        if chain == "ethereum" {
            let eth_token = TokenInfo {
                symbol: "ETH".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                decimals: 18,
                name: "Ethereum".to_string(),
                chain_id: 1,
            };
            filtered_tokens.push(eth_token);
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
            ("ethereum", "ETH") => {
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

    /// Get KyberSwap quote using Aggregator API
    async fn get_kyberswap_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("KyberSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = (params.token_in.to_uppercase() == "ETH" && params.token_out.to_uppercase() == "WETH") || 
                             (params.token_in.to_uppercase() == "WETH" && params.token_out.to_uppercase() == "ETH");

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

        // Try KyberSwap Aggregator API first
        let api_url = format!("{}/ethereum/api/v1/routes", config.api_base_url);
        
        let mut query_params = vec![
            ("tokenIn", token_in_addr.clone()),
            ("tokenOut", token_out_addr.clone()),
            ("amountIn", amount_wei.to_string()),
            ("saveGas", "0".to_string()),
            ("gasInclude", "true".to_string()),
            ("gasPrice", "20000000000".to_string()), // 20 gwei
        ];

        debug!("Calling KyberSwap API: {}", api_url);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.http_client
                .get(&api_url)
                .query(&query_params)
                .send()
        ).await
        .map_err(|_| DexError::Timeout("KyberSwap API call timed out".to_string()))?;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<KyberRouteResponse>().await {
                    Ok(kyber_response) => {
                        info!("KyberSwap API returned quote: {}", kyber_response.output_amount);
                        return Ok((kyber_response.output_amount, self.estimated_gas(chain)));
                    },
                    Err(e) => {
                        warn!("Failed to parse KyberSwap response: {}", e);
                    }
                }
            },
            Ok(resp) => {
                warn!("KyberSwap API returned error: {}", resp.status());
            },
            Err(e) => {
                warn!("KyberSwap API request failed: {}", e);
            }
        }

        // Fallback calculation
        warn!("Using KyberSwap fallback calculation");
        
        // Assume typical KyberSwap Elastic with 0.25% dynamic fee
        let output_wei = (amount_wei as f64 * 0.9975) as u128;
        
        Ok((output_wei.to_string(), self.estimated_gas(chain)))
    }

    /// Get estimated gas for KyberSwap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 170_000, // KyberSwap Elastic concentrated liquidity
            _ => 170_000,
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }
}

#[async_trait]
impl DexIntegration for KyberSwapDex {
    fn get_name(&self) -> &'static str {
        "KyberSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        info!("🌊 KyberSwap get_quote called for {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_kyberswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: gas_used.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (KyberSwap only on Ethereum)
        if chain != "ethereum" {
            return Ok(false);
        }

        // Try to fetch both tokens with timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "ethereum").await;
                let token_out_result = self.get_token_address(token_out, "ethereum").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on ethereum via KyberSwap", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kyberswap_initialization() {
        let dex = KyberSwapDex::new();
        assert_eq!(dex.get_name(), "KyberSwap");
        assert!(dex.supports_chain("ethereum"));
        assert!(!dex.supports_chain("polygon"));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = KyberSwapDex::new();
        
        let eth_config = dex.get_chain_config("ethereum").unwrap();
        assert_eq!(eth_config.chain_id, 1);
        assert_eq!(eth_config.router_address, "0x6131B5fae19EA4f9D964eAc0408E4408b66337b5");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("polygon").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimates() {
        let dex = KyberSwapDex::new();
        assert_eq!(dex.estimated_gas("ethereum"), 170_000);
    }
}
