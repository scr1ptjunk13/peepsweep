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
pub struct AerodromeDex {
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

impl AerodromeDex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "base".to_string(),
        ];

        Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "base" => Ok(ChainConfig {
                chain_id: 8453,
                rpc_url: std::env::var("BASE_RPC_URL")
                    .unwrap_or_else(|_| "https://mainnet.base.org".to_string()),
                router_address: "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43".to_string(), // Aerodrome Router
                factory_address: "0x420DD381b31aEf6683db6B902084cB0FFECe40Da".to_string(), // Aerodrome Factory
                token_list_url: "https://raw.githubusercontent.com/aerodrome-finance/token-list/main/src/tokens/base.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by Aerodrome", chain))),
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
        
        debug!("Fetching Aerodrome token list from: {}", config.token_list_url);

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

        // Add ETH as a special case for Base
        if chain == "base" {
            let eth_token = TokenInfo {
                symbol: "ETH".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                decimals: 18,
                name: "Ethereum".to_string(),
                chain_id: 8453,
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
            ("base", "ETH") => {
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

    /// Get Aerodrome quote using getAmountsOut
    async fn get_aerodrome_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("base");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("Aerodrome doesn't support chain: {}", chain)));
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

        // TODO: Implement actual Aerodrome Router getAmountsOut call
        // For now, return a placeholder quote
        warn!("Aerodrome quote implementation pending - returning placeholder");
        
        let amount_f64: f64 = params.amount_in.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", params.amount_in)))?;
        let multiplier = 10_u128.pow(token_in_decimals as u32);
        let input_wei = (amount_f64 * multiplier as f64) as u128;
        
        // Placeholder: assume 0.05% fee (Aerodrome has very low fees)
        let output_wei = (input_wei as f64 * 0.9995) as u128;
        
        Ok((output_wei.to_string(), self.estimated_gas(chain)))
    }

    /// Get estimated gas for Aerodrome swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "base" => 135_000, // Base L2 gas costs
            _ => 135_000,
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }
}

#[async_trait]
impl DexIntegration for AerodromeDex {
    fn get_name(&self) -> &'static str {
        "Aerodrome"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("base");
        info!("✈️ Aerodrome get_quote called for {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_aerodrome_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: gas_used.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (Aerodrome only on Base)
        if chain != "base" {
            return Ok(false);
        }

        // Check if tokens exist on Base
        match tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "base").await;
                let token_out_result = self.get_token_address(token_out, "base").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on base via Aerodrome", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["base"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aerodrome_initialization() {
        let dex = AerodromeDex::new();
        assert_eq!(dex.get_name(), "Aerodrome");
        assert!(dex.supports_chain("base"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = AerodromeDex::new();
        
        let base_config = dex.get_chain_config("base").unwrap();
        assert_eq!(base_config.chain_id, 8453);
        assert_eq!(base_config.router_address, "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimates() {
        let dex = AerodromeDex::new();
        assert_eq!(dex.estimated_gas("base"), 135_000);
    }
}
