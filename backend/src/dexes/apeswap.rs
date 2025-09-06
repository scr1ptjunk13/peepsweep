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
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use futures::future;

#[derive(Debug, Clone)]
pub struct ApeSwapDex {
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
    native_token: NativeTokenConfig,
}

#[derive(Debug)]
struct NativeTokenConfig {
    symbol: String,
    wrapped_address: String,
    decimals: u8,
}

impl ApeSwapDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "bsc".to_string(),
            "polygon".to_string(),
        ];

        Ok(Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "bsc" => Ok(ChainConfig {
                chain_id: 56,
                rpc_url: std::env::var("BSC_RPC_URL")
                    .unwrap_or_else(|_| "https://bsc-dataseed.binance.org".to_string()),
                router_address: "0xcF0feBd3f17CEf5b47b0cD257aCf6025c5BFf3b7".to_string(), // ApeSwap Router V2
                factory_address: "0x0841BD0B734E4F5853f0dD8d7Ea041c241fb0Da6".to_string(), // ApeSwap Factory
                token_list_url: "https://raw.githubusercontent.com/ApeSwapFinance/apeswap-token-lists/main/lists/apeswap.json".to_string(),
                native_token: NativeTokenConfig {
                    symbol: "BNB".to_string(),
                    wrapped_address: "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string(), // WBNB
                    decimals: 18,
                },
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                router_address: "0xC0788A3aD43d79aa53B09c2EaCc313A787d1d607".to_string(), // ApeSwap Router Polygon
                factory_address: "0xCf083Be4164828f00cAE704EC15a36D711491284".to_string(), // ApeSwap Factory Polygon
                token_list_url: "https://raw.githubusercontent.com/ApeSwapFinance/apeswap-token-lists/main/lists/apeswap.json".to_string(),
                native_token: NativeTokenConfig {
                    symbol: "MATIC".to_string(),
                    wrapped_address: "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270".to_string(), // WMATIC
                    decimals: 18,
                },
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by ApeSwap", chain))),
        }
    }

    /// Fetch token list for a specific chain with caching (follows Uniswap pattern)
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
        
        debug!("Fetching ApeSwap token list from: {}", config.token_list_url);

        let response = self.http_client
            .get(&config.token_list_url)
            .timeout(Duration::from_secs(10))
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

        // Add native token using proper chain-specific config
        let native_token = TokenInfo {
            symbol: config.native_token.symbol.clone(),
            address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), // Universal native token address
            decimals: config.native_token.decimals,
            name: config.native_token.symbol.clone(),
            chain_id: config.chain_id,
        };
        filtered_tokens.push(native_token);

        // Cache the result
        {
            let mut cache = self.token_cache.write().await;
            cache.insert(chain.to_string(), filtered_tokens.clone());
        }

        debug!("Found {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol with proper native token handling
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let config = self.get_chain_config(chain)?;
        let tokens = self.fetch_token_list(chain).await?;
        
        let symbol_upper = symbol.to_uppercase();
        
        // Handle native tokens with chain-specific logic
        if symbol_upper == config.native_token.symbol {
            return Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), config.native_token.decimals));
        }
        
        // Handle wrapped tokens -> native conversion
        let normalized_symbol = match (chain, symbol_upper.as_str()) {
            ("bsc", "BNB") => "BNB",
            ("bsc", "WBNB") => "WBNB", 
            ("polygon", "MATIC") => "MATIC",
            ("polygon", "WMATIC") => "WMATIC",
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

    /// Convert user-friendly amount to wei/smallest unit (follows Uniswap pattern)
    fn convert_to_wei(&self, amount: &str, decimals: u8) -> Result<U256, DexError> {
        let amount_f64: f64 = amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount)))?;

        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".to_string()));
        }

        let multiplier = 10_u128.pow(decimals as u32);
        let wei_amount = (amount_f64 * multiplier as f64) as u128;
        
        Ok(U256::from(wei_amount))
    }

    /// Get ApeSwap quote using Router's getAmountsOut function
    async fn get_apeswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("ApeSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = match chain {
            "bsc" => (params.token_in.to_uppercase() == "BNB" && params.token_out.to_uppercase() == "WBNB") || 
                     (params.token_in.to_uppercase() == "WBNB" && params.token_out.to_uppercase() == "BNB"),
            "polygon" => (params.token_in.to_uppercase() == "MATIC" && params.token_out.to_uppercase() == "WMATIC") || 
                        (params.token_in.to_uppercase() == "WMATIC" && params.token_out.to_uppercase() == "MATIC"),
            _ => false,
        };

        if is_native_wrap {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok(wei_amount.to_string());
        }

        // Convert amount to wei
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Create provider
        let provider = ProviderBuilder::new()
            .on_http(config.rpc_url.parse()
                .map_err(|e| DexError::ParseError(format!("Invalid RPC URL: {}", e)))?);

        let router_address = Address::from_str(&config.router_address)
            .map_err(|e| DexError::ParseError(format!("Invalid router address: {}", e)))?;

        // Convert addresses for contract calls (native -> wrapped)
        let token_in_address = if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            Address::from_str(&config.native_token.wrapped_address)
                .map_err(|e| DexError::ParseError(format!("Invalid wrapped token address: {}", e)))?
        } else {
            Address::from_str(&token_in_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_in address: {}", e)))?
        };

        let token_out_address = if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            Address::from_str(&config.native_token.wrapped_address)
                .map_err(|e| DexError::ParseError(format!("Invalid wrapped token address: {}", e)))?
        } else {
            Address::from_str(&token_out_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_out address: {}", e)))?
        };

        // Try multiple paths concurrently (like Uniswap's multi-fee approach)
        let paths = vec![
            vec![token_in_address, token_out_address], // Direct path
            // Could add intermediate tokens like WBNB/WMATIC for better routing
        ];

        let mut quote_futures = Vec::new();
        for path in paths {
            let provider_clone = provider.clone();
            let fut = self.try_quote_with_path_timeout(
                provider_clone,
                router_address,
                amount_in_wei,
                path,
            );
            quote_futures.push(fut);
        }

        // Execute all paths concurrently with timeout
        let results = tokio::time::timeout(
            Duration::from_secs(10),
            future::join_all(quote_futures)
        ).await.map_err(|_| DexError::Timeout("All quote attempts timed out".to_string()))?;

        // Find the best quote
        let mut best_quote = U256::ZERO;
        let mut successful_quote = false;

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(quote) => {
                    debug!("Quote successful for path {}: {}", i, quote);
                    if quote > best_quote {
                        best_quote = quote;
                        successful_quote = true;
                    }
                }
                Err(e) => {
                    debug!("Quote failed for path {}: {}", i, e);
                }
            }
        }

        if successful_quote {
            info!("✅ ApeSwap quote: {} {} -> {} {} on {}", 
                  params.amount_in, params.token_in, best_quote, params.token_out, chain);
            Ok(best_quote.to_string())
        } else {
            Err(DexError::InvalidResponse("No liquidity found for this pair on any path".to_string()))
        }
    }

    async fn try_quote_with_path_timeout(
        &self,
        provider: RootProvider<Http<Client>>,
        router_address: Address,
        amount_in: U256,
        path: Vec<Address>,
    ) -> Result<U256, DexError> {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.try_quote_with_path(&provider, router_address, amount_in, path)
        ).await
        .map_err(|_| DexError::Timeout("Quote timeout for path".to_string()))?
    }

    async fn try_quote_with_path(
        &self,
        provider: &RootProvider<Http<Client>>,
        router_address: Address,
        amount_in: U256,
        path: Vec<Address>,
    ) -> Result<U256, DexError> {
        if path.len() < 2 {
            return Err(DexError::InvalidResponse("Path must have at least 2 tokens".to_string()));
        }

        // Encode getAmountsOut call
        // Function signature: getAmountsOut(uint256 amountIn, address[] calldata path)
        let function_selector = "0xd06ca61f";
        
        // Encode dynamic array for path
        let path_offset = "0000000000000000000000000000000000000000000000000000000000000040"; // offset to path array
        let path_length = format!("{:0>64x}", path.len());
        let path_data: String = path.iter()
            .map(|addr| format!("{:0>64}", hex::encode(addr.as_slice())))
            .collect();

        let call_data = format!(
            "{}{}{}{}{}",
            function_selector,
            format!("{:0>64x}", amount_in), // amountIn
            path_offset,                    // offset to path array
            path_length,                   // path length
            path_data                      // path addresses
        );
        
        let call_request = alloy::rpc::types::eth::TransactionRequest {
            to: Some(router_address.into()),
            input: alloy::rpc::types::eth::TransactionInput::new(
                hex::decode(&call_data[2..])
                    .map_err(|e| DexError::ParseError(format!("Invalid call data: {}", e)))?
                    .into()
            ),
            ..Default::default()
        };
        
        let call_result = provider.call(&call_request).await
            .map_err(|e| DexError::ContractError(format!("Router call failed: {}", e)))?;
        
        // Parse getAmountsOut response - returns array of amounts
        // We want the last amount (output amount)
        if call_result.len() >= 64 {
            // Skip first 32 bytes (array offset), next 32 bytes (array length)
            // Then we have the amounts - we want the last one
            let amounts_start = 64;
            let num_amounts = path.len();
            let last_amount_offset = amounts_start + (num_amounts - 1) * 32;
            
            if call_result.len() >= last_amount_offset + 32 {
                let amount_bytes = &call_result[last_amount_offset..last_amount_offset + 32];
                Ok(U256::from_be_slice(amount_bytes))
            } else {
                Err(DexError::InvalidResponse("Invalid getAmountsOut response length".to_string()))
            }
        } else {
            Err(DexError::InvalidResponse("Invalid router response length".to_string()))
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for ApeSwap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "bsc" => 115_000,
            "polygon" => 125_000,
            _ => 120_000,
        }
    }
}

#[async_trait]
impl DexIntegration for ApeSwapDex {
    fn get_name(&self) -> &'static str {
        "ApeSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_apeswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("bsc")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported
        if !self.supported_chains.contains(&chain.to_string()) {
            return Ok(false);
        }

        // Try to fetch both tokens with timeout
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
                debug!("Pair {}/{} not supported on {} via ApeSwap", token_in, token_out, chain);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["bsc", "polygon"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_apeswap_initialization() {
        let apeswap = ApeSwapDex::new().await;
        assert!(apeswap.is_ok());
        
        let dex = apeswap.unwrap();
        assert_eq!(dex.get_name(), "ApeSwap");
        assert!(dex.supports_chain("bsc"));
        assert!(dex.supports_chain("polygon"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_native_token_handling() {
        let dex = ApeSwapDex::new().await.unwrap();
        
        // Test BSC native token
        let bnb_result = dex.get_token_address("BNB", "bsc").await;
        println!("BNB lookup: {:?}", bnb_result);
        
        // Test Polygon native token  
        let matic_result = dex.get_token_address("MATIC", "polygon").await;
        println!("MATIC lookup: {:?}", matic_result);
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = ApeSwapDex::new().await.unwrap();
        
        // Test BNB conversion (18 decimals)
        let bnb_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(bnb_wei, U256::from(1_000_000_000_000_000_000_u128));
        
        // Test USDC conversion (6 decimals) 
        let usdc_wei = dex.convert_to_wei("100.0", 6).unwrap();
        assert_eq!(usdc_wei, U256::from(100_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = ApeSwapDex::new().await.unwrap();
        
        let bsc_config = dex.get_chain_config("bsc").unwrap();
        assert_eq!(bsc_config.chain_id, 56);
        assert_eq!(bsc_config.native_token.symbol, "BNB");
        assert_eq!(bsc_config.native_token.wrapped_address, "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c");
        
        let polygon_config = dex.get_chain_config("polygon").unwrap();
        assert_eq!(polygon_config.chain_id, 137);
        assert_eq!(polygon_config.native_token.symbol, "MATIC");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test] 
    async fn test_native_wrap_conversion() {
        let dex = ApeSwapDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "BNB".to_string(),
            token_out: "WBNB".to_string(),
            amount_in: "1.0".to_string(),
            chain: Some("bsc".to_string()),
        };
        
        let quote = dex.get_apeswap_quote(&params).await;
        println!("BNB->WBNB quote: {:?}", quote);
    }
}