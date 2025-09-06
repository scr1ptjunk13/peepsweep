use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::http::{Client, Http},
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn, error, debug, instrument};

#[derive(Clone, Debug, Deserialize)]
pub struct TokenListResponse {
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

#[derive(Clone, Debug, Deserialize)]
pub struct CamelotQuoteResponse {
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "amountIn")]
    pub amount_in: String,
    #[serde(rename = "routerAddress")]
    pub router_address: Option<String>,
    #[serde(rename = "gasEstimate")]
    pub gas_estimate: Option<String>,
    #[serde(rename = "priceImpact")]
    pub price_impact: Option<String>,
    #[serde(rename = "route")]
    pub route: Option<Vec<CamelotRouteStep>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CamelotRouteStep {
    pub pool: String,
    #[serde(rename = "tokenIn")]
    pub token_in: String,
    #[serde(rename = "tokenOut")]
    pub token_out: String,
    pub fee: Option<String>,
    pub percentage: Option<f64>,
}

#[derive(Debug)]
struct ChainConfig {
    chain_id: u32,
    rpc_url: String,
    quoter_address: String,
    router_address: String,
    token_list_url: String,
    api_url: String,
}

#[derive(Clone)]
pub struct CamelotDex {
    http_client: HttpClient,
    supported_chains: Vec<String>,
}

impl CamelotDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "arbitrum".to_string(),
            "polygon".to_string(), // Camelot is expanding to other chains
        ];

        Ok(Self {
            http_client,
            supported_chains,
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                rpc_url: std::env::var("ARBITRUM_RPC_URL")
                    .unwrap_or_else(|_| "https://arbitrum.llamarpc.com".to_string()),
                quoter_address: "0x9b7A7c93Db745D5f6c679eeDa7Dd15CCd638C063".to_string(), // Algebra v4 Quoter
                router_address: "0xa555826C9a26E13238F657dB06E0A02431839Ef5".to_string(), // Algebra v4 SwapRouter
                token_list_url: "https://bridge.arbitrum.io/token-list-42161.json".to_string(),
                api_url: "https://api.camelot.exchange".to_string(),
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                quoter_address: "0x9b7A7c93Db745D5f6c679eeDa7Dd15CCd638C063".to_string(), // Camelot expanding
                router_address: "0xa555826C9a26E13238F657dB06E0A02431839Ef5".to_string(),
                token_list_url: "https://unpkg.com/quickswap-default-token-list@1.2.28/build/quickswap-default.tokenlist.json".to_string(),
                api_url: "https://polygon-api.camelot.exchange".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by Camelot", chain))),
        }
    }

    /// Fetch token list for a specific chain - NO HARDCODING
    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching Camelot token list from: {}", config.token_list_url);

        let response = self.http_client
            .get(&config.token_list_url)
            .send()
            .await
            .map_err(|e| DexError::NetworkError(e))?;

        if !response.status().is_success() {
            return Err(DexError::ApiError(format!("Failed to fetch token list: {}", response.status())));
        }

        let token_response: TokenListResponse = response.json().await
            .map_err(|e| DexError::InvalidResponse(format!("Failed to parse token list: {}", e)))?;

        // Filter tokens for the specific chain
        let filtered_tokens: Vec<TokenInfo> = token_response.tokens
            .into_iter()
            .filter(|token| token.chain_id == config.chain_id)
            .collect();

        debug!("Found {} tokens for chain {}", filtered_tokens.len(), chain);
        Ok(filtered_tokens)
    }

    /// Get token address by symbol on a specific chain - DYNAMIC LOOKUP
    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        
        for token in tokens {
            if token.symbol.to_uppercase() == symbol.to_uppercase() {
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

    async fn call_camelot_api(&self, params: &QuoteParams) -> Result<String, DexError> {
        let chain = params.chain.as_deref().unwrap_or("arbitrum");
        let config = self.get_chain_config(chain)?;

        // Get token addresses and decimals DYNAMICALLY
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Convert amount to wei
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Try Camelot API first
        let url = format!(
            "{}/v1/quote?tokenIn={}&tokenOut={}&amountIn={}&gasPrice=100000000&maxSteps=3",
            config.api_url, token_in_addr, token_out_addr, amount_in_wei
        );

        info!("Calling Camelot API: {}", url);

        let response = self.http_client
            .get(&url)
            .header("User-Agent", "DexAggregator/1.0")
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| DexError::NetworkError(e))?;

        if response.status().is_success() {
            let api_response: serde_json::Value = response.json().await
                .map_err(|e| DexError::InvalidResponse(format!("Failed to parse API response: {}", e)))?;

            if let Some(amount_out) = api_response.get("amountOut").and_then(|v| v.as_str()) {
                return Ok(amount_out.to_string());
            }
        }

        // Fallback to direct contract call if API fails
        self.call_algebra_quoter_contract(
            &config,
            &token_in_addr,
            &token_out_addr,
            amount_in_wei,
        ).await
    }

    async fn call_algebra_quoter_contract(
        &self,
        config: &ChainConfig,
        token_in: &str,
        token_out: &str,
        amount_in: U256,
    ) -> Result<String, DexError> {
        // Create provider
        let provider = ProviderBuilder::new()
            .on_http(config.rpc_url.parse()
                .map_err(|e| DexError::ParseError(format!("Invalid RPC URL: {}", e)))?);

        let quoter_address = Address::from_str(&config.quoter_address)
            .map_err(|e| DexError::ParseError(format!("Invalid quoter address: {}", e)))?;

        let token_in_address = Address::from_str(token_in)
            .map_err(|e| DexError::ParseError(format!("Invalid token_in address: {}", e)))?;

        let token_out_address = Address::from_str(token_out)
            .map_err(|e| DexError::ParseError(format!("Invalid token_out address: {}", e)))?;

        // Encode the quoteExactInputSingle call for Algebra
        // Function signature: quoteExactInputSingle(address,address,uint256,uint160)
        let function_selector = "0xf7729d43";
        
        let call_data = format!(
            "{}{}{}{}{}",
            function_selector,
            format!("{:0>64}", hex::encode(token_in_address.as_slice())),
            format!("{:0>64}", hex::encode(token_out_address.as_slice())),
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
            .map_err(|e| DexError::ContractError(format!("Camelot quoter call failed: {}", e)))?;
        
        // Parse the returned bytes as U256
        if call_result.len() >= 32 {
            let amount_bytes = &call_result[call_result.len()-32..];
            let amount_out = U256::from_be_slice(amount_bytes);
            Ok(amount_out.to_string())
        } else {
            Err(DexError::InvalidResponse("Invalid quoter response length".to_string()))
        }
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for Camelot swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "arbitrum" => 140_000, // Algebra v4 is gas optimized
            "polygon" => 120_000,
            _ => 150_000,
        }
    }
}

#[async_trait]
impl DexIntegration for CamelotDex {
    fn get_name(&self) -> &'static str {
        "Camelot"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.call_camelot_api(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("arbitrum")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (Camelot only on Arbitrum)
        if chain != "arbitrum" {
            return Ok(false);
        }

        // Try to fetch both tokens - if both exist, pair is supported
        match (
            self.get_token_address(token_in, "arbitrum").await,
            self.get_token_address(token_out, "arbitrum").await
        ) {
            (Ok(_), Ok(_)) => Ok(true),
            _ => {
                debug!("Pair {}/{} not supported on arbitrum via Camelot", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["arbitrum", "polygon"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_camelot_initialization() {
        let camelot = CamelotDex::new().await;
        assert!(camelot.is_ok());
        
        let dex = camelot.unwrap();
        assert_eq!(dex.get_name(), "Camelot");
        assert!(dex.supports_chain("arbitrum"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = CamelotDex::new().await.unwrap();
        
        // Test ETH conversion (18 decimals)
        let eth_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(eth_wei, U256::from(1_000_000_000_000_000_000_u128));
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("100.0", 6).unwrap();
        assert_eq!(usdc_wei, U256::from(100_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = CamelotDex::new().await.unwrap();
        
        let arbitrum_config = dex.get_chain_config("arbitrum").unwrap();
        assert_eq!(arbitrum_config.chain_id, 42161);
        assert_eq!(arbitrum_config.quoter_address, "0x9b7A7c93Db745D5f6c679eeDa7Dd15CCd638C063");
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_token_lookup() {
        let dex = CamelotDex::new().await.unwrap();
        
        // Test fetching real token list
        match dex.fetch_token_list("arbitrum").await {
            Ok(tokens) => {
                println!("✅ Found {} tokens on Arbitrum", tokens.len());
                // Look for GRAIL (Camelot's native token)
                let grail = tokens.iter().find(|t| t.symbol.to_uppercase() == "GRAIL");
                if let Some(grail_token) = grail {
                    println!("Found GRAIL: {} ({})", grail_token.address, grail_token.decimals);
                }
            }
            Err(e) => {
                println!("❌ Token list fetch failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API and RPC
    async fn test_real_camelot_quote() {
        let dex = CamelotDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "1000".to_string(), // 1000 USDC
            chain: "arbitrum".to_string(),
            slippage: Some(0.5),
        };

        match dex.get_quote(&params).await {
            Ok(route) => {
                println!("✅ Real Camelot quote successful!");
                println!("Amount out: {}", route.amount_out);
                println!("Gas estimate: {}", route.gas_used);
            }
            Err(e) => {
                println!("❌ Real API test failed: {:?}", e);
            }
        }
    }
}