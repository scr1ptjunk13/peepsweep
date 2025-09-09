use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown, SwapParams};
use crate::dexes::utils::{
    dex_template::{DexConfig, ChainConfig, DexConfigBuilder},
    DexUtils, ProviderCache
};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256},
    sol,
};
use std::str::FromStr;
use tracing::{debug, info};

// SushiSwap V2 Router ABI (identical to UniswapV2)
sol! {
    #[sol(rpc)]
    interface ISushiSwapV2Router {
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    }
}

#[derive(Clone)]
pub struct SushiSwapV2Dex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl SushiSwapV2Dex {
    pub fn new() -> Self {
        // Use framework builder - massive code reduction!
        let mut config = DexConfigBuilder::uniswap_v2_fork("SushiSwap V2");
        
        // Define chain data in compact config format
        let chains = [
            ("ethereum", "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f", "0xc0aee478e3658e2610c5f7a4a2e1777ce9e4f2ac"),
            ("polygon", "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506", "0xc35dadb65012ec5796536bd9864ed8773abc74c4"),
            ("arbitrum", "0x1b02da8cb0d097eb8d57a175b88c7d8b47997506", "0xc35dadb65012ec5796536bd9864ed8773abc74c4"),
            ("base", "0x6BDED42c6DA8FBf0d2bA55B2fa120C5e0c8D7891", "0x71524B4f93c58fcbF659783284E38825f0622859"),
        ];

        // Auto-populate chain configs - 90% less code!
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: None,
                fee_denominator: Some(U256::from(1000)), // 0.3% fee (same as UniswapV2)
                supported_tokens: vec![], // Auto-discovery via framework
            });
        }

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_sushiswap_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Handle native ETH conversion to WETH
        let (token_in_addr, token_out_addr) = self.handle_native_tokens(params, chain)?;
        let amount_in_wei = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Framework provider management
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Get router address from config
        let router_address = self.get_router_address(chain)?;

        // Simple direct path for V2 (framework can handle WETH routing later)
        let path = vec![token_in_addr, token_out_addr];

        // Call getAmountsOut on router
        let router = ISushiSwapV2Router::new(router_address, &provider);
        
        match router.getAmountsOut(amount_in_wei, path.clone()).call().await {
            Ok(result) => {
                let amounts = result.amounts;
                if amounts.len() >= 2 {
                    let amount_out = amounts[amounts.len() - 1];
                    info!("✅ SushiSwap V2 quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("SushiSwap V2 getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No SushiSwap V2 liquidity available".into()))
            }
        }
    }

    fn get_router_address(&self, chain: &str) -> Result<Address, DexError> {
        let chain_config = self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain not supported: {}", chain)))?;
        
        Address::from_str(&chain_config.router_address)
            .map_err(|_| DexError::ConfigError("Invalid router address".into()))
    }

    /// Handle native ETH conversion to WETH for SushiSwap compatibility
    fn handle_native_tokens(&self, params: &QuoteParams, chain: &str) -> Result<(Address, Address), DexError> {
        let mut token_in_addr = params.token_in_address.as_deref().unwrap_or("");
        let mut token_out_addr = params.token_out_address.as_deref().unwrap_or("");
        
        // Get WETH address for this chain from config
        let weth_addr = match chain {
            "ethereum" => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "polygon" => "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270", // WMATIC
            "arbitrum" => "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
            "base" => "0x4200000000000000000000000000000000000006",
            _ => return Err(DexError::UnsupportedChain(chain.to_string())),
        };
        
        // Handle ETH -> WETH conversion
        if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_in_addr = weth_addr;
            debug!("Converted native ETH input to WETH: {}", weth_addr);
        }
        
        if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_out_addr = weth_addr;
            debug!("Converted native ETH output to WETH: {}", weth_addr);
        }
        
        let parsed_in = DexUtils::parse_token_address(&Some(token_in_addr.to_string()), "token_in")?;
        let parsed_out = DexUtils::parse_token_address(&Some(token_out_addr.to_string()), "token_out")?;
        
        Ok((parsed_in, parsed_out))
    }
}

#[async_trait]
impl DexIntegration for SushiSwapV2Dex {
    fn get_name(&self) -> &str {
        "SushiSwap V2"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_sushiswap_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "150000".to_string(), // V2 is more gas efficient than V3
        })
    }

    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, chain: &str) -> Result<bool, DexError> {
        Ok(self.config.chains.contains_key(chain))
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("Swap execution not yet implemented for SushiSwap V2".to_string()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(150_000) // V2 swaps are generally more gas efficient
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}