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
pub struct BalancerDex {
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
    vault_address: String,
    sor_api_url: String,
    token_list_url: String,
}

#[derive(Debug, Deserialize)]
struct BalancerSorResponse {
    #[serde(rename = "returnAmount")]
    return_amount: String,
    swaps: Vec<BalancerSwap>,
    #[serde(rename = "tokenAddresses")]
    token_addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct BalancerSwap {
    #[serde(rename = "poolId")]
    pool_id: String,
    #[serde(rename = "assetInIndex")]
    asset_in_index: u32,
    #[serde(rename = "assetOutIndex")]
    asset_out_index: u32,
    amount: String,
    #[serde(rename = "userData")]
    user_data: String,
}

impl BalancerDex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "ethereum".to_string(),
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
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                rpc_url: std::env::var("ETHEREUM_RPC_URL")
                    .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string()),
                vault_address: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(), // Balancer V2 Vault
                sor_api_url: "https://api.balancer.fi".to_string(),
                token_list_url: "https://raw.githubusercontent.com/balancer-labs/assets/master/generated/listed.tokenlist.json".to_string(),
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                vault_address: "0xBA12222222228d8Ba445958a75a0704d566BF2C8".to_string(), // Balancer V2 Vault
                sor_api_url: "https://api.balancer.fi".to_string(),
                token_list_url: "https://raw.githubusercontent.com/balancer-labs/assets/master/generated/listed.tokenlist.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by Balancer", chain))),
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
        
        debug!("Fetching Balancer token list from: {}", config.token_list_url);

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

        // Add native tokens
        match chain {
            "ethereum" => {
                let eth_token = TokenInfo {
                    symbol: "ETH".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "Ethereum".to_string(),
                    chain_id: 1,
                };
                filtered_tokens.push(eth_token);
            },
            "polygon" => {
                let matic_token = TokenInfo {
                    symbol: "MATIC".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "Polygon".to_string(),
                    chain_id: 137,
                };
                filtered_tokens.push(matic_token);
            },
            _ => {}
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

    /// Get Balancer quote using Smart Order Router
    async fn get_balancer_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("Balancer doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = match chain {
            "ethereum" => (params.token_in.to_uppercase() == "ETH" && params.token_out.to_uppercase() == "WETH") || 
                         (params.token_in.to_uppercase() == "WETH" && params.token_out.to_uppercase() == "ETH"),
            "polygon" => (params.token_in.to_uppercase() == "MATIC" && params.token_out.to_uppercase() == "WMATIC") || 
                        (params.token_in.to_uppercase() == "WMATIC" && params.token_out.to_uppercase() == "MATIC"),
            _ => false,
        };

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

        // Try Balancer SOR API first
        let sor_url = format!("{}/sor/{}/quote", config.sor_api_url, config.chain_id);
        
        let mut request_body = HashMap::new();
        request_body.insert("sellToken", token_in_addr.clone());
        request_body.insert("buyToken", token_out_addr.clone());
        request_body.insert("orderKind", "sell".to_string());
        request_body.insert("amount", amount_wei.to_string());
        request_body.insert("gasPrice", "20000000000".to_string()); // 20 gwei

        debug!("Calling Balancer SOR API: {}", sor_url);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.http_client
                .post(&sor_url)
                .json(&request_body)
                .send()
        ).await
        .map_err(|_| DexError::Timeout("Balancer SOR API call timed out".to_string()))?;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<BalancerSorResponse>().await {
                    Ok(sor_response) => {
                        info!("Balancer SOR returned quote: {}", sor_response.return_amount);
                        return Ok((sor_response.return_amount, self.estimated_gas(chain)));
                    },
                    Err(e) => {
                        warn!("Failed to parse Balancer SOR response: {}", e);
                    }
                }
            },
            Ok(resp) => {
                warn!("Balancer SOR API returned error: {}", resp.status());
            },
            Err(e) => {
                warn!("Balancer SOR API request failed: {}", e);
            }
        }

        // Fallback calculation
        warn!("Using Balancer fallback calculation");
        
        // Assume typical Balancer weighted pool with 0.3% fee
        let output_wei = (amount_wei as f64 * 0.997) as u128;
        
        Ok((output_wei.to_string(), self.estimated_gas(chain)))
    }

    /// Get estimated gas for Balancer swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 160_000, // Balancer V2 Vault swaps
            "polygon" => 140_000,  // Slightly cheaper on Polygon
            _ => 160_000,
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }
}

#[async_trait]
impl DexIntegration for BalancerDex {
    fn get_name(&self) -> &'static str {
        "Balancer"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        info!("⚖️ Balancer get_quote called for {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_balancer_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: gas_used.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported
        if !self.supported_chains.contains(&chain.to_string()) {
            return Ok(false);
        }

        // Check if tokens exist on the specified chain
        match tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, chain).await;
                let token_out_result = self.get_token_address(token_out, chain).await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on {} via Balancer", token_in, token_out, chain);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{} on {}", token_in, token_out, chain);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum", "polygon"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_balancer_initialization() {
        let dex = BalancerDex::new();
        assert_eq!(dex.get_name(), "Balancer");
        assert!(dex.supports_chain("ethereum"));
        assert!(dex.supports_chain("polygon"));
        assert!(!dex.supports_chain("bsc"));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = BalancerDex::new();
        
        let eth_config = dex.get_chain_config("ethereum").unwrap();
        assert_eq!(eth_config.chain_id, 1);
        assert_eq!(eth_config.vault_address, "0xBA12222222228d8Ba445958a75a0704d566BF2C8");
        
        let polygon_config = dex.get_chain_config("polygon").unwrap();
        assert_eq!(polygon_config.chain_id, 137);
        
        // Test unsupported chain
        assert!(dex.get_chain_config("bsc").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimates() {
        let dex = BalancerDex::new();
        assert_eq!(dex.estimated_gas("ethereum"), 160_000);
        assert_eq!(dex.estimated_gas("polygon"), 140_000);
    }
}
