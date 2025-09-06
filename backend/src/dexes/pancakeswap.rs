use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use alloy_primitives::{Address, U256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_transport_http::{Client, Http};
use async_trait::async_trait;
use futures::future;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug, Clone)]
pub struct PancakeSwapDex {
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
    quoter_address: String,
    factory_address: String,
}

impl PancakeSwapDex {
    pub fn new() -> Self {
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("DexAggregator/1.0")
            .build()
            .expect("Failed to create HTTP client");

        let supported_chains = vec![
            "bsc".to_string(),
            "ethereum".to_string(),
            "arbitrum".to_string(),
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
            "bsc" => Ok(ChainConfig {
                chain_id: 56,
                rpc_url: std::env::var("BSC_RPC_URL")
                    .unwrap_or_else(|_| "https://bsc-dataseed1.binance.org".to_string()),
                quoter_address: "0x678Aa4bF4E210cF2166753e054d5b7c31cc7fa86".to_string(), // Mixed Route Quoter V1
                factory_address: "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".to_string(),
            }),
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                rpc_url: std::env::var("ETHEREUM_RPC_URL")
                    .unwrap_or_else(|_| "https://cloudflare-eth.com".to_string()),
                quoter_address: "0x3d146FcE6c1006857750cBe8aF44f76a28041CCc".to_string(),
                factory_address: "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".to_string(),
            }),
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                rpc_url: std::env::var("ARBITRUM_RPC_URL")
                    .unwrap_or_else(|_| "https://arb1.arbitrum.io/rpc".to_string()),
                quoter_address: "0xbC203d7f83677c7ed3F7acEc959963E7F4ECC5C2".to_string(),
                factory_address: "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".to_string(),
            }),
            "base" => Ok(ChainConfig {
                chain_id: 8453,
                rpc_url: std::env::var("BASE_RPC_URL")
                    .unwrap_or_else(|_| "https://mainnet.base.org".to_string()),
                quoter_address: "0xbC203d7f83677c7ed3F7acEc959963E7F4ECC5C2".to_string(),
                factory_address: "0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865".to_string(),
            }),
            _ => Err(DexError::UnsupportedChain(format!("Chain {} not supported", chain))),
        }
    }

    // HARDCODED TOKENS - No more broken API calls
    fn get_hardcoded_tokens(&self, chain: &str) -> Vec<TokenInfo> {
        match chain.to_lowercase().as_str() {
            "bsc" => vec![
                TokenInfo {
                    symbol: "BNB".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "BNB".to_string(),
                    chain_id: 56,
                },
                TokenInfo {
                    symbol: "WBNB".to_string(),
                    address: "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string(),
                    decimals: 18,
                    name: "Wrapped BNB".to_string(),
                    chain_id: 56,
                },
                TokenInfo {
                    symbol: "USDT".to_string(),
                    address: "0x55d398326f99059fF775485246999027B3197955".to_string(),
                    decimals: 18,
                    name: "Tether USD".to_string(),
                    chain_id: 56,
                },
                TokenInfo {
                    symbol: "USDC".to_string(),
                    address: "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d".to_string(),
                    decimals: 18,
                    name: "USD Coin".to_string(),
                    chain_id: 56,
                },
                TokenInfo {
                    symbol: "CAKE".to_string(),
                    address: "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82".to_string(),
                    decimals: 18,
                    name: "PancakeSwap Token".to_string(),
                    chain_id: 56,
                },
            ],
            "ethereum" => vec![
                TokenInfo {
                    symbol: "ETH".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "Ethereum".to_string(),
                    chain_id: 1,
                },
                TokenInfo {
                    symbol: "WETH".to_string(),
                    address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                    decimals: 18,
                    name: "Wrapped Ether".to_string(),
                    chain_id: 1,
                },
                TokenInfo {
                    symbol: "USDC".to_string(),
                    address: "0xA0b86a33E6c39e2A1b12556D99b86E9AAb6C79C8".to_string(),
                    decimals: 6,
                    name: "USD Coin".to_string(),
                    chain_id: 1,
                },
                TokenInfo {
                    symbol: "USDT".to_string(),
                    address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
                    decimals: 6,
                    name: "Tether USD".to_string(),
                    chain_id: 1,
                },
            ],
            "arbitrum" => vec![
                TokenInfo {
                    symbol: "ETH".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "Ethereum".to_string(),
                    chain_id: 42161,
                },
                TokenInfo {
                    symbol: "WETH".to_string(),
                    address: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string(),
                    decimals: 18,
                    name: "Wrapped Ether".to_string(),
                    chain_id: 42161,
                },
                TokenInfo {
                    symbol: "USDC".to_string(),
                    address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string(),
                    decimals: 6,
                    name: "USD Coin".to_string(),
                    chain_id: 42161,
                },
            ],
            "base" => vec![
                TokenInfo {
                    symbol: "ETH".to_string(),
                    address: "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(),
                    decimals: 18,
                    name: "Ethereum".to_string(),
                    chain_id: 8453,
                },
                TokenInfo {
                    symbol: "WETH".to_string(),
                    address: "0x4200000000000000000000000000000000000006".to_string(),
                    decimals: 18,
                    name: "Wrapped Ether".to_string(),
                    chain_id: 8453,
                },
                TokenInfo {
                    symbol: "USDC".to_string(),
                    address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
                    decimals: 6,
                    name: "USD Coin".to_string(),
                    chain_id: 8453,
                },
            ],
            _ => vec![],
        }
    }

    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(cached_tokens) = cache.get(chain) {
                debug!("Using cached token list for {}", chain);
                return Ok(cached_tokens.clone());
            }
        }

        // Use hardcoded tokens instead of broken APIs
        let tokens = self.get_hardcoded_tokens(chain);
        
        if tokens.is_empty() {
            return Err(DexError::UnsupportedChain(format!("No tokens available for chain {}", chain)));
        }

        // Cache the result
        {
            let mut cache = self.token_cache.write().await;
            cache.insert(chain.to_string(), tokens.clone());
        }

        info!("Using {} hardcoded tokens for chain {}", tokens.len(), chain);
        Ok(tokens)
    }

    pub async fn get_token_address(&self, symbol: &str, chain: &str) -> Result<(String, u8), DexError> {
        let tokens = self.fetch_token_list(chain).await?;
        let symbol_upper = symbol.to_uppercase();
        
        for token in tokens {
            if token.symbol.to_uppercase() == symbol_upper {
                return Ok((token.address, token.decimals));
            }
        }

        Err(DexError::UnsupportedPair(format!("Token {} not found on {}", symbol, chain)))
    }

    fn convert_to_wei(&self, amount: &str, decimals: u8) -> Result<U256, DexError> {
        let amount_f64: f64 = amount.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount)))?;

        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".to_string()));
        }

        // Convert to wei with proper decimal handling
        let multiplier = 10_f64.powi(decimals as i32);
        let wei_amount = (amount_f64 * multiplier) as u128;
        
        Ok(U256::from(wei_amount))
    }

    fn get_wrapped_address(&self, token_addr: &str, chain: &str) -> Result<Address, DexError> {
        if token_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            match chain {
                "bsc" => Address::from_str("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"), // WBNB
                "ethereum" => Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), // WETH
                "arbitrum" => Address::from_str("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"), // WETH
                "base" => Address::from_str("0x4200000000000000000000000000000000000006"), // WETH
                _ => return Err(DexError::UnsupportedChain(format!("No wrapped token for chain {}", chain))),
            }
        } else {
            Address::from_str(token_addr)
        }.map_err(|e| DexError::InvalidResponse(format!("Invalid address: {}", e)))
    }

    async fn try_quote_with_fee(
        &self,
        provider: &RootProvider<Http<Client>>,
        quoter_address: Address,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        fee: u32,
        chain: &str,
    ) -> Result<(U256, u32), DexError> {
        // PancakeSwap V3 quoteExactInputSingle function signature
        let function_selector = "0xf7729d43";
        
        // Encode parameters properly
        let mut call_data = String::from(function_selector);
        call_data.push_str(&format!("{:0>64}", hex::encode(token_in.as_slice())));
        call_data.push_str(&format!("{:0>64}", hex::encode(token_out.as_slice())));
        call_data.push_str(&format!("{:0>64}", format!("{:x}", fee)));
        call_data.push_str(&format!("{:0>64}", format!("{:x}", amount_in)));
        call_data.push_str(&format!("{:0>64}", "0")); // sqrtPriceLimitX96 = 0

        debug!("Contract call - Fee: {}, Data: 0x{}", fee, call_data);

        let call_data_bytes = hex::decode(&call_data)
            .map_err(|e| DexError::InvalidResponse(format!("Failed to encode call: {}", e)))?;

        let tx = alloy_rpc_types::TransactionRequest::default()
            .to(quoter_address)
            .input(call_data_bytes.into());

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            provider.call(&tx).block(alloy_rpc_types::BlockId::latest())
        ).await
        .map_err(|_| DexError::Timeout("Contract call timed out".to_string()))?
        .map_err(|e| DexError::ApiError(format!("RPC call failed: {}", e)))?;

        if result.is_empty() || result.len() < 32 {
            return Err(DexError::ApiError("Invalid contract response".to_string()));
        }

        let amount_out = U256::from_be_slice(&result[0..32]);
        
        if amount_out.is_zero() {
            return Err(DexError::ApiError(format!("No liquidity for fee {}", fee)));
        }

        let gas_estimate = self.estimated_gas(chain) as u32;
        debug!("Quote success - Fee: {}, Out: {}, Gas: {}", fee, amount_out, gas_estimate);
        
        Ok((amount_out, gas_estimate))
    }

    async fn get_pancakeswap_quote(&self, params: &QuoteParams) -> Result<(String, u64), DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("Chain {} not supported", chain)));
        }

        let config = self.get_chain_config(chain)?;

        // Get token info
        let (token_in_addr, token_in_decimals) = self.get_token_address(&params.token_in, chain).await?;
        let (token_out_addr, _token_out_decimals) = self.get_token_address(&params.token_out, chain).await?;

        // Handle native/wrapped 1:1 conversions
        let is_native_wrap = match chain {
            "bsc" => (params.token_in.to_uppercase() == "BNB" && params.token_out.to_uppercase() == "WBNB") || 
                     (params.token_in.to_uppercase() == "WBNB" && params.token_out.to_uppercase() == "BNB"),
            _ => (params.token_in.to_uppercase() == "ETH" && params.token_out.to_uppercase() == "WETH") || 
                 (params.token_in.to_uppercase() == "WETH" && params.token_out.to_uppercase() == "ETH"),
        };

        if is_native_wrap {
            let wei_amount = self.convert_to_wei(&params.amount_in, token_in_decimals)?;
            return Ok((wei_amount.to_string(), self.estimated_gas(chain)));
        }

        // Get contract addresses
        let token_in_contract = self.get_wrapped_address(&token_in_addr, chain)?;
        let token_out_contract = self.get_wrapped_address(&token_out_addr, chain)?;
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Create provider
        let rpc_client = RpcClient::new_http(config.rpc_url.parse().unwrap());
        let provider = RootProvider::new(rpc_client);
        
        let quoter_address = Address::from_str(&config.quoter_address)
            .map_err(|e| DexError::InvalidResponse(format!("Invalid quoter: {}", e)))?;

        // PancakeSwap V3 fee tiers
        let fee_tiers = [100u32, 500u32, 2500u32, 10000u32];
        
        info!("Trying {} PancakeSwap fee tiers for {} {} -> {} on {}", 
              fee_tiers.len(), params.amount_in, params.token_in, params.token_out, chain);

        // Try all fee tiers concurrently
        let mut quote_futures = Vec::new();
        
        for fee in fee_tiers {
            let quote_future = self.try_quote_with_fee(
                &provider,
                quoter_address,
                token_in_contract,
                token_out_contract,
                amount_in_wei,
                fee,
                chain,
            );
            quote_futures.push(quote_future);
        }

        let results = tokio::time::timeout(
            Duration::from_secs(10),
            future::join_all(quote_futures)
        ).await
        .map_err(|_| DexError::Timeout("All quote attempts timed out".to_string()))?;

        // Find best quote
        let mut best_quote: Option<(U256, u32)> = None;
        let mut best_fee_tier = 0u32;
        
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok((amount_out, gas_estimate)) => {
                    debug!("Fee tier {} quote: {}", fee_tiers[i], amount_out);
                    match &best_quote {
                        None => {
                            best_quote = Some((amount_out, gas_estimate));
                            best_fee_tier = fee_tiers[i];
                        }
                        Some((best_amount, _)) => {
                            if amount_out > *best_amount {
                                best_quote = Some((amount_out, gas_estimate));
                                best_fee_tier = fee_tiers[i];
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!("Fee tier {} failed: {}", fee_tiers[i], e);
                }
            }
        }

        match best_quote {
            Some((amount_out, gas_estimate)) => {
                info!("Best quote: {} {} -> {} {} (fee: {}bp)", 
                      params.amount_in, params.token_in, amount_out, params.token_out, best_fee_tier);
                Ok((amount_out.to_string(), gas_estimate as u64))
            }
            None => {
                Err(DexError::ApiError("No quotes available for any fee tier".to_string()))
            }
        }
    }

    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 180_000,
            "bsc" => 120_000,
            "arbitrum" | "base" => 100_000,
            _ => 150_000,
        }
    }

    // Helper for readable conversion
    pub fn wei_to_readable(&self, wei_amount: &str, decimals: u8) -> Result<String, DexError> {
        let wei = U256::from_str(wei_amount)
            .map_err(|_| DexError::InvalidAmount(format!("Invalid wei: {}", wei_amount)))?;

        let divisor = U256::from(10).pow(U256::from(decimals));
        let readable = wei.as_limbs()[0] as f64 / divisor.as_limbs()[0] as f64;
        
        Ok(format!("{:.6}", readable))
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.token_cache.write().await;
        cache.clear();
    }

    pub async fn get_cache_stats(&self) -> HashMap<String, usize> {
        let cache = self.token_cache.read().await;
        cache.iter()
            .map(|(chain, tokens)| (chain.clone(), tokens.len()))
            .collect()
    }
}

#[async_trait]
impl DexIntegration for PancakeSwapDex {
    fn get_name(&self) -> &'static str {
        "PancakeSwap V3"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        info!("PancakeSwap get_quote: {} {} -> {} on {}",
              params.amount_in, params.token_in, params.token_out, chain);

        let (quote, gas_used) = self.get_pancakeswap_quote(params).await?;
        
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
                debug!("Pair {}/{} not supported on {} via PancakeSwap", token_in, token_out, chain);
                Ok(false)
            }
            Err(_) => {
                warn!("Pair support check timed out for {}/{}", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["bsc", "ethereum", "arbitrum", "base"]
    }
}

impl PancakeSwapDex {
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    async fn check_pair_on_chain(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        if !self.supported_chains.contains(&chain.to_string()) {
            return Ok(false);
        }

        match tokio::time::timeout(
            Duration::from_secs(3),
            async {
                let token_in_result = self.get_token_address(token_in, chain).await;
                let token_out_result = self.get_token_address(token_out, chain).await;
                (token_in_result, token_out_result)
            }
        ).await {
            Ok((Ok(_), Ok(_))) => Ok(true),
            _ => Ok(false),
        }
    }
}