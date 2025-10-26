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

// Uniswap V2 Router ABI
sol! {
    #[sol(rpc)]
    interface IUniswapV2Router {
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    }
}

#[derive(Clone)]
pub struct UniswapV2Dex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl UniswapV2Dex {
    pub fn new() -> Self {
        // Use framework builder - massive code reduction!
        let mut config = DexConfigBuilder::uniswap_v2_fork("UniswapV2");
        
        // Define chain data in compact config format
        let chains = [
            ("ethereum", "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D", "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
            ("arbitrum", "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24", "0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9"),
            ("polygon", "0xedf6066a2b290C185783862C7F4776A2C8077AD1", "0x9e5A52f57b3038F1B8EeE45F28b3C1967e22799C"),
            ("base", "0x4752ba5dbc23f44d87826276bf6fd6b1c372ad24", "0x8909Dc15e40173Ff4699343b6eB8132c65e18eC6"),
        ];

        // Auto-populate chain configs - 90% less code!
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: None,
                fee_denominator: Some(U256::from(1000)), // 0.3% fee
                supported_tokens: vec![], // Auto-discovery via framework
            });
        }

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_uniswap_v2_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Handle ETH → WETH conversion like SushiSwap
        let mut token_in_addr_str = params.token_in_address.as_deref().unwrap_or("");
        let mut token_out_addr_str = params.token_out_address.as_deref().unwrap_or("");
        
        // Convert ETH to WETH for Uniswap V2
        let weth_addr = match chain {
            "ethereum" => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "polygon" => "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270", // WMATIC
            "arbitrum" => "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
            "base" => "0x4200000000000000000000000000000000000006",
            _ => return Err(DexError::UnsupportedChain(chain.to_string())),
        };
        
        if token_in_addr_str == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_in_addr_str = weth_addr;
        }
        if token_out_addr_str == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_out_addr_str = weth_addr;
        }
        
        let token_in_addr = DexUtils::parse_token_address(&Some(token_in_addr_str.to_string()), "token_in")?;
        let token_out_addr = DexUtils::parse_token_address(&Some(token_out_addr_str.to_string()), "token_out")?;
        let amount_in_wei = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Framework provider management
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Get router address from config
        let router_address = self.get_router_address(chain)?;

        // Simple direct path for V2 (framework can handle WETH routing later)
        let path = vec![token_in_addr, token_out_addr];

        // Call getAmountsOut on router
        let router = IUniswapV2Router::new(router_address, &provider);
        
        match router.getAmountsOut(amount_in_wei, path.clone()).call().await {
            Ok(result) => {
                let amounts = result.amounts;
                if amounts.len() >= 2 {
                    let amount_out = amounts[amounts.len() - 1];
                    info!("✅ Uniswap V2 quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("Uniswap V2 getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No V2 liquidity available".into()))
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
impl DexIntegration for UniswapV2Dex {
    fn get_name(&self) -> &'static str {
        "UniswapV2"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_uniswap_v2_quote(params).await?;
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
        Err(DexError::NotImplemented("Swap execution not yet implemented for UniswapV2".to_string()))
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