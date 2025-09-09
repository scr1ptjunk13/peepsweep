use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown, SwapParams};
use crate::dexes::utils::{
    dex_template::{DexConfig, ChainConfig, RouterMethod, DexConfigBuilder},
    DexUtils, ProviderCache
};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256},
    sol,
};
use std::str::FromStr;
use tracing::{debug, info};

// Aerodrome Router ABI - Same as Velodrome
sol! {
    #[derive(Debug)]
    struct Route {
        address from;
        address to;
        bool stable;
        address factory;
    }

    #[derive(Debug)]
    #[sol(rpc)]
    interface IAerodromeRouter {
        function getAmountsOut(uint256 amountIn, Route[] memory routes)
            external view returns (uint256[] memory amounts);
    }
}

#[derive(Clone)]
pub struct AerodromeDex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl AerodromeDex {
    pub fn new() -> Self {
        let mut config = DexConfigBuilder::custom_dex("Aerodrome", RouterMethod::Custom);
        config.gas_estimate = U256::from(150_000);
        config.supports_multi_hop = true;
        
        // Base Aerodrome configuration
        let base_config = ChainConfig {
            router_address: "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43".to_string(),
            factory_address: "0x420DD381b31aEf6683db6B902084cB0FFECe40Da".to_string(),
            init_code_hash: None,
            fee_denominator: Some(U256::from(10000)), // Variable fees (0-3%)
            supported_tokens: vec![], // Auto-discovery via framework
        };
        
        config.chains.insert("base".to_string(), base_config);

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_aerodrome_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("base");
        
        // Handle ETH native token conversion to WETH
        let (token_in_addr, token_out_addr) = self.handle_native_tokens(params)?;
        
        let amount_in_wei = DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;

        // Framework provider management
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Get router address from config
        let router_address = self.get_router_address(chain)?;

        // Determine if this should be a stable or volatile pool
        let is_stable = self.is_stable_pair(&params.token_in, &params.token_out);
        
        // Get factory address
        let factory_address = self.get_factory_address(chain)?;

        // Create route for Aerodrome
        let route = Route {
            from: token_in_addr,
            to: token_out_addr,
            stable: is_stable,
            factory: factory_address,
        };

        // Call getAmountsOut on router
        let router = IAerodromeRouter::new(router_address, &provider);
        
        match router.getAmountsOut(amount_in_wei, vec![route]).call().await {
            Ok(result) => {
                let amounts = result.amounts;
                if amounts.len() >= 2 {
                    let amount_out = amounts[amounts.len() - 1];
                    info!("✅ Aerodrome quote successful: output: {}", amount_out);
                    Ok(amount_out)
                } else {
                    Err(DexError::InvalidResponse("Invalid amounts array length".into()))
                }
            }
            Err(e) => {
                debug!("Aerodrome getAmountsOut failed: {:?}", e);
                Err(DexError::ContractCallFailed("No Aerodrome liquidity available".into()))
            }
        }
    }

    fn get_router_address(&self, chain: &str) -> Result<Address, DexError> {
        let chain_config = self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain not supported: {}", chain)))?;
        
        Address::from_str(&chain_config.router_address)
            .map_err(|_| DexError::ConfigError("Invalid router address".into()))
    }

    fn get_factory_address(&self, chain: &str) -> Result<Address, DexError> {
        let chain_config = self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain not supported: {}", chain)))?;
        
        Address::from_str(&chain_config.factory_address)
            .map_err(|_| DexError::ConfigError("Invalid factory address".into()))
    }

    /// Determine if a token pair should use stable or volatile pool
    fn is_stable_pair(&self, token_in: &str, token_out: &str) -> bool {
        let stablecoins = ["USDC", "USDT", "DAI", "FRAX", "LUSD"];
        
        let token_in_stable = stablecoins.iter().any(|&stable| token_in.contains(stable));
        let token_out_stable = stablecoins.iter().any(|&stable| token_out.contains(stable));
        
        // Both tokens must be stablecoins for stable pool
        token_in_stable && token_out_stable
    }

    /// Handle native ETH conversion to WETH for Aerodrome compatibility
    fn handle_native_tokens(&self, params: &QuoteParams) -> Result<(Address, Address), DexError> {
        let mut token_in_addr = params.token_in_address.as_deref().unwrap_or("");
        let mut token_out_addr = params.token_out_address.as_deref().unwrap_or("");
        
        // Convert native ETH to WETH for Base
        let base_weth = "0x4200000000000000000000000000000000000006";
        
        // Handle ETH -> WETH conversion
        if token_in_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_in_addr = base_weth;
            debug!("Converted native ETH input to WETH: {}", base_weth);
        }
        
        if token_out_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            token_out_addr = base_weth;
            debug!("Converted native ETH output to WETH: {}", base_weth);
        }
        
        let parsed_in = DexUtils::parse_token_address(&Some(token_in_addr.to_string()), "token_in")?;
        let parsed_out = DexUtils::parse_token_address(&Some(token_out_addr.to_string()), "token_out")?;
        
        Ok((parsed_in, parsed_out))
    }
}

#[async_trait]
impl DexIntegration for AerodromeDex {
    fn get_name(&self) -> &str {
        "Aerodrome"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_aerodrome_quote(params).await?;
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, params.token_out_decimals.unwrap_or(18));

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "150000".to_string(), // Base optimized gas
        })
    }

    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, chain: &str) -> Result<bool, DexError> {
        Ok(self.config.chains.contains_key(chain))
    }

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        Err(DexError::NotImplemented("Aerodrome execution not implemented".into()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        Ok(150_000) // Base chain optimized gas estimate
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        self.config.chains.keys().map(|s| s.as_str()).collect()
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }
}