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
pub struct BiSwapDex {
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
    router_address: String,
    factory_address: String,
    token_list_url: String,
}

impl BiSwapDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "bsc".to_string(),
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
                router_address: "0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8".to_string(), // Current BiSwap Router
                factory_address: "0x858E3312ed3A876947EA49d572A7C42DE08af7EE".to_string(), // Current BiSwap Factory
                token_list_url: "https://tokens.pancakeswap.finance/pancakeswap-extended.json".to_string(), // Use PancakeSwap's comprehensive BSC token list
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by BiSwap", chain))),
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
        
        debug!("Fetching BiSwap token list from: {}", config.token_list_url);

        let response = self.http_client
            .get(&config.token_list_url)
            .timeout(std::time::Duration::from_secs(10))
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

        // Add BNB as a special case for BSC
        if chain == "bsc" {
            let bnb_token = TokenInfo {
                symbol: "BNB".to_string(),
                address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), // Special BNB address
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

        debug!("Found {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol on a specific chain with BNB/WBNB normalization
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        
        let symbol_upper = symbol.to_uppercase();
        let normalized_symbol = match symbol_upper.as_str() {
            "BNB" => {
                // For BNB, first try to find BNB, then fallback to WBNB
                if let Some(token) = tokens.iter().find(|t| t.symbol.to_uppercase() == "BNB") {
                    return Ok((token.address.clone(), token.decimals));
                } else {
                    "WBNB" // Fallback to WBNB if BNB not found
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

    /// Get a quote from BiSwap using the Router contract's getAmountsOut
    async fn get_biswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        // Validate chain support
        let chain = params.chain.as_deref().unwrap_or("bsc");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("BiSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses with proper BNB/WBNB handling
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle BNB/WBNB 1:1 conversion edge case
        if (params.token_in.to_uppercase() == "BNB" && params.token_out.to_uppercase() == "WBNB") || 
           (params.token_in.to_uppercase() == "WBNB" && params.token_out.to_uppercase() == "BNB") {
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

        // Handle BNB address conversion for contracts
        let token_in_address = if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert BNB to WBNB for BiSwap contracts
            Address::from_str("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c") // WBNB on BSC
                .map_err(|e| DexError::ParseError(format!("Invalid WBNB address: {}", e)))?
        } else {
            Address::from_str(&token_in_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_in address: {}", e)))?
        };

        let token_out_address = if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            // Convert BNB to WBNB for BiSwap contracts
            Address::from_str("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c") // WBNB on BSC
                .map_err(|e| DexError::ParseError(format!("Invalid WBNB address: {}", e)))?
        } else {
            Address::from_str(&token_out_addr)
                .map_err(|e| DexError::ParseError(format!("Invalid token_out address: {}", e)))?
        };

        // Call getAmountsOut with timeout
        let quote_result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.get_amounts_out(&provider, router_address, amount_in_wei, token_in_address, token_out_address)
        ).await
        .map_err(|_| DexError::Timeout("BiSwap quote request timed out".to_string()))?;

        match quote_result {
            Ok(amount_out) => {
                info!("✅ BiSwap quote: {} {} -> {} {} on {}", 
                      params.amount_in, params.token_in, amount_out, params.token_out, chain);
                Ok(amount_out.to_string())
            }
            Err(e) => {
                error!("❌ BiSwap quote failed: {}", e);
                Err(e)
            }
        }
    }

    /// Call BiSwap Router's getAmountsOut function
    async fn get_amounts_out(
        &self,
        provider: &RootProvider<Http<Client>>,
        router_address: Address,
        amount_in: U256,
        token_in: Address,
        token_out: Address,
    ) -> Result<U256, DexError> {
        // Encode the getAmountsOut call
        // Function signature: getAmountsOut(uint amountIn, address[] calldata path)
        let function_selector = "0xd06ca61f";
        
        // Create path array: [token_in, token_out]
        let path_offset = "0000000000000000000000000000000000000000000000000000000000000040"; // Offset to path array
        let path_length = "0000000000000000000000000000000000000000000000000000000000000002"; // Array length = 2
        
        let call_data = format!(
            "{}{}{}{}{}{}",
            function_selector,
            format!("{:0>64}", format!("{:x}", amount_in)), // amountIn
            path_offset, // offset to path array
            path_length, // path array length
            format!("{:0>64}", hex::encode(token_in.as_slice())), // path[0]
            format!("{:0>64}", hex::encode(token_out.as_slice()))  // path[1]
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
            .map_err(|e| DexError::ContractError(format!("BiSwap router call failed: {}", e)))?;
        
        // Parse the returned bytes - getAmountsOut returns uint[] array
        // The last element in the array is our output amount
        if call_result.len() >= 64 {
            // Skip array metadata and get the last amount (amounts[1])
            let amount_bytes = &call_result[call_result.len()-32..];
            Ok(U256::from_be_slice(amount_bytes))
        } else {
            Err(DexError::InvalidResponse("Invalid BiSwap router response length".to_string()))
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for BiSwap swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "bsc" => 110_000, // BiSwap is optimized for BSC with lower fees
            _ => 110_000,
        }
    }
}

#[async_trait]
impl DexIntegration for BiSwapDex {
    fn get_name(&self) -> &'static str {
        "BiSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_biswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("bsc")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (BiSwap only on BSC)
        if chain != "bsc" {
            return Ok(false);
        }

        // Try to fetch both tokens with timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            async {
                let token_in_result = self.get_token_address(token_in, "bsc").await;
                let token_out_result = self.get_token_address(token_out, "bsc").await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            Ok(_) => {
                debug!("Pair {}/{} not supported on bsc via BiSwap", token_in, token_out);
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
    async fn test_biswap_initialization() {
        let biswap = BiSwapDex::new().await;
        assert!(biswap.is_ok());
        
        let dex = biswap.unwrap();
        assert_eq!(dex.get_name(), "BiSwap");
        assert!(dex.supports_chain("bsc"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_bnb_wbnb_handling() {
        let dex = BiSwapDex::new().await.unwrap();
        
        // Test that both BNB and WBNB are found
        let bnb_result = dex.get_token_address("BNB", "bsc").await;
        let wbnb_result = dex.get_token_address("WBNB", "bsc").await;
        
        // Both should succeed (assuming network is available)
        println!("BNB lookup: {:?}", bnb_result);
        println!("WBNB lookup: {:?}", wbnb_result);
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = BiSwapDex::new().await.unwrap();
        
        // Test BNB conversion (18 decimals)
        let bnb_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(bnb_wei, U256::from(1_000_000_000_000_000_000_u128));
        
        // Test BUSD conversion (18 decimals on BSC)
        let busd_wei = dex.convert_to_wei("100.0", 18).unwrap();
        assert_eq!(busd_wei, U256::from(100_000_000_000_000_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = BiSwapDex::new().await.unwrap();
        
        let bsc_config = dex.get_chain_config("bsc").unwrap();
        assert_eq!(bsc_config.chain_id, 56);
        assert_eq!(bsc_config.router_address, "0x3a6d8cA21D1CF76F653A67577FA0D27453350dD8");
        assert_eq!(bsc_config.factory_address, "0x858E3312ed3A876947EA49d572A7C42DE08af7EE");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    async fn test_native_wrap_conversion() {
        let dex = BiSwapDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "BNB".to_string(),
            token_out: "WBNB".to_string(),
            amount_in: "1.0".to_string(),
            chain: Some("bsc".to_string()),
        };
        
        // Should handle BNB->WBNB as 1:1
        let result = dex.get_biswap_quote(&params).await;
        assert!(result.is_ok());
        
        if let Ok(quote) = result {
            assert_eq!(quote, "1000000000000000000"); // 1 BNB in wei
        }
    }
}