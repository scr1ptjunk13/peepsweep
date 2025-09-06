use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, debug, warn};

#[derive(Debug, Clone)]
pub struct SushiswapDex {
    client: Client,
    supported_chains: Vec<String>,
    // Cache for token address lookups to avoid repeated API calls
    token_cache: Arc<RwLock<HashMap<String, TokenInfo>>>,
    coingecko_api_key: Option<String>, // Optional API key for higher rate limits
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub name: String,
    pub chain_id: u32,
}


// SushiSwap API response (same as before but with fix)
#[derive(Debug, Deserialize)]
struct SushiQuoteResponse {
    status: String,
    #[serde(rename = "amountOut")]
    amount_out: Option<String>,
    #[serde(rename = "assumedAmountOut")] // Fix for the API response issue
    assumed_amount_out: Option<String>,
    #[serde(rename = "gasSpent")]
    gas_spent: Option<u64>,
    #[serde(rename = "priceImpact")]
    price_impact: Option<f64>,
    route: Option<serde_json::Value>,
    error: Option<String>,
    message: Option<String>,
}

#[derive(Debug)]
struct ChainConfig {
    chain_id: u32,
    api_base_url: String,
    coingecko_platform_id: String, // CoinGecko platform identifier
}

impl SushiswapDex {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "ethereum".to_string(),
            "polygon".to_string(),
            "arbitrum".to_string(),
            "optimism".to_string(),
            "avalanche".to_string(),
            "bsc".to_string(),
            "base".to_string(),
        ];

        Self {
            client,
            supported_chains,
            token_cache: Arc::new(RwLock::new(HashMap::new())),
            coingecko_api_key: None, // Set via environment variable if needed
        }
    }

    /// Create instance with CoinGecko API key for higher rate limits
    pub fn with_coingecko_api_key(api_key: String) -> Self {
        let mut instance = Self::new();
        instance.coingecko_api_key = Some(api_key);
        instance
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "ethereum".to_string(),
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "polygon-pos".to_string(),
            }),
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "arbitrum-one".to_string(),
            }),
            "optimism" => Ok(ChainConfig {
                chain_id: 10,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "optimistic-ethereum".to_string(),
            }),
            "avalanche" => Ok(ChainConfig {
                chain_id: 43114,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "avalanche".to_string(),
            }),
            "bsc" => Ok(ChainConfig {
                chain_id: 56,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "binance-smart-chain".to_string(),
            }),
            "base" => Ok(ChainConfig {
                chain_id: 8453,
                api_base_url: "https://api.sushi.com/swap/v7".to_string(),
                coingecko_platform_id: "base".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by SushiSwap", chain))),
        }
    }

    /// Get token address using hardcoded token mappings
    pub fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let symbol_upper = symbol.to_uppercase();
        let chain_lower = chain.to_lowercase();
        
        match (symbol_upper.as_str(), chain_lower.as_str()) {
            // Ethereum mainnet
            ("ETH", "ethereum") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "ethereum") => Ok(("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(), 18)),
            ("USDC", "ethereum") => Ok(("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(), 6)),
            ("USDT", "ethereum") => Ok(("0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(), 6)),
            ("DAI", "ethereum") => Ok(("0x6b175474e89094c44da98b954eedeac495271d0f".to_string(), 18)),
            ("WBTC", "ethereum") => Ok(("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599".to_string(), 8)),
            ("SUSHI", "ethereum") => Ok(("0x6b3595068778dd592e39a122f4f5a5cf09c90fe2".to_string(), 18)),
            
            // Polygon
            ("ETH", "polygon") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "polygon") => Ok(("0x7ceb23fd6c0c043f60c6c6755c4f2de7f5ac9de".to_string(), 18)),
            ("USDC", "polygon") => Ok(("0x3c499c542cef5e3811e1192ce70d8cc03d5c3359".to_string(), 6)),
            ("USDT", "polygon") => Ok(("0xc2132d05d31c914a87c6611c10748aeb04b58e8f".to_string(), 6)),
            ("DAI", "polygon") => Ok(("0x8f3cf7ad23cd3cadbd9735aff958023239c6a063".to_string(), 18)),
            ("WBTC", "polygon") => Ok(("0x1bfd67037b42cf73acf2047067bd4f2c47d9bfd6".to_string(), 8)),
            
            // Arbitrum
            ("ETH", "arbitrum") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "arbitrum") => Ok(("0x82af49447d8a07e3bd95bd0d56f35241523fbab1".to_string(), 18)),
            ("USDC", "arbitrum") => Ok(("0xaf88d065e77c8cc2239327c5edb3a432268e5831".to_string(), 6)),
            ("USDT", "arbitrum") => Ok(("0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9".to_string(), 6)),
            ("DAI", "arbitrum") => Ok(("0xda10009cbd5d07dd0cecc66161fc93d7c9000da1".to_string(), 18)),
            ("WBTC", "arbitrum") => Ok(("0x2f2a2543b76a4166549f7aab2e75bef0aefc5b0f".to_string(), 8)),
            
            // Base
            ("ETH", "base") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "base") => Ok(("0x4200000000000000000000000000000000000006".to_string(), 18)),
            ("USDC", "base") => Ok(("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_string(), 6)),
            ("DAI", "base") => Ok(("0x50c5725949a6f0c72e6c4a641f24049a917db0cb".to_string(), 18)),
            
            // Optimism
            ("ETH", "optimism") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "optimism") => Ok(("0x4200000000000000000000000000000000000006".to_string(), 18)),
            ("USDC", "optimism") => Ok(("0x0b2c639c533813f4aa9d7837caf62653d097ff85".to_string(), 6)),
            ("USDT", "optimism") => Ok(("0x94b008aa00579c1307b0ef2c499ad98a8ce58e58".to_string(), 6)),
            ("DAI", "optimism") => Ok(("0xda10009cbd5d07dd0cecc66161fc93d7c9000da1".to_string(), 18)),
            ("WBTC", "optimism") => Ok(("0x68f180fcce6836688e9084f035309e29bf0a2095".to_string(), 8)),
            
            // BSC
            ("ETH", "bsc") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "bsc") => Ok(("0x2170ed0880ac9a755fd29b2688956bd959f933f8".to_string(), 18)),
            ("USDC", "bsc") => Ok(("0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d".to_string(), 18)),
            ("USDT", "bsc") => Ok(("0x55d398326f99059ff775485246999027b3197955".to_string(), 18)),
            ("DAI", "bsc") => Ok(("0x1af3f329e8be154074d8769d1ffa4ee058b1dbc3".to_string(), 18)),
            ("WBTC", "bsc") => Ok(("0x7130d2a12b9bcbfae4f2634d864a1ee1ce3ead9c".to_string(), 18)),
            
            // Avalanche
            ("ETH", "avalanche") => Ok(("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18)),
            ("WETH", "avalanche") => Ok(("0x49d5c2bdffac6ce2bfdb6640f4f80f226bc10bab".to_string(), 18)),
            ("USDC", "avalanche") => Ok(("0xb97ef9ef8734c71904d8002f8b6bc66dd9c48a6e".to_string(), 6)),
            ("USDT", "avalanche") => Ok(("0x9702230a8ea53601f5cd2dc00fdbc13d4df4a8c7".to_string(), 6)),
            ("DAI", "avalanche") => Ok(("0xd586e7f844cea2f87f50152665bcbc2c279d8d70".to_string(), 18)),
            ("WBTC", "avalanche") => Ok(("0x50b7545627a5162f82a992c33b87adc75187b218".to_string(), 8)),
            
            _ => Err(DexError::UnsupportedPair(format!("Token {} not supported on {} by SushiSwap", symbol, chain)))
        }
    }

    /// Cache token information
    async fn cache_token(&self, cache_key: String, token: TokenInfo) {
        let mut cache = self.token_cache.write().await;
        cache.insert(cache_key, token);
    }

    /// Convert user-friendly amount to wei/smallest unit
    fn convert_to_wei(&self, amount: &str, decimals: u8) -> Result<String, DexError> {
        let amount_f64: f64 = amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount)))?;

        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".to_string()));
        }

        let multiplier = 10_u128.pow(decimals as u32);
        let wei_amount = (amount_f64 * multiplier as f64) as u128;
        
        Ok(wei_amount.to_string())
    }

    async fn get_sushiswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("SushiSwap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses using hardcoded mappings
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain)?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain)?;

        // Handle ETH/WETH edge case
        if (params.token_in.to_uppercase() == "ETH" && params.token_out.to_uppercase() == "WETH") || 
           (params.token_in.to_uppercase() == "WETH" && params.token_out.to_uppercase() == "ETH") {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok(wei_amount);
        }

        let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        let url = format!(
            "{}/{}?tokenIn={}&tokenOut={}&amount={}&maxSlippage={}&sender={}",
            config.api_base_url,
            config.chain_id,
            token_in_addr,
            token_out_addr,
            wei_amount,
            params.slippage.unwrap_or(0.005),
            "0x0000000000000000000000000000000000000000"
        );

        info!("SushiSwap API call: {}", url);

        let response = tokio::time::timeout(
            Duration::from_secs(10),
            self.client
                .get(&url)
                .header("Accept", "application/json")
                .send()
        ).await
        .map_err(|_| DexError::Timeout("SushiSwap API call timed out".to_string()))?
        .map_err(|e| DexError::NetworkError(e))?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| DexError::NetworkError(e))?;

        if !status.is_success() {
            error!("SushiSwap API error {}: {}", status, response_text);
            return Err(DexError::ApiError(format!("SushiSwap API error {}: {}", status, response_text)));
        }

        debug!("SushiSwap raw response: {}", response_text);

        let quote_response: SushiQuoteResponse = serde_json::from_str(&response_text)
            .map_err(|e| DexError::InvalidResponse(format!("Failed to parse response: {}", e)))?;

        if let Some(error_msg) = quote_response.error.or(quote_response.message) {
            return Err(DexError::ApiError(format!("SushiSwap API error: {}", error_msg)));
        }

        if quote_response.status != "Success" {
            return Err(DexError::ApiError(format!("SushiSwap quote failed with status: {}", quote_response.status)));
        }

        // Fixed: Handle both amountOut and assumedAmountOut
        if let Some(amount_out_str) = quote_response.amount_out.or(quote_response.assumed_amount_out) {
            info!("SushiSwap quote: {} {} -> {} {} on {}", 
                  params.amount_in, params.token_in, amount_out_str, params.token_out, chain);
            Ok(amount_out_str)
        } else {
            Err(DexError::InvalidResponse("No amountOut or assumedAmountOut in SushiSwap response".to_string()))
        }
    }
}

#[async_trait]
impl DexIntegration for SushiswapDex {
    fn get_name(&self) -> &'static str {
        "SushiSwap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let quote = self.get_sushiswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("ethereum")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        if !self.supported_chains.contains(&chain.to_string()) {
            return Ok(false);
        }

        let token_in_result = self.get_token_address(token_in, chain);
        let token_out_result = self.get_token_address(token_out, chain);
        
        match (token_in_result, token_out_result) {
            (Ok(_), Ok(_)) => Ok(true),
            _ => {
                debug!("Pair {}/{} not supported on {} via SushiSwap", token_in, token_out, chain);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum", "polygon", "arbitrum", "optimism", "avalanche", "bsc", "base"]
    }
}

impl SushiswapDex {
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 200_000,
            "polygon" | "arbitrum" | "optimism" | "base" => 130_000,
            "avalanche" | "bsc" => 140_000,
            _ => 160_000,
        }
    }

    pub fn wei_to_readable(&self, wei_amount: &str, decimals: u8) -> Result<String, DexError> {
        let wei: u128 = wei_amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid wei amount: {}", wei_amount)))?;

        let divisor = 10_u128.pow(decimals as u32);
        let readable = wei as f64 / divisor as f64;
        
        Ok(format!("{:.6}", readable))
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.token_cache.write().await;
        cache.clear();
    }

    pub async fn get_cache_stats(&self) -> HashMap<String, usize> {
        let cache = self.token_cache.read().await;
        HashMap::from([("cached_tokens".to_string(), cache.len())])
    }

    /// Debug function to show what's in cache
    pub async fn debug_cache(&self) {
        let cache = self.token_cache.read().await;
        println!("Token cache contents:");
        for (key, token) in cache.iter() {
            println!("  {}: {} ({})", key, token.address, token.name);
        }
    }
}