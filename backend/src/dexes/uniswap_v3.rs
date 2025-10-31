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

// Uniswap V3 Quoter V1 ABI
sol! {
    #[sol(rpc)]
    interface IUniswapV3Quoter {
        function quoteExactInputSingle(
            address tokenIn,
            address tokenOut,
            uint24 fee,
            uint256 amountIn,
            uint160 sqrtPriceLimitX96
        ) external returns (uint256 amountOut);
    }
}

#[derive(Clone)]
pub struct UniswapV3Dex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl UniswapV3Dex {
    pub fn new() -> Self {
        // Use framework builder - massive code reduction!
        let mut config = DexConfigBuilder::uniswap_v3_fork("UniswapV3");
        
        // Define chain data in compact config format
        let chains = [
            ("ethereum", "0xE592427A0AEce92De3Edee1F18E0157C05861564", "0x1F98431c8aD98523631AE4a59f267346ea31F984"),
            ("arbitrum", "0xE592427A0AEce92De3Edee1F18E0157C05861564", "0x1F98431c8aD98523631AE4a59f267346ea31F984"),
            ("optimism", "0xE592427A0AEce92De3Edee1F18E0157C05861564", "0x1F98431c8aD98523631AE4a59f267346ea31F984"),
            ("base", "0x2626664c2603336E57B271c5C0b26F421741e481", "0x33128a8fC17869897dcE68Ed026d694621f6FDfD"),
        ];

        // Auto-populate chain configs - 90% less code!
        for (chain, router, factory) in chains {
            config.chains.insert(chain.to_string(), ChainConfig {
                router_address: router.to_string(),
                factory_address: factory.to_string(),
                init_code_hash: None,
                fee_denominator: None,
                supported_tokens: vec![], // Auto-discovery via framework
            });
        }

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_uniswap_v3_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Handle ETH → WETH conversion like SushiSwap
        let mut token_in_addr_str = params.token_in_address.as_deref().unwrap_or("");
        let mut token_out_addr_str = params.token_out_address.as_deref().unwrap_or("");
        
        // Convert ETH to WETH for Uniswap V3
        let weth_addr = match chain {
            "ethereum" => "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
            "polygon" => "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270", // WMATIC
            "arbitrum" => "0x82af49447d8a07e3bd95bd0d56f35241523fbab1",
            "base" => "0x4200000000000000000000000000000000000006",
            "optimism" => "0x4200000000000000000000000000000000000006",
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
        
        // Get quoter address from config (could be extended to support per-chain quoters)
        let quoter_address = self.get_quoter_address(chain)?;

        // V3 fee tier optimization - could be moved to config
        let fee_tiers = [100u32, 500, 3000, 10000];
        let mut best_output = U256::ZERO;
        let mut best_fee = 0u32;

        for fee in fee_tiers {
            if let Ok(amount_out) = self.call_quoter_with_fee(&provider, quoter_address, token_in_addr, token_out_addr, fee, amount_in_wei).await {
                if amount_out > best_output {
                    best_output = amount_out;
                    best_fee = fee;
                }
            }
        }

        if best_output > U256::ZERO {
            info!("✅ Uniswap V3 best route: fee {} bps, output: {}", best_fee, best_output);
            Ok(best_output)
        } else {
            Err(DexError::UnsupportedPair("No valid V3 pool found".into()))
        }
    }

    async fn call_quoter_with_fee(
        &self,
        provider: &alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>,
        quoter_address: Address,
        token_in: Address,
        token_out: Address,
        fee: u32,
        amount_in: U256,
    ) -> Result<U256, DexError> {
        let quoter = IUniswapV3Quoter::new(quoter_address, provider);

        match quoter.quoteExactInputSingle(
            token_in,
            token_out,
            fee,
            amount_in,
            U256::ZERO
        ).call().await {
            Ok(result) => Ok(result.amountOut),
            Err(e) => {
                debug!("Quoter call failed for fee {}: {:?}", fee, e);
                Err(DexError::ContractCallFailed(format!("No pool for fee tier {}", fee)))
            }
        }
    }

    fn get_quoter_address(&self, _chain: &str) -> Result<Address, DexError> {
        // Use same Quoter V1 address as existing working files
        let quoter_addr = "0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6"; // Quoter V1
        Address::from_str(quoter_addr)
            .map_err(|_| DexError::ConfigError("Invalid quoter address".into()))
    }
}

#[async_trait]
impl DexIntegration for UniswapV3Dex {
    fn get_name(&self) -> &'static str {
        "UniswapV3"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_uniswap_v3_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "200000".to_string(), // V3 can be more gas intensive due to complexity
            confidence_score: 0.90, // High confidence for Uniswap V3
        })
    }

    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, chain: &str) -> Result<bool, DexError> {
        Ok(self.config.chains.contains_key(chain))
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("Swap execution not yet implemented for UniswapV3".to_string()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(180_000)
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}
