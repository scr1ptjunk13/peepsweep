use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::http::{Client, Http},
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument, error, debug, warn};
use futures::future;

#[derive(Debug, Clone)]
pub struct UniswapDex {
    http_client: HttpClient,
    supported_chains: Vec<String>,
    // Cache token lists to avoid repeated API calls
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
    quoter_address: String,
    token_list_url: String,
}

impl UniswapDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(15)) // Reduced timeout
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "ethereum".to_string(),
            "polygon".to_string(),
            "arbitrum".to_string(),
            "optimism".to_string(),
            "base".to_string(),
        ];

        Ok(Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                rpc_url: std::env::var("ETHEREUM_RPC_URL")
                    .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string()),
                quoter_address: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
                token_list_url: "https://gateway.ipfs.io/ipns/tokens.uniswap.org".to_string(),
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                quoter_address: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
                token_list_url: "https://unpkg.com/quickswap-default-token-list@1.2.28/build/quickswap-default.tokenlist.json".to_string(),
            }),
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                rpc_url: std::env::var("ARBITRUM_RPC_URL")
                    .unwrap_or_else(|_| "https://arbitrum.llamarpc.com".to_string()),
                quoter_address: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
                token_list_url: "https://bridge.arbitrum.io/token-list-42161.json".to_string(),
            }),
            "optimism" => Ok(ChainConfig {
                chain_id: 10,
                rpc_url: std::env::var("OPTIMISM_RPC_URL")
                    .unwrap_or_else(|_| "https://optimism.llamarpc.com".to_string()),
                quoter_address: "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6".to_string(),
                token_list_url: "https://static.optimism.io/optimism.tokenlist.json".to_string(),
            }),
            "base" => Ok(ChainConfig {
                chain_id: 8453,
                rpc_url: std::env::var("BASE_RPC_URL")
                    .unwrap_or_else(|_| "https://base.llamarpc.com".to_string()),
                quoter_address: "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a".to_string(),
                token_list_url: "https://raw.githubusercontent.com/base-org/brand-kit/main/base-default-token-list.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by Uniswap", chain))),
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
        
        debug!("Fetching Uniswap token list from: {}", config.token_list_url);

        let response = self.http_client
            .get(&config.token_list_url)
            .timeout(std::time::Duration::from_secs(10)) // Explicit timeout
            .send()
            .await
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

        // ADD ETH as a special case for Ethereum - this fixes the ETH lookup issue
        if chain == "ethereum" {
            let eth_token = TokenInfo {
                symbol: "ETH".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), // Special ETH address
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

        debug!("Found {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol on a specific chain with ETH/WETH normalization
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        
        let symbol_upper = symbol.to_uppercase();
        let normalized_symbol = match symbol_upper.as_str() {
            "ETH" => {
                // For ETH, first try to find ETH, then fallback to WETH
                if let Some(token) = tokens.iter().find(|t| t.symbol.to_uppercase() == "ETH") {
                    return Ok((token.address.clone(), token.decimals));
                } else {
                    "WETH" // Fallback to WETH if ETH not found
                }
            }
            other => other,
        };
        
        for token in tokens {
            if token.symbol.to_uppercase() == normalized_symbol {
                return Ok((token.address, token.decimals));
            }
        }

        Err(DexError::UnsupportedPair(format!("Token {} not found on {}", symbol, chain)))
    }

    /// Convert user-friendly amount to wei/smallest unit
    fn convert_to_wei(&self, amount: &str, decimals: u8) -> Result<U256, DexError> {
        let amount_f64: f64 = amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount)))?;

        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".to_string()));
        }

        // Convert to smallest unit
        let multiplier = 10_u128.pow(decimals as u32);
        let wei_amount = (amount_f64 * multiplier as f64) as u128;
        
        Ok(U256::from(wei_amount))
    }

    /// Get a quote from Uniswap V3 using the Quoter contract with improved error handling
    async fn get_uniswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        // Validate chain support
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("Uniswap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses with proper ETH/WETH handling
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle ETH/WETH 1:1 conversion edge case
        if (params.token_in.to_uppercase() == "ETH" && params.token_out.to_uppercase() == "WETH") || 
           (params.token_in.to_uppercase() == "WETH" && params.token_out.to_uppercase() == "ETH") {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok(wei_amount.to_string());
        }

        // Convert amount to wei
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Create provider with timeout
        let provider = ProviderBuilder::new()
            .on_http(config.rpc_url.parse()
                .map_err(|e| DexError::ParseError(format!("Invalid RPC URL: {}", e)))?);

        let quoter_address = Address::from_str(&config.quoter_address)
            .map_err(|e| DexError::ParseError(format!("Invalid quoter address: {}", e)))?;

        // Handle ETH address conversion for contracts
        let token_in_address = if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert ETH to WETH for Uniswap contracts
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .map_err(|e| DexError::ParseError(format!("Invalid WETH address: {}", e)))?
        } else {
            Address::from_str(&token_in_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_in address: {}", e)))?
        };

        let token_out_address = if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert ETH to WETH for Uniswap contracts
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                .map_err(|e| DexError::ParseError(format!("Invalid WETH address: {}", e)))?
        } else {
            Address::from_str(&token_out_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_out address: {}", e)))?
        };

        // Try different fee tiers concurrently for speed
        let fee_tiers = [500u32, 3000u32, 10000u32];
        let mut quote_futures = Vec::new();

        for fee in fee_tiers {
            let provider_clone = provider.clone();
            let fut = self.try_quote_with_fee_timeout(
                provider_clone,
                quoter_address,
                token_in_address,
                token_out_address,
                amount_in_wei,
                fee,
            );
            quote_futures.push(fut);
        }

        // Wait for all quotes with a reasonable timeout
        let results = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            future::join_all(quote_futures)
        ).await.map_err(|_| DexError::Timeout("All quote attempts timed out".to_string()))?;

        // Find the best quote
        let mut best_quote = U256::ZERO;
        let mut successful_quote = false;

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(quote) => {
                    debug!("Quote successful for fee tier {}: {}", fee_tiers[i], quote);
                    if quote > best_quote {
                        best_quote = quote;
                        successful_quote = true;
                    }
                }
                Err(e) => {
                    debug!("Quote failed for fee tier {}: {}", fee_tiers[i], e);
                }
            }
        }

        if successful_quote {
            info!("✅ Uniswap quote: {} {} -> {} {} on {}", 
                  params.amount_in, params.token_in, best_quote, params.token_out, chain);
            Ok(best_quote.to_string())
        } else {
            Err(DexError::InvalidResponse("No liquidity found for this pair on any fee tier".to_string()))
        }
    }

    async fn try_quote_with_fee_timeout(
        &self,
        provider: RootProvider<Http<Client>>,
        quoter_address: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee: u32,
    ) -> Result<U256, DexError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(5), // Individual quote timeout
            self.try_quote_with_fee(&provider, quoter_address, token_in, token_out, amount_in, fee)
        ).await
        .map_err(|_| DexError::Timeout(format!("Quote timeout for fee tier {}", fee)))?
    }

    async fn try_quote_with_fee(
        &self,
        provider: &RootProvider<Http<Client>>,
        quoter_address: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee: u32,
    ) -> Result<U256, DexError> {
        // Encode the quoteExactInputSingle call
        // Function signature: quoteExactInputSingle(address,address,uint24,uint256,uint160)
        let function_selector = "0xf7729d43";
        
        let call_data = format!(
            "{}{}{}{}{}{}",
            function_selector,
            format!("{:0>64}", hex::encode(token_in.as_slice())),
            format!("{:0>64}", hex::encode(token_out.as_slice())),
            format!("{:0>64}", format!("{:x}", fee)),
            format!("{:0>64}", format!("{:x}", amount_in)),
            format!("{:0>64}", "0") // sqrtPriceLimitX96 = 0
        );
        
        let call_request = alloy::rpc::types::eth::TransactionRequest {
            to: Some(quoter_address.into()),
            input: alloy::rpc::types::eth::TransactionInput::new(
                hex::decode(&call_data[2..])
                    .map_err(|e| DexError::ParseError(format!("Invalid call data: {}", e)))?
                    .into()
            ),
            ..Default::default()
        };
        
        let call_result = provider.call(&call_request).await
            .map_err(|e| DexError::ContractError(format!("Quoter call failed for fee {}: {}", fee, e)))?;
        
        // Parse the returned bytes as U256
        if call_result.len() >= 32 {
            let amount_bytes = &call_result[call_result.len()-32..];
            Ok(U256::from_be_slice(amount_bytes))
        } else {
            Err(DexError::InvalidResponse("Invalid quoter response length".to_string()))
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for Uniswap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 180_000,
            "polygon" | "arbitrum" | "optimism" | "base" => 120_000,
            _ => 150_000,
        }
    }
}

#[async_trait]
impl DexIntegration for UniswapDex {
    fn get_name(&self) -> &'static str {
        "Uniswap V3"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_uniswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("ethereum")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (Uniswap only on Ethereum for now)
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
                debug!("Pair {}/{} not supported on ethereum via Uniswap", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum", "polygon", "arbitrum", "optimism", "base"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_uniswap_initialization() {
        let uniswap = UniswapDex::new().await;
        assert!(uniswap.is_ok());
        
        let dex = uniswap.unwrap();
        assert_eq!(dex.get_name(), "Uniswap V3");
        assert!(dex.supports_chain("ethereum"));
        assert!(dex.supports_chain("polygon"));
        assert!(!dex.supports_chain("solana"));
    }

    #[tokio::test]
    async fn test_eth_weth_handling() {
        let dex = UniswapDex::new().await.unwrap();
        
        // Test that both ETH and WETH are found
        let eth_result = dex.get_token_address("ETH", "ethereum").await;
        let weth_result = dex.get_token_address("WETH", "ethereum").await;
        
        // Both should succeed (assuming network is available)
        // In unit tests, you might want to mock this
        println!("ETH lookup: {:?}", eth_result);
        println!("WETH lookup: {:?}", weth_result);
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = UniswapDex::new().await.unwrap();
        
        // Test ETH conversion (18 decimals)
        let eth_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(eth_wei, U256::from(1_000_000_000_000_000_000_u128));
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("100.0", 6).unwrap();
        assert_eq!(usdc_wei, U256::from(100_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = UniswapDex::new().await.unwrap();
        
        let eth_config = dex.get_chain_config("ethereum").unwrap();
        assert_eq!(eth_config.chain_id, 1);
        assert_eq!(eth_config.quoter_address, "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6");
        
        let polygon_config = dex.get_chain_config("polygon").unwrap();
        assert_eq!(polygon_config.chain_id, 137);
        
        // Test unsupported chain
        assert!(dex.get_chain_config("solana").is_err());
    }
}