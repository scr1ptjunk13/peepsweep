use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn, error, debug, instrument};

#[derive(Clone, Debug)]
pub struct CowSwapDex {
    http_client: HttpClient,
    supported_chains: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CowSwapQuote {
    #[serde(rename = "buyAmount")]
    pub buy_amount: String,
    #[serde(rename = "sellAmount")]
    pub sell_amount: String,
    #[serde(rename = "feeAmount")]
    pub fee_amount: String,
    #[serde(rename = "buyToken")]
    pub buy_token: String,
    #[serde(rename = "sellToken")]
    pub sell_token: String,
    #[serde(rename = "validTo")]
    pub valid_to: u64,
    pub kind: String,
}

#[derive(Serialize)]
struct CowSwapQuoteRequest {
    #[serde(rename = "sellToken")]
    sell_token: String,
    #[serde(rename = "buyToken")]
    buy_token: String,
    #[serde(rename = "sellAmountBeforeFee")]
    sell_amount_before_fee: String,
    #[serde(rename = "kind")]
    kind: String,
    #[serde(rename = "from")]
    from: String,
    #[serde(rename = "receiver")]
    receiver: String,
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
    api_url: String,
    token_list_url: String,
}

impl CowSwapDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "ethereum".to_string(),
            "gnosis".to_string(),
            "arbitrum".to_string(),
        ];

        Ok(Self {
            http_client,
            supported_chains,
        })
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        match chain.to_lowercase().as_str() {
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                api_url: "https://api.cow.fi/mainnet/api/v1".to_string(),
                token_list_url: "https://gateway.ipfs.io/ipns/tokens.uniswap.org".to_string(),
            }),
            "gnosis" => Ok(ChainConfig {
                chain_id: 100,
                api_url: "https://api.cow.fi/xdai/api/v1".to_string(),
                token_list_url: "https://tokens.honeyswap.org".to_string(),
            }),
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                api_url: "https://api.cow.fi/arbitrum_one/api/v1".to_string(),
                token_list_url: "https://bridge.arbitrum.io/token-list-42161.json".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported by CoW Swap", chain))),
        }
    }

    /// Fetch token list for a specific chain - NO HARDCODING
    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching CoW Swap token list from: {}", config.token_list_url);

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

    /// Get quote from CoW Swap API
    #[instrument(skip(self))]
    async fn get_cowswap_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        let start = std::time::Instant::now();

        // Validate chain support
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("CoW Swap doesn't support chain: {}", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token addresses and decimals DYNAMICALLY
        let (sell_token_addr, sell_token_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (buy_token_addr, _buy_token_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Convert amount to wei using actual token decimals
        let sell_amount_wei = self.convert_to_wei(&params.amount_in, sell_token_decimals)?;

        // Create quote request
        let quote_request = CowSwapQuoteRequest {
            sell_token: sell_token_addr.clone(),
            buy_token: buy_token_addr.clone(),
            sell_amount_before_fee: sell_amount_wei,
            kind: "sell".to_string(),
            from: "0x0000000000000000000000000000000000000000".to_string(), // Dummy address for quote
            receiver: "0x0000000000000000000000000000000000000000".to_string(),
        };

        info!("Making CoW Swap API call: {} {} -> {} {} on {}", 
              params.amount_in, params.token_in, params.token_out, chain, config.chain_id);

        // Make API call to CoW Protocol
        let url = format!("{}/quote", config.api_url);
        let response = self.http_client
            .post(&url)
            .json(&quote_request)
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| DexError::NetworkError(e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("CoW Swap API error: {} - {}", status, error_text);
            return Err(DexError::ApiError(format!("CoW Swap API error {}: {}", status, error_text)));
        }

        let quote_response: CowSwapQuote = response.json().await
            .map_err(|e| DexError::InvalidResponse(format!("Failed to parse CoW Swap response: {}", e)))?;

        let elapsed = start.elapsed().as_millis();
        info!("✅ CoW Swap quote: {} {} -> {} {} on {} ({}ms, fee: {})",
              params.amount_in, params.token_in,
              quote_response.buy_amount, params.token_out, chain,
              start.elapsed().as_millis(), quote_response.fee_amount);

        Ok(quote_response.buy_amount)
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for CoW Swap (gasless for users!)
    pub fn estimated_gas(&self, _chain: &str) -> u64 {
        0 // CoW Protocol is gasless for users - solvers pay gas
    }
}

#[async_trait]
impl DexIntegration for CowSwapDex {
    fn get_name(&self) -> &'static str {
        "CoW Swap"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_cowswap_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("ethereum")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (CoW Swap only on Ethereum)
        if chain != "ethereum" {
            return Ok(false);
        }

        // Try to fetch both tokens - if both exist, pair is supported
        match (
            self.get_token_address(token_in, "ethereum").await,
            self.get_token_address(token_out, "ethereum").await
        ) {
            (Ok(_), Ok(_)) => {
                debug!("✅ CoW Swap supports {}/{} on ethereum", token_in, token_out);
                Ok(true)
            }
            _ => {
                debug!("❌ CoW Swap doesn't support {}/{} on ethereum", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum", "gnosis", "arbitrum"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cowswap_initialization() {
        let cowswap = CowSwapDex::new().await;
        assert!(cowswap.is_ok());
        
        let dex = cowswap.unwrap();
        assert_eq!(dex.get_name(), "CoW Swap");
        assert!(dex.supports_chain("ethereum"));
        assert!(dex.supports_chain("gnosis"));
        assert!(dex.supports_chain("arbitrum"));
        assert!(!dex.supports_chain("polygon"));
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = CowSwapDex::new().await.unwrap();
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("1000.0", 6).unwrap();
        assert_eq!(usdc_wei, "1000000000");
        
        // Test WETH conversion (18 decimals)
        let eth_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(eth_wei, "1000000000000000000");
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = CowSwapDex::new().await.unwrap();
        
        let eth_config = dex.get_chain_config("ethereum").unwrap();
        assert_eq!(eth_config.chain_id, 1);
        assert!(eth_config.api_url.contains("mainnet"));
        
        let gnosis_config = dex.get_chain_config("gnosis").unwrap();
        assert_eq!(gnosis_config.chain_id, 100);
        assert!(gnosis_config.api_url.contains("xdai"));
        
        let arbitrum_config = dex.get_chain_config("arbitrum").unwrap();
        assert_eq!(arbitrum_config.chain_id, 42161);
        assert!(arbitrum_config.api_url.contains("arbitrum_one"));
        
        // Test unsupported chain
        assert!(dex.get_chain_config("polygon").is_err());
    }

    #[tokio::test]
    async fn test_gas_estimation() {
        let dex = CowSwapDex::new().await.unwrap();
        
        // CoW Swap is gasless for users
        assert_eq!(dex.estimated_gas("ethereum"), 0);
        assert_eq!(dex.estimated_gas("gnosis"), 0);
        assert_eq!(dex.estimated_gas("arbitrum"), 0);
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_token_lookup() {
        let dex = CowSwapDex::new().await.unwrap();
        
        match dex.fetch_token_list("ethereum").await {
            Ok(tokens) => {
                println!("✅ Found {} tokens on Ethereum", tokens.len());
                // Look for USDC
                let usdc = tokens.iter().find(|t| t.symbol.to_uppercase() == "USDC");
                if let Some(usdc_token) = usdc {
                    println!("Found USDC: {} ({})", usdc_token.address, usdc_token.decimals);
                }
            }
            Err(e) => {
                println!("❌ Token list fetch failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_cowswap_quote() {
        let dex = CowSwapDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "1000".to_string(), // 1000 USDC
            chain: "ethereum".to_string(),
            slippage: Some(0.5),
        };

        match dex.get_quote(&params).await {
            Ok(route) => {
                println!("✅ Real CoW Swap quote successful!");
                println!("Amount out: {}", route.amount_out);
                println!("Gas estimate: {} (should be 0)", route.gas_used);
            }
            Err(e) => {
                println!("❌ Real API test failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API  
    async fn test_multi_chain_support() {
        let dex = CowSwapDex::new().await.unwrap();
        
        // Test Ethereum
        let eth_params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "100".to_string(),
            chain: "ethereum".to_string(),
            slippage: Some(0.5),
        };

        // Test Gnosis Chain
        let gnosis_params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "100".to_string(),
            chain: "gnosis".to_string(),
            slippage: Some(0.5),
        };

        // Test Arbitrum
        let arbitrum_params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "100".to_string(),
            chain: "arbitrum".to_string(),
            slippage: Some(0.5),
        };

        println!("Testing multi-chain CoW Swap support...");
        
        for (chain_name, params) in [
            ("Ethereum", &eth_params),
            ("Gnosis", &gnosis_params), 
            ("Arbitrum", &arbitrum_params)
        ] {
            match dex.get_quote(params).await {
                Ok(route) => {
                    println!("✅ {} quote successful: {}", chain_name, route.amount_out);
                }
                Err(e) => {
                    println!("❌ {} quote failed: {:?}", chain_name, e);
                }
            }
        }
    }
}