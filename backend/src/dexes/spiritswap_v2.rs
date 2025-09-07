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

// SpiritSwap V2 Router ABI (identical to Uniswap V2)
sol! {
    #[sol(rpc)]
    interface ISpiritV2Router {
        function getAmountsOut(
            uint amountIn,
            address[] calldata path
        ) external view returns (uint[] memory amounts);
    }
}

#[derive(Clone)]
pub struct SpiritSwapV2Dex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl SpiritSwapV2Dex {
    pub fn new() -> Self {
        // Use framework builder - massive code reduction!
        let mut config = DexConfigBuilder::uniswap_v2_fork("SpiritSwapV2");
        
        // Define chain data - Fantom Opera only (SpiritSwap's exclusive chain)
        let chains = [
            // Fantom Opera - Only chain with verified contracts
            ("fantom", "0x16327e3fbdaca3bcf7e38f5af2599d2ddc33ae52", "0xef45d134b73241eda7703fa787148d9c9f4950b0"),
        ];

        // Auto-populate chain configs - 90% less code!
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: Some("0x00fb7f630766e6a796048ea87d01acd3068e8ff67d078148a3fa3f4a84f69bd5".to_string()), // SpiritSwap init code hash
                fee_denominator: Some(U256::from(10000)), // For 0.25% fee calculation
                supported_tokens: vec![], // Will be populated later
            });
        }

        // Fee is handled in ChainConfig.fee_denominator

        Self {
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_spiritswap_v2_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_ref().ok_or_else(|| DexError::ConfigError("Chain not specified".into()))?;
        let provider = self.provider_cache.get_provider(chain).await?;
        let router_address = self.get_router_address(chain)?;
        
        // Parse token addresses
        let token_in = Address::from_str(&params.token_in)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token_in address: {}", params.token_in)))?;
        let token_out = Address::from_str(&params.token_out)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token_out address: {}", params.token_out)))?;

        // Build path for routing
        let path = vec![token_in, token_out];
        
        // Parse amount
        let amount_in = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Create router contract instance
        let router = ISpiritV2Router::new(router_address, &provider);

        // Call getAmountsOut
        match router.getAmountsOut(amount_in, path).call().await {
            Ok(amounts) => {
                if amounts.amounts.len() >= 2 {
                    let amount_out = amounts.amounts[1];
                    info!("✅ SpiritSwap V2 quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::ContractCallFailed("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("SpiritSwap V2 getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No SpiritSwap V2 liquidity available".into()))
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
impl DexIntegration for SpiritSwapV2Dex {
    fn get_name(&self) -> &'static str {
        "SpiritSwapV2"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_spiritswap_v2_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "135000".to_string(), // Slightly lower than Uniswap V2 due to Fantom optimizations
        })
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("SpiritSwap V2 swap execution not implemented".into()))
    }

    async fn is_pair_supported(&self, token_a: &str, token_b: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported
        if !self.config.chains.contains_key(chain) {
            return Ok(false);
        }

        // Basic validation
        if token_a == token_b {
            return Ok(false);
        }

        // For SpiritSwap V2, assume all valid token pairs are supported
        // In production, you might want to check factory.getPair()
        Ok(true)
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(135000) // Fantom-optimized gas estimate
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}