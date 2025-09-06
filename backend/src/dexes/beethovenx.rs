use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn, debug, instrument};

#[derive(Debug, Clone)]
pub struct BeethovenXDex {
    client: Client,
    supported_chains: Vec<String>,
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

#[derive(Debug, Deserialize)]
struct BeethovenXQuoteResponse {
    #[serde(rename = "amountOut")]
    amount_out: Option<String>,
    #[serde(rename = "priceImpact")]
    price_impact: Option<f64>,
    #[serde(rename = "gasEstimate")]
    gas_estimate: Option<u64>,
    route: Option<Vec<BeethovenXRoute>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BeethovenXRoute {
    #[serde(rename = "poolAddress")]
    pool_address: String,
    #[serde(rename = "tokenIn")]
    token_in: String,
    #[serde(rename = "tokenOut")]
    token_out: String,
}

#[derive(Debug)]
struct ChainConfig {
    chain_id: u32,
    api_url: String,
    token_list_url: String,
}

impl BeethovenXDex {
    pub async fn new() -> Result<Self, DexError> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("DexAggregator/1.0")
            .build()
            .map_err(|e| DexError::NetworkError(e))?;

        let supported_chains = vec![
            "fantom".to_string(),
            "optimism".to_string(),
        ];

        Ok(Self {
            client,
            supported_chains,
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "fantom" => Ok(ChainConfig {
                chain_id: 250,
                api_url: "https://api.beethovenx.io/v1".to_string(),
                token_list_url: "https://raw.githubusercontent.com/beethovenxfi/token-list/main/generated/beethovenx-default.tokenlist.json".to_string(),
            }),
            "optimism" => Ok(ChainConfig {
                chain_id: 10,
                api_url: "https://api.beethovenx.io/v1".to_string(),
                token_list_url: "https://raw.githubusercontent.com/beethovenxfi/token-list/main/generated/beethovenx-optimism.tokenlist.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by BeethovenX", chain))),
        }
    }

    /// Fetch token list for a specific chain - NO HARDCODING
    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching BeethovenX token list from: {}", config.token_list_url);

        let response = self.client
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
        // Handle native tokens
        match (symbol.to_uppercase().as_str(), chain.to_lowercase().as_str()) {
            ("ETH", "optimism") => return Ok(("0x4200000000000000000000000000000000000006".to_string(), 18)), // WETH on Optimism
            ("FTM", "fantom") => return Ok(("0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83".to_string(), 18)), // WFTM on Fantom
            _ => {}
        }

        let tokens = self.fetch_token_list(chain).await?;
        
        for token in tokens {
            if token.symbol.to_uppercase() == symbol.to_uppercase() {
                return Ok((token.address, token.decimals));
            }
        }

        Err(DexError::UnsupportedPair(format!("Token {} not found on {}", symbol, chain)))
    }

    /// Convert user-friendly amount to wei/smallest unit
    fn convert_to_wei(&self, amount: &str, decimals: u8) -> Result<String, DexError> {
        let amount_f64: f64 = amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount)))?;

        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".to_string()));
        }

        // Convert to smallest unit
        let multiplier = 10_u128.pow(decimals as u32);
        let wei_amount = (amount_f64 * multiplier as f64) as u128;
        
        Ok(wei_amount.to_string())
    }

    async fn get_beethoven_x_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        // Validate chain support
        let chain = params.chain.as_deref().unwrap_or("fantom");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("BeethovenX doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle ETH/WETH and FTM/WFTM 1:1 conversion edge cases
        if self.is_native_wrapper_pair(&params.token_in, &params.token_out) {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok(wei_amount);
        }

        // Convert amount to wei
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Build the quote URL for BeethovenX API
        let mut url = format!(
            "{}/quote?chainId={}&tokenIn={}&tokenOut={}&amount={}",
            config.api_url, config.chain_id, token_in_addr, token_out_addr, amount_in_wei
        );

        // Add slippage if provided
        if let Some(slippage) = params.slippage {
            url.push_str(&format!("&slippage={}", slippage));
        }

        info!("BeethovenX API call: {}", url);

        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                error!("BeethovenX network error: {}", e);
                DexError::NetworkError(e)
            })?;

        let status = response.status();
        let response_text = response.text().await
            .map_err(|e| DexError::NetworkError(e))?;

        if !status.is_success() {
            error!("BeethovenX API error {}: {}", status, response_text);
            return Err(DexError::ApiError(format!("BeethovenX API error {}: {}", status, response_text)));
        }

        debug!("BeethovenX raw response: {}", response_text);

        let quote_response: BeethovenXQuoteResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                error!("Failed to parse BeethovenX response: {} - Raw: {}", e, response_text);
                DexError::InvalidResponse(format!("Failed to parse response: {}", e))
            })?;

        if let Some(error_msg) = quote_response.error {
            return Err(DexError::ApiError(format!("BeethovenX API error: {}", error_msg)));
        }

        if let Some(amount_out_str) = quote_response.amount_out {
            info!("✅ BeethovenX quote: {} {} -> {} {} on {}",
                  params.amount_in, params.token_in,
                  amount_out_str, params.token_out, chain);
            Ok(amount_out_str)
        } else {
            Err(DexError::InvalidResponse("No amount_out in BeethovenX response".to_string()))
        }
    }

    fn is_native_wrapper_pair(&self, token_in: &str, token_out: &str) -> bool {
        let pairs = [
            ("ETH", "WETH"), ("WETH", "ETH"),
            ("FTM", "WFTM"), ("WFTM", "FTM"),
        ];
        
        pairs.iter().any(|(a, b)| {
            (token_in.to_uppercase() == *a && token_out.to_uppercase() == *b) ||
            (token_in.to_uppercase() == *b && token_out.to_uppercase() == *a)
        })
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for BeethovenX swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "fantom" => 180_000,
            "optimism" => 150_000,
            _ => 180_000,
        }
    }
}

#[async_trait]
impl DexIntegration for BeethovenXDex {
    fn get_name(&self) -> &'static str {
        "BeethovenX"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_beethoven_x_quote(params).await?;
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(chain).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (BeethovenX supports Fantom and Optimism)
        if chain != "fantom" && chain != "optimism" {
            return Ok(false);
        }

        // Try to fetch both tokens - if both exist, pair is supported
        match (
            self.get_token_address(token_in, chain).await,
            self.get_token_address(token_out, chain).await
        ) {
            (Ok(_), Ok(_)) => Ok(true),
            _ => {
                debug!("Pair {}/{} not supported on {} via BeethovenX", token_in, token_out, chain);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["fantom", "optimism"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_beethovenx_initialization() {
        let beethovenx = BeethovenXDex::new().await;
        assert!(beethovenx.is_ok());
        
        let dex = beethovenx.unwrap();
        assert_eq!(dex.get_name(), "BeethovenX");
        assert!(dex.supports_chain("fantom"));
        assert!(dex.supports_chain("optimism"));
        assert!(!dex.supports_chain("ethereum"));
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = BeethovenXDex::new().await.unwrap();
        
        // Test FTM conversion (18 decimals)
        let ftm_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(ftm_wei, "1000000000000000000");
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("100.0", 6).unwrap();
        assert_eq!(usdc_wei, "100000000");
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = BeethovenXDex::new().await.unwrap();
        
        let fantom_config = dex.get_chain_config("fantom").unwrap();
        assert_eq!(fantom_config.chain_id, 250);
        
        let optimism_config = dex.get_chain_config("optimism").unwrap();
        assert_eq!(optimism_config.chain_id, 10);
        
        // Test unsupported chain
        assert!(dex.get_chain_config("ethereum").is_err());
    }

    #[tokio::test]
    async fn test_native_wrapper_detection() {
        let dex = BeethovenXDex::new().await.unwrap();
        
        assert!(dex.is_native_wrapper_pair("ETH", "WETH"));
        assert!(dex.is_native_wrapper_pair("WETH", "ETH"));
        assert!(dex.is_native_wrapper_pair("FTM", "WFTM"));
        assert!(dex.is_native_wrapper_pair("WFTM", "FTM"));
        assert!(!dex.is_native_wrapper_pair("USDC", "DAI"));
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_token_lookup() {
        let dex = BeethovenXDex::new().await.unwrap();
        
        // Test fetching real token list for Fantom
        match dex.fetch_token_list("fantom").await {
            Ok(tokens) => {
                println!("✅ Found {} tokens on Fantom", tokens.len());
                // Look for BEETS token (BeethovenX governance token)
                let beets = tokens.iter().find(|t| t.symbol.to_uppercase() == "BEETS");
                if let Some(beets_token) = beets {
                    println!("Found BEETS: {} ({})", beets_token.address, beets_token.decimals);
                }
            }
            Err(e) => {
                println!("❌ Token list fetch failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_beethovenx_quote() {
        let dex = BeethovenXDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "FTM".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "100".to_string(), // 100 FTM
            chain: "fantom".to_string(),
            slippage: Some(0.5),
        };

        match dex.get_quote(&params).await {
            Ok(route) => {
                println!("✅ Real BeethovenX quote successful!");
                println!("Amount out: {}", route.amount_out);
                println!("Gas estimate: {}", route.gas_used);
            }
            Err(e) => {
                println!("❌ Real API test failed: {:?}", e);
            }
        }
    }
}