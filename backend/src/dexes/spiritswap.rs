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
pub struct SpiritSwapDex {
    http_client: HttpClient,
    supported_chains: Vec<String>,
    // Cache token lists to avoid repeated API calls - following Uniswap pattern
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

impl SpiritSwapDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "fantom".to_string(),
        ];

        Ok(Self {
            http_client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "fantom" => Ok(ChainConfig {
                chain_id: 250,
                rpc_url: std::env::var("FANTOM_RPC_URL")
                    .unwrap_or_else(|_| "https://rpc.ftm.tools".to_string()),
                router_address: "0x16327E3FbDaCA3bcF7E38F5Af2599D2DDc33aE52".to_string(), // SpiritSwap Router
                factory_address: "0xEF45d134b73241eDa7703fa787148D9C9F4950b0".to_string(), // SpiritSwap Factory
                token_list_url: "https://raw.githubusercontent.com/SpookySwap/spooky-info/master/src/constants/token/spookyswap.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by SpiritSwap", chain))),
        }
    }

    /// Fetch token list for a specific chain with caching - following Uniswap pattern
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
        
        debug!("Fetching SpiritSwap token list from: {}", config.token_list_url);

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

        // Add FTM as a special case for Fantom - following ETH pattern from Uniswap
        if chain == "fantom" {
            let ftm_token = TokenInfo {
                symbol: "FTM".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), // Special FTM address
                decimals: 18,
                name: "Fantom".to_string(),
                chain_id: 250,
            };
            filtered_tokens.push(ftm_token);
        }

        // Cache the result
        {
            let mut cache = self.token_cache.write().await;
            cache.insert(chain.to_string(), filtered_tokens.clone());
        }

        debug!("Found {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol with FTM/WFTM normalization - following Uniswap ETH/WETH pattern
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        
        let symbol_upper = symbol.to_uppercase();
        let normalized_symbol = match symbol_upper.as_str() {
            "FTM" => {
                // For FTM, first try to find FTM, then fallback to WFTM
                if let Some(token) = tokens.iter().find(|t| t.symbol.to_uppercase() == "FTM") {
                    return Ok((token.address.clone(), token.decimals));
                } else {
                    "WFTM" // Fallback to WFTM if FTM not found
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

    /// Convert user-friendly amount to wei/smallest unit - following Uniswap pattern
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

    /// Get SpiritSwap quote using Router's getAmountsOut - following contract-first pattern
    async fn get_spiritswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        let chain = params.chain.as_deref().unwrap_or("fantom");
        
        // Validate chain support
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("SpiritSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses with proper FTM/WFTM handling
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle FTM/WFTM 1:1 conversion edge case - following Uniswap ETH/WETH pattern
        if (params.token_in.to_uppercase() == "FTM" && params.token_out.to_uppercase() == "WFTM") || 
           (params.token_in.to_uppercase() == "WFTM" && params.token_out.to_uppercase() == "FTM") {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok(wei_amount.to_string());
        }

        // Convert amount to wei
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Create provider with timeout
        let provider = ProviderBuilder::new()
            .on_http(config.rpc_url.parse()
                .map_err(|e| DexError::ParseError(format!("Invalid RPC URL: {}", e)))?);

        let router_address = Address::from_str(&config.router_address)
            .map_err(|e| DexError::ParseError(format!("Invalid router address: {}", e)))?;

        // Handle FTM address conversion for contracts - following Uniswap ETH/WETH pattern
        let token_in_address = if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert FTM to WFTM for SpiritSwap contracts
            Address::from_str("0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83") // WFTM on Fantom
                .map_err(|e| DexError::ParseError(format!("Invalid WFTM address: {}", e)))?
        } else {
            Address::from_str(&token_in_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_in address: {}", e)))?
        };

        let token_out_address = if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert FTM to WFTM for SpiritSwap contracts
            Address::from_str("0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83") // WFTM on Fantom
                .map_err(|e| DexError::ParseError(format!("Invalid WFTM address: {}", e)))?
        } else {
            Address::from_str(&token_out_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_out address: {}", e)))?
        };

        // Try multiple routes concurrently - SpiritSwap is Uniswap V2 fork, so try direct and via WFTM
        let mut quote_futures = Vec::new();

        // Direct route
        let direct_route = vec![token_in_address, token_out_address];
        let provider_clone = provider.clone();
        let fut_direct = self.try_quote_with_path_timeout(
            provider_clone,
            router_address,
            amount_in_wei,
            direct_route,
        );
        quote_futures.push(fut_direct);

        // Route via WFTM (if not already using WFTM)
        let wftm_address = Address::from_str("0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83").unwrap();
        if token_in_address != wftm_address && token_out_address != wftm_address {
            let wftm_route = vec![token_in_address, wftm_address, token_out_address];
            let provider_clone = provider.clone();
            let fut_wftm = self.try_quote_with_path_timeout(
                provider_clone,
                router_address,
                amount_in_wei,
                wftm_route,
            );
            quote_futures.push(fut_wftm);
        }

        // Wait for all quotes with timeout - following Uniswap pattern
        let results = tokio::time::timeout(
            Duration::from_secs(10),
            future::join_all(quote_futures)
        ).await.map_err(|_| DexError::Timeout("All quote attempts timed out".to_string()))?;

        // Find the best quote - following Uniswap pattern
        let mut best_quote = U256::ZERO;
        let mut successful_quote = false;

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(quote) => {
                    debug!("Quote successful for route {}: {}", i, quote);
                    if quote > best_quote {
                        best_quote = quote;
                        successful_quote = true;
                    }
                }
                Err(e) => {
                    debug!("Quote failed for route {}: {}", i, e);
                }
            }
        }

        if successful_quote {
            info!("✅ SpiritSwap quote: {} {} -> {} {} on {}", 
                  params.amount_in, params.token_in, best_quote, params.token_out, chain);
            Ok(best_quote.to_string())
        } else {
            Err(DexError::InvalidResponse("No liquidity found for this pair on any route".to_string()))
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
            Duration::from_secs(5), // Individual quote timeout
            self.try_quote_with_path(&provider, router_address, amount_in, path.clone())
        ).await
        .map_err(|_| DexError::Timeout(format!("Quote timeout for path: {:?}", path)))?
    }

    /// Call SpiritSwap Router's getAmountsOut function - following Uniswap manual ABI encoding pattern
    async fn try_quote_with_path(
        &self,
        provider: &RootProvider<Http<Client>>,
        router_address: Address,
        amount_in: U256,
        path: Vec<Address>,
    ) -> Result<U256, DexError> {
        // Encode the getAmountsOut call
        // Function signature: getAmountsOut(uint amountIn, address[] calldata path)
        let function_selector = "0xd06ca61f"; // getAmountsOut selector

        // Encode dynamic array of addresses
        let path_count = path.len();
        let mut encoded_path = String::new();
        
        // Array offset (0x40 = 64 bytes from start)
        encoded_path.push_str(&format!("{:0>64}", "40"));
        
        // Array length
        encoded_path.push_str(&format!("{:0>64}", format!("{:x}", path_count)));
        
        // Array elements
        for addr in &path {
            encoded_path.push_str(&format!("{:0>64}", hex::encode(addr.as_slice())));
        }

        let call_data = format!(
            "{}{}{}",
            function_selector,
            format!("{:0>64}", format!("{:x}", amount_in)), // amountIn
            encoded_path // path array
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
            .map_err(|e| DexError::ContractError(format!("Router getAmountsOut call failed: {}", e)))?;
        
        // Parse the returned bytes as uint256[] array
        // Return data format: [offset, length, amount0, amount1, ...]
        // We want the last amount (output amount)
        if call_result.len() < 96 { // At least 3 * 32 bytes (offset + length + one amount)
            return Err(DexError::InvalidResponse("Invalid getAmountsOut response length".to_string()));
        }

        // Skip offset (32 bytes) and length (32 bytes), then read the last amount
        let array_length_bytes = &call_result[32..64];
        let array_length = U256::from_be_slice(array_length_bytes).to::<usize>();
        
        if array_length != path.len() {
            return Err(DexError::InvalidResponse("Returned array length doesn't match path length".to_string()));
        }

        // The last element is at position: 64 + (array_length - 1) * 32
        let last_amount_start = 64 + (array_length - 1) * 32;
        let last_amount_end = last_amount_start + 32;
        
        if call_result.len() < last_amount_end {
            return Err(DexError::InvalidResponse("Response too short for expected amount".to_string()));
        }

        let amount_out_bytes = &call_result[last_amount_start..last_amount_end];
        Ok(U256::from_be_slice(amount_out_bytes))
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for SpiritSwap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "fantom" => 105_000, // Fantom has very low gas costs
            _ => 105_000,
        }
    }
}

#[async_trait]
impl DexIntegration for SpiritSwapDex {
    fn get_name(&self) -> &'static str {
        "SpiritSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_spiritswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("fantom")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (SpiritSwap only on Fantom)
        if chain != "fantom" {
            return Ok(false);
        }

        // Check if tokens exist on Fantom
        match tokio::time::timeout(
            Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "fantom").await;
                let token_out_result = self.get_token_address(token_out, "fantom").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on fantom via SpiritSwap", token_in, token_out);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["fantom"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spiritswap_initialization() {
        let dex = SpiritSwapDex::new().await.unwrap();
        assert_eq!(dex.get_name(), "SpiritSwap");
        assert!(dex.supports_chain("fantom"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_ftm_wftm_handling() {
        let dex = SpiritSwapDex::new().await.unwrap();
        
        // Test that both FTM and WFTM are found
        let ftm_result = dex.get_token_address("FTM", "fantom").await;
        let wftm_result = dex.get_token_address("WFTM", "fantom").await;
        
        println!("FTM lookup: {:?}", ftm_result);
        println!("WFTM lookup: {:?}", wftm_result);
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = SpiritSwapDex::new().await.unwrap();
        
        // Test FTM conversion (18 decimals)
        let ftm_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(ftm_wei, U256::from(1_000_000_000_000_000_000_u128));
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("100.0", 6).unwrap();
        assert_eq!(usdc_wei, U256::from(100_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = SpiritSwapDex::new().await.unwrap();
        
        let fantom_config = dex.get_chain_config("fantom").unwrap();
        assert_eq!(fantom_config.chain_id, 250);
        assert_eq!(fantom_config.router_address, "0x16327E3FbDaCA3bcF7E38F5Af2599D2DDc33aE52");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }
}