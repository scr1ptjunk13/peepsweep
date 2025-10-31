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

// ApeSwap Router ABI - Uniswap V2 compatible
sol! {
    #[sol(rpc)]
    interface IApeSwapRouter {
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    }
}

#[derive(Clone)]
pub struct ApeSwapDex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl ApeSwapDex {
    pub fn new() -> Self {
        // Use framework builder for ApeSwap (Uniswap V2 fork)
        let mut config = DexConfigBuilder::uniswap_v2_fork("ApeSwap");
        
        // Define chain data - BSC and Polygon only
        let chains = [
            ("bsc", "0xcF0feBd3f17CEf5b47b0cD257aCf6025c5BFf3b7", "0x0841BD0B734E4F5853f0dD8d7Ea041c241fb0Da6"),
            ("polygon", "0xC0788A3aD43d79aa53B09c2EaCc313A787d1d607", "0xCf083Be4164828f00cAE704EC15a36D711491284"),
        ];

        // Auto-populate chain configs with ApeSwap-specific settings
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: None,
                fee_denominator: Some(U256::from(1000)), // 0.1% fee (lower than Uniswap)
                supported_tokens: vec![], // Auto-discovery via framework
            });
        }

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_apeswap_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc");
        
        // Use framework helpers - no manual parsing!
        let token_in_addr = DexUtils::parse_token_address(&params.token_in_address, "token_in")?;
        let token_out_addr = DexUtils::parse_token_address(&params.token_out_address, "token_out")?;
        let amount_in_wei = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Framework provider management
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Get router address from config
        let router_address = self.get_router_address(chain)?;

        // Simple direct path for ApeSwap V2 (framework can handle WETH routing later)
        let path = vec![token_in_addr, token_out_addr];

        // Call getAmountsOut on router
        let router = IApeSwapRouter::new(router_address, &provider);
        
        match router.getAmountsOut(amount_in_wei, path.clone()).call().await {
            Ok(result) => {
                let amounts = result.amounts;
                if amounts.len() >= 2 {
                    let amount_out = amounts[amounts.len() - 1];
                    info!("✅ ApeSwap quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("ApeSwap getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No ApeSwap liquidity available".into()))
            }
        }
    }

    fn get_router_address(&self, chain: &str) -> Result<Address, DexError> {
        let chain_config = self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain not supported: {}", chain)))?;
        
        Address::from_str(&chain_config.router_address)
            .map_err(|_| DexError::ConfigError("Invalid router address".into()))
    }
}

#[async_trait]
impl DexIntegration for ApeSwapDex {
    fn get_name(&self) -> &str {
        "ApeSwap"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_apeswap_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "150000".to_string(), // ApeSwap gas usage
            confidence_score: 0.82, // Medium confidence for ApeSwap
        })
    }

    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, chain: &str) -> Result<bool, DexError> {
        Ok(self.config.chains.contains_key(chain))
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("ApeSwap execution not implemented".into()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(150_000) // Standard Uniswap V2 fork gas estimate
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}
