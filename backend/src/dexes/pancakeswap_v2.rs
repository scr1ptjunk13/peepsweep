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

// PancakeSwap V2 Router ABI (identical to Uniswap V2)
sol! {
    #[sol(rpc)]
    interface IPancakeV2Router {
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    }
}

#[derive(Clone)]
pub struct PancakeSwapV2Dex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl PancakeSwapV2Dex {
    pub fn new() -> Self {
        // Use framework builder - massive code reduction!
        let mut config = DexConfigBuilder::uniswap_v2_fork("PancakeSwapV2");
        
        // Define chain data in compact config format from research
        let chains = [
            // BSC - Primary chain with highest liquidity
            ("bsc", "0x10ED43C718714eb63d5aA57B78B54704E256024E", "0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73"),
            // Ethereum
            ("ethereum", "0xEfF92A263d31888d860bD50809A8D171709b7b1c", "0x1097053Fd2ea711dad45caCcc45EfF7548fCB362"),
            // Arbitrum One
            ("arbitrum", "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb", "0x02a84c5285d32195eA98161ccA58B899BDCf5BA2"),
            // Base
            ("base", "0x8cFe327CEc66d1C090Dd72bd0FF11d690C33a2Eb", "0x02a84c5285d32195eA98161ccA58B899BDCf5BA2"),
            // zkSync Era - Different router address
            ("zksync", "0x5aEaF2883FBf30f3D62471154eDa3C0C1b05942d", "0xd03D8D566183F0086d8D09A84E1e30b58Dd5619d"),
        ];

        // Auto-populate chain configs - 90% less code!
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: None,
                fee_denominator: Some(U256::from(400)), // 0.25% fee = 1/400
                supported_tokens: vec![], // Auto-discovery via framework
            });
        }

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_pancakeswap_v2_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("bsc"); // Default to BSC
        
        // Use framework helpers - no manual parsing!
        let token_in_addr = DexUtils::parse_token_address(&params.token_in_address, "token_in")?;
        let token_out_addr = DexUtils::parse_token_address(&params.token_out_address, "token_out")?;
        let amount_in_wei = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Framework provider management
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Get router address from config
        let router_address = self.get_router_address(chain)?;

        // Simple direct path for V2 (framework can handle routing later)
        let path = vec![token_in_addr, token_out_addr];

        // Call getAmountsOut on router
        let router = IPancakeV2Router::new(router_address, &provider);
        
        match router.getAmountsOut(amount_in_wei, path.clone()).call().await {
            Ok(result) => {
                let amounts = result.amounts;
                if amounts.len() >= 2 {
                    let amount_out = amounts[amounts.len() - 1];
                    info!("✅ PancakeSwap V2 quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("PancakeSwap V2 getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No PancakeSwap V2 liquidity available".into()))
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
impl DexIntegration for PancakeSwapV2Dex {
    fn get_name(&self) -> &'static str {
        "PancakeSwapV2"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_pancakeswap_v2_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "160000".to_string(), // PancakeSwap V2 gas usage
            confidence_score: 0.88, // Good confidence for PancakeSwap
        })
    }

    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, chain: &str) -> Result<bool, DexError> {
        Ok(self.config.chains.contains_key(chain))
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("Swap execution not yet implemented for PancakeSwapV2".to_string()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(140_000) // Lower gas than Uniswap V2 due to PancakeSwap optimizations
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}