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
pub struct PancakeSwapV2Dex {
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
    factory_address: String,
    token_list_url: String,
}

impl PancakeSwapV2Dex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "bsc".to_string(),
        ];

        Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "bsc" => Ok(ChainConfig {
                chain_id: 56,
                rpc_url: std::env::var("BSC_RPC_URL")
                    .unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string()),
                router_address: "0x10ED43C718714eb63d5aA57B78B54704E256024E".to_string(), // PancakeSwap V2 Router
                factory_address: "0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73".to_string(), // PancakeSwap V2 Factory
                token_list_url: "https://tokens.pancakeswap.finance/pancakeswap-extended.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by PancakeSwap V2", chain))),
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
        
        debug!("Fetching PancakeSwap V2 token list from: {}", config.token_list_url);

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

        // Add BNB as a special case for BSC
        if chain == "bsc" {
            let bnb_token = TokenInfo {
                symbol: "BNB".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                decimals: 18,
                name: "BNB".to_string(),
                chain_id: 56,
            };
            filtered_tokens.push(bnb_token);
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
            ("bsc", "BNB") => {
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

    /// Get PancakeSwap V2 quote using getAmountsOut
    async fn get_pancakeswap_v2_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("PancakeSwap V2 doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = (params.token_in.to_uppercase() == "BNB" && params.token_out.to_uppercase() == "WBNB") || 
                             (params.token_in.to_uppercase() == "WBNB" && params.token_out.to_uppercase() == "BNB");

        if is_native_wrap {
            let amount_f64: f64 = params.amount_in.parse()
                .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", params.amount_in)))?;
            let multiplier = 10_u128.pow(token_in_decimals as u32);
            let wei_amount = (amount_f64 * multiplier as f64) as u128;
            return Ok((wei_amount.to_string(), self.estimated_gas(chain)));
        }

        // TODO: Implement actual PancakeSwap V2 Router getAmountsOut call
        // For now, return a placeholder quote
        warn!("PancakeSwap V2 quote implementation pending - returning placeholder");
        
        let amount_f64: f64 = params.amount_in.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", params.amount_in)))?;
        let multiplier = 10_u128.pow(token_in_decimals as u32);
        let input_wei = (amount_f64 * multiplier as f64) as u128;
        
        // Placeholder: assume 0.25% fee and some slippage
        let output_wei = (input_wei as f64 * 0.9975) as u128;
        
        Ok((output_wei.to_string(), self.estimated_gas(chain)))
    }

    /// Get estimated gas for PancakeSwap V2 swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "bsc" => 120_000, // BSC is cheaper than Ethereum
            _ => 120_000,
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }
}

#[async_trait]
impl DexIntegration for PancakeSwapV2Dex {
    fn get_name(&self) -> &'static str {
        "PancakeSwap V2"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        info!("🥞 PancakeSwap V2 get_quote called for {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_pancakeswap_v2_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: gas_used.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (PancakeSwap V2 only on BSC)
        if chain != "bsc" {
            return Ok(false);
        }

        // Check if tokens exist on BSC
        match tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "bsc").await;
                let token_out_result = self.get_token_address(token_out, "bsc").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on bsc via PancakeSwap V2", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["bsc"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pancakeswap_v2_initialization() {
        let dex = PancakeSwapV2Dex::new();
        assert_eq!(dex.get_name(), "PancakeSwap V2");
        assert!(dex.supports_chain("bsc"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = PancakeSwapV2Dex::new();
        
        let bsc_config = dex.get_chain_config("bsc").unwrap();
        assert_eq!(bsc_config.chain_id, 56);
        assert_eq!(bsc_config.router_address, "0x10ED43C718714eb63d5aA57B78B54704E256024E");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimates() {
        let dex = PancakeSwapV2Dex::new();
        assert_eq!(dex.estimated_gas("bsc"), 120_000);
    }
}
