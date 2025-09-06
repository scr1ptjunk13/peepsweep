use super::{DexError, DexIntegration};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256, Bytes},
    providers::{Provider, ProviderBuilder, RootProvider},
    transports::http::{Client, Http},
    rpc::types::eth::TransactionRequest,
};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn, error, debug, instrument};

#[derive(Debug, Clone)]
pub struct CurveDex {
    http_client: HttpClient,
    provider: HashMap<String, Arc<RootProvider<Http<Client>>>>,
    supported_chains: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CurvePoolsResponse {
    #[serde(rename = "poolData")]
    pool_data: Vec<CurvePool>,
}

#[derive(Debug, Deserialize, Clone)]
struct CurvePool {
    pub id: String,
    pub name: String,
    pub address: String,
    #[serde(rename = "coins")]
    pub tokens: Vec<CurveToken>,
    #[serde(rename = "chainId")]
    pub chain_id: u32,
    #[serde(rename = "implementation")]
    pub implementation: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct CurveToken {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
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
    curve_api_url: String,
    token_list_url: String,
}

impl CurveDex {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let http_client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("DexAggregator/1.0")
            .build()?;

        let supported_chains = vec![
            "ethereum".to_string(),
            "polygon".to_string(),
            "arbitrum".to_string(),
            "optimism".to_string(),
            "avalanche".to_string(),
        ];

        // Initialize providers for each chain
        let mut provider = HashMap::new();
        for chain in &supported_chains {
            let config = Self::get_chain_config_static(chain)?;
            let chain_provider = ProviderBuilder::new()
                .on_http(config.rpc_url.parse()?);
            provider.insert(chain.clone(), Arc::new(chain_provider));
        }

        Ok(Self {
            http_client,
            provider,
            supported_chains,
        })
    }

    fn get_chain_config_static(chain: &str) -> Result<ChainConfig, anyhow::Error> {
        match chain.to_lowercase().as_str() {
            "ethereum" => Ok(ChainConfig {
                chain_id: 1,
                rpc_url: std::env::var("ETHEREUM_RPC_URL")
                    .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string()),
                curve_api_url: "https://api.curve.fi/api/getPools/ethereum/main".to_string(),
                token_list_url: "https://gateway.ipfs.io/ipns/tokens.uniswap.org".to_string(),
            }),
            "polygon" => Ok(ChainConfig {
                chain_id: 137,
                rpc_url: std::env::var("POLYGON_RPC_URL")
                    .unwrap_or_else(|_| "https://polygon.llamarpc.com".to_string()),
                curve_api_url: "https://api.curve.fi/api/getPools/polygon".to_string(),
                token_list_url: "https://unpkg.com/quickswap-default-token-list@1.2.28/build/quickswap-default.tokenlist.json".to_string(),
            }),
            "arbitrum" => Ok(ChainConfig {
                chain_id: 42161,
                rpc_url: std::env::var("ARBITRUM_RPC_URL")
                    .unwrap_or_else(|_| "https://arbitrum.llamarpc.com".to_string()),
                curve_api_url: "https://api.curve.fi/api/getPools/arbitrum".to_string(),
                token_list_url: "https://bridge.arbitrum.io/token-list-42161.json".to_string(),
            }),
            "optimism" => Ok(ChainConfig {
                chain_id: 10,
                rpc_url: std::env::var("OPTIMISM_RPC_URL")
                    .unwrap_or_else(|_| "https://optimism.llamarpc.com".to_string()),
                curve_api_url: "https://api.curve.fi/api/getPools/optimism".to_string(),
                token_list_url: "https://static.optimism.io/optimism.tokenlist.json".to_string(),
            }),
            "avalanche" => Ok(ChainConfig {
                chain_id: 43114,
                rpc_url: std::env::var("AVALANCHE_RPC_URL")
                    .unwrap_or_else(|_| "https://avalanche.llamarpc.com".to_string()),
                curve_api_url: "https://api.curve.fi/api/getPools/avalanche".to_string(),
                token_list_url: "https://raw.githubusercontent.com/traderjoe-xyz/joe-tokenlists/main/joe.tokenlist.json".to_string(),
            }),
            _ => Err(anyhow::anyhow!("Chain {} not supported by Curve", chain)),
        }
    }

    fn get_chain_config(&self, chain: &str) -> Result<ChainConfig, DexError> {
        Self::get_chain_config_static(chain)
            .map_err(|e| DexError::UnsupportedChain(e.to_string()))
    }

    /// Fetch all Curve pools for a specific chain - NO HARDCODING
    pub async fn fetch_curve_pools(&self, chain: &str) -> Result<Vec<CurvePool>, DexError> {
        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching Curve pools from: {}", config.curve_api_url);

        let response = self.http_client
            .get(&config.curve_api_url)
            .send()
            .await
            .map_err(|e| DexError::NetworkError(e))?;

        if !response.status().is_success() {
            return Err(DexError::ApiError(format!("Failed to fetch Curve pools: {}", response.status())));
        }

        let pools_response: CurvePoolsResponse = response.json().await
            .map_err(|e| DexError::InvalidResponse(format!("Failed to parse Curve pools: {}", e)))?;

        debug!("Found {} Curve pools for chain {}", pools_response.pool_data.len(), chain);
        Ok(pools_response.pool_data)
    }

    /// Fetch token list for a specific chain - DYNAMIC LOOKUP
    pub async fn fetch_token_list(&self, chain: &str) -> Result<Vec<TokenInfo>, DexError> {
        let config = self.get_chain_config(chain)?;
        
        debug!("Fetching token list from: {}", config.token_list_url);

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

    /// Find pool that contains both tokens - DYNAMIC POOL DISCOVERY
    pub async fn find_curve_pool(&self, token_in: &str, token_out: &str, chain: &str) -> Result<(CurvePool, usize, usize), DexError> {
        let pools = self.fetch_curve_pools(chain).await?;
        let (token_in_addr, _) = self.get_token_address(token_in, chain).await?;
        let (token_out_addr, _) = self.get_token_address(token_out, chain).await?;

        for pool in pools {
            let mut token_in_index = None;
            let mut token_out_index = None;

            // Find token indices in this pool
            for (idx, pool_token) in pool.tokens.iter().enumerate() {
                if pool_token.address.to_lowercase() == token_in_addr.to_lowercase() {
                    token_in_index = Some(idx);
                }
                if pool_token.address.to_lowercase() == token_out_addr.to_lowercase() {
                    token_out_index = Some(idx);
                }
            }

            // If both tokens found in this pool, return it
            if let (Some(i), Some(j)) = (token_in_index, token_out_index) {
                debug!("Found Curve pool: {} with tokens at indices {} and {}", pool.name, i, j);
                return Ok((pool, i, j));
            }
        }

        Err(DexError::UnsupportedPair(format!("No Curve pool found for {}/{} on {}", token_in, token_out, chain)))
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

    /// Call Curve pool's get_dy function to get quote
    async fn call_curve_get_dy(
        &self, 
        pool_address: Address, 
        i: usize, 
        j: usize, 
        dx: U256,
        chain: &str
    ) -> Result<U256, DexError> {
        let provider = self.provider.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("No provider for chain {}", chain)))?;

        // Encode get_dy(int128 i, int128 j, uint256 dx) call
        // Function selector: get_dy(int128,int128,uint256) = 0x5e0d443f
        let mut call_data = Vec::with_capacity(100);
        call_data.extend_from_slice(&[0x5e, 0x0d, 0x44, 0x3f]); // Function selector
        
        // Encode i (int128) as 32 bytes
        let mut i_bytes = [0u8; 32];
        let i_u128 = i as u128;
        i_bytes[16..32].copy_from_slice(&i_u128.to_be_bytes());
        call_data.extend_from_slice(&i_bytes);
        
        // Encode j (int128) as 32 bytes
        let mut j_bytes = [0u8; 32];
        let j_u128 = j as u128;
        j_bytes[16..32].copy_from_slice(&j_u128.to_be_bytes());
        call_data.extend_from_slice(&j_bytes);
        
        // Encode dx (uint256) as 32 bytes
        let dx_bytes = {
            let mut bytes = [0u8; 32];
            let dx_bytes_vec = dx.to_be_bytes_vec();
            let start_idx = 32_usize.saturating_sub(dx_bytes_vec.len());
            bytes[start_idx..].copy_from_slice(&dx_bytes_vec);
            bytes
        };
        call_data.extend_from_slice(&dx_bytes);

        let call_request = TransactionRequest {
            to: Some(pool_address.into()),
            input: alloy::rpc::types::eth::TransactionInput::new(Bytes::from(call_data)),
            ..Default::default()
        };
        
        debug!("🚀 Curve contract call: pool={:?}, i={}, j={}, dx={}", pool_address, i, j, dx);
        
        match provider.call(&call_request).await {
            Ok(result) => {
                debug!("📥 Curve response: {} bytes", result.len());
                if result.len() >= 32 {
                    let amount_bytes = &result[result.len()-32..];
                    let amount_out = U256::from_be_slice(amount_bytes);
                    debug!("✅ Curve quote successful: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid contract response length".to_string()))
                }
            }
            Err(e) => {
                error!("💥 Curve contract call failed: {}", e);
                Err(DexError::ContractError(format!("Curve get_dy call failed: {}", e)))
            }
        }
    }

    /// Get quote from Curve pools - DYNAMIC POOL DISCOVERY
    #[instrument(skip(self))]
    async fn get_curve_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        let start = std::time::Instant::now();

        // Validate chain support
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        if !self.supported_chains.contains(&chain.to_string()) {
            return Err(DexError::UnsupportedChain(format!("Curve doesn't support chain: {}", chain)));
        }

        // Find pool that contains both tokens DYNAMICALLY
        let (pool, token_in_index, token_out_index) = self.find_curve_pool(
            &params.token_in,
            &params.token_out,
            chain,
        ).await?;

        // Get token decimals from the pool info
        let token_in_decimals = pool.tokens[token_in_index].decimals;
        let token_out_decimals = pool.tokens[token_out_index].decimals;

        // Convert amount to wei using actual token decimals
        let amount_in_wei = self.convert_to_wei(&params.amount_in, token_in_decimals)?;

        // Parse pool address
        let pool_address = Address::from_str(&pool.address)
            .map_err(|e| DexError::ParseError(format!("Invalid pool address: {}", e)))?;

        // Get quote from the pool contract
        let amount_out = self.call_curve_get_dy(
            pool_address,
            token_in_index,
            token_out_index,
            amount_in_wei,
            chain,
        ).await?;

        let elapsed = start.elapsed().as_millis();
        info!("✅ Curve quote: {} {} -> {} {} via pool {} on {} ({}ms)", 
              params.amount_in, params.token_in, amount_out, params.token_out, pool.name, chain, elapsed);

        Ok(amount_out.to_string())
    }

    /// Check if a specific chain is supported
    pub fn supports_chain(&self, chain: &str) -> bool {
        self.supported_chains.contains(&chain.to_string())
    }

    /// Get estimated gas for Curve swaps
    pub fn estimated_gas(&self, chain: &str) -> u64 {
        match chain.to_lowercase().as_str() {
            "ethereum" => 150_000,
            "polygon" | "arbitrum" | "optimism" | "avalanche" => 100_000,
            _ => 120_000,
        }
    }
}

#[async_trait]
impl DexIntegration for CurveDex {
    fn get_name(&self) -> &'static str {
        "Curve Finance"
    }

    #[instrument(skip(self))]
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out = self.get_curve_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.estimated_gas(params.chain.as_deref().unwrap_or("ethereum")).to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (Curve only on Ethereum)
        if chain != "ethereum" {
            return Ok(false);
        }

        // Try to find a pool that contains both tokens
        match self.find_curve_pool(token_in, token_out, "ethereum").await {
            Ok(_) => {
                debug!("✅ Curve supports {}/{} on ethereum", token_in, token_out);
                Ok(true)
            }
            Err(_) => {
                debug!("❌ Curve doesn't support {}/{} on ethereum", token_in, token_out);
                Ok(false)
            }
        }
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        vec!["ethereum", "polygon", "arbitrum", "optimism", "avalanche"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_curve_initialization() {
        let curve = CurveDex::new().await;
        assert!(curve.is_ok());
        
        let dex = curve.unwrap();
        assert_eq!(dex.get_name(), "Curve Finance");
        assert!(dex.supports_chain("ethereum"));
        assert!(dex.supports_chain("polygon"));
        assert!(!dex.supports_chain("solana"));
    }

    #[tokio::test]
    async fn test_amount_conversion() {
        let dex = CurveDex::new().await.unwrap();
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = dex.convert_to_wei("1000.0", 6).unwrap();
        assert_eq!(usdc_wei, U256::from(1_000_000_000_u128));
        
        // Test DAI conversion (18 decimals)
        let dai_wei = dex.convert_to_wei("1.0", 18).unwrap();
        assert_eq!(dai_wei, U256::from(1_000_000_000_000_000_000_u128));
    }

    #[tokio::test]
    async fn test_chain_config() {
        let dex = CurveDex::new().await.unwrap();
        
        let eth_config = dex.get_chain_config("ethereum").unwrap();
        assert_eq!(eth_config.chain_id, 1);
        assert!(eth_config.curve_api_url.contains("ethereum"));
        
        let polygon_config = dex.get_chain_config("polygon").unwrap();
        assert_eq!(polygon_config.chain_id, 137);
        
        // Test unsupported chain
        assert!(dex.get_chain_config("solana").is_err());
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API
    async fn test_real_curve_pools() {
        let dex = CurveDex::new().await.unwrap();
        
        match dex.fetch_curve_pools("ethereum").await {
            Ok(pools) => {
                println!("✅ Found {} Curve pools on Ethereum", pools.len());
                // Look for 3pool
                let three_pool = pools.iter().find(|p| p.name.contains("3pool") || p.name.contains("3Pool"));
                if let Some(pool) = three_pool {
                    println!("Found 3Pool: {} at {}", pool.name, pool.address);
                    for (i, token) in pool.tokens.iter().enumerate() {
                        println!("  Token {}: {} ({})", i, token.symbol, token.address);
                    }
                }
            }
            Err(e) => {
                println!("❌ Curve pools fetch failed: {:?}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Remove to test with real API and RPC
    async fn test_real_curve_quote() {
        let dex = CurveDex::new().await.unwrap();
        
        let params = QuoteParams {
            token_in: "USDC".to_string(),
            token_out: "DAI".to_string(),
            amount_in: "1000".to_string(), // 1000 USDC
            chain: "ethereum".to_string(),
            slippage: Some(0.5),
        };

        match dex.get_quote(&params).await {
            Ok(route) => {
                println!("✅ Real Curve quote successful!");
                println!("Amount out: {}", route.amount_out);
                println!("Gas estimate: {}", route.gas_used);
            }
            Err(e) => {
                println!("❌ Real API test failed: {:?}", e);
            }
        }
    }
}