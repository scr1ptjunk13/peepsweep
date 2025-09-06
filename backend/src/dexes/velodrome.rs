use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown};
use crate::dexes::utils::{
    dex_template::{DexConfig, ChainConfig, RouterMethod, DexConfigBuilder},
    DexUtils, ProviderCache
};
use async_trait::async_trait;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, RootProvider},
    transports::http::{Client, Http},
    sol,
};
use std::str::FromStr;
use std::collections::HashMap;
use tracing::{debug, error};

// Velodrome V2 ABI - Using Universal DEX Framework
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
    interface IVelodromeRouter {
        function getAmountsOut(uint256 amountIn, Route[] memory routes)
            external view returns (uint256[] memory amounts);
    }
}

#[derive(Clone)]
pub struct VelodromeDex {
    config: DexConfig,
    provider_cache: ProviderCache,
}

impl VelodromeDex {
    pub fn new() -> Self {
        let mut config = DexConfigBuilder::custom_dex("Velodrome", RouterMethod::Custom);
        config.gas_estimate = U256::from(150_000);
        config.supports_multi_hop = true;
        
        // Optimism Velodrome V2 configuration
        let optimism_config = ChainConfig {
            router_address: "0xa062ae8a9c5e11aaa026fc2670b0d65ccc8b2858".to_string(),
            factory_address: "0xF1046053aa5682b4F9a81b5481394DA16BE5FF5a".to_string(),
            init_code_hash: None,
            fee_denominator: None,
            supported_tokens: vec![], // Will be populated dynamically
        };
        
        // Base Aerodrome configuration
        let base_config = ChainConfig {
            router_address: "0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43".to_string(),
            factory_address: "0x420DD381b31aEf6683db6B902084cB0FFECe40Da".to_string(),
            init_code_hash: None,
            fee_denominator: None,
            supported_tokens: vec![],
        };
        
        config.chains.insert("optimism".to_string(), optimism_config);
        config.chains.insert("base".to_string(), base_config);

        Self { 
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    async fn get_velodrome_quote(&self, params: &QuoteParams) -> Result<U256, DexError> {
        let chain = params.chain.as_deref().unwrap_or("optimism");
        let chain_config = self.get_chain_config(chain)?;

        // Use standardized address resolution
        let token_in_addr = params.token_in_address.as_deref().unwrap_or("0x0000000000000000000000000000000000000000");
        let token_out_addr = params.token_out_address.as_deref().unwrap_or("0x0000000000000000000000000000000000000000");
        let token_in = DexUtils::resolve_eth_to_weth(token_in_addr, chain)?;
        let token_out = DexUtils::resolve_eth_to_weth(token_out_addr, chain)?;
        DexUtils::validate_token_pair(&format!("{:?}", token_in), &format!("{:?}", token_out))?;

        // Use standardized amount parsing
        let decimals_in = params.token_in_decimals.unwrap_or(18);
        let amount_in = DexUtils::parse_amount_safe(&params.amount_in, decimals_in)?;
        DexUtils::validate_amount(amount_in, Some(U256::from(1)), None)?;

        // Get provider using cache
        let provider = self.provider_cache.get_provider(chain).await?;
        let router_address = chain_config.router_address.parse::<Address>()
            .map_err(|_| DexError::InvalidAddress("Invalid router address".into()))?;
        let factory_address = chain_config.factory_address.parse::<Address>()
            .map_err(|_| DexError::InvalidAddress("Invalid factory address".into()))?;

        // Try different route strategies
        let weth_addr = DexUtils::get_weth_address(chain)?;
        let route_strategies = self.generate_routes(token_in, token_out, factory_address, &weth_addr);

        for routes in route_strategies {
            match self.call_router(&provider, router_address, amount_in, routes).await {
                Ok(amount_out) if amount_out > U256::ZERO => {
                    debug!("✅ Velodrome route found, output: {}", amount_out);
                    return Ok(amount_out);
                }
                Ok(_) => continue, // Zero output, try next route
                Err(e) => {
                    debug!("Route failed: {:?}", e);
                    continue;
                }
            }
        }

        Err(DexError::UnsupportedPair("No viable Velodrome route found".into()))
    }

    // Use Alloy's proper contract instantiation with #[sol(rpc)]
    async fn call_router(
        &self,
        provider: &RootProvider<Http<Client>>,
        router_address: Address,
        amount_in: U256,
        routes: Vec<Route>,
    ) -> Result<U256, DexError> {
        // Create contract instance - now works with #[sol(rpc)] attribute
        let router = IVelodromeRouter::new(router_address, provider);

        // Call getAmountsOut - Alloy handles all ABI encoding/decoding
        match router.getAmountsOut(amount_in, routes).call().await {
            Ok(amounts) => {
                if let Some(last_amount) = amounts.amounts.last() {
                    Ok(*last_amount)
                } else {
                    Ok(U256::ZERO)
                }
            }
            Err(e) => {
                debug!("Router call failed: {:?}", e);
                Err(DexError::ContractCallFailed("Router call failed".into()))
            }
        }
    }

    fn generate_routes(
        &self,
        token_in: Address,
        token_out: Address,
        factory: Address,
        weth: &str,
    ) -> Vec<Vec<Route>> {
        let mut routes = Vec::new();

        // Direct routes (try both stable and volatile)
        routes.push(vec![Route { from: token_in, to: token_out, stable: false, factory }]);
        routes.push(vec![Route { from: token_in, to: token_out, stable: true, factory }]);

        // WETH routing (if not already involving WETH)
        if let Ok(weth_addr) = Address::from_str(weth) {
            if token_in != weth_addr && token_out != weth_addr {
                // Volatile WETH routing
                routes.push(vec![
                    Route { from: token_in, to: weth_addr, stable: false, factory },
                    Route { from: weth_addr, to: token_out, stable: false, factory },
                ]);
                
                // Mixed stable/volatile through WETH
                routes.push(vec![
                    Route { from: token_in, to: weth_addr, stable: true, factory },
                    Route { from: weth_addr, to: token_out, stable: false, factory },
                ]);
            }
        }

        routes
    }

    fn get_chain_config(&self, chain: &str) -> Result<&ChainConfig, DexError> {
        self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain {} not supported by Velodrome", chain)))
    }
    // Removed - using DexUtils::parse_amount_safe instead

    // Removed - using DexUtils::format_amount_safe instead
}

#[async_trait]
impl DexIntegration for VelodromeDex {
    fn get_name(&self) -> &'static str {
        "Velodrome"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_velodrome_quote(params).await?;
        let decimals_out = params.token_out_decimals.unwrap_or(18);
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, decimals_out);

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: self.config.gas_estimate.to_string(),
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Use standardized pair support validation
        let _chain_config = self.get_chain_config(chain)?;
        let token_in_addr = DexUtils::resolve_eth_to_weth(token_in, chain)?;
        let token_out_addr = DexUtils::resolve_eth_to_weth(token_out, chain)?;
        
        // Validate token pair
        DexUtils::validate_token_pair(&format!("{:?}", token_in_addr), &format!("{:?}", token_out_addr))?;
        
        // For Velodrome, we support most pairs - let quote attempts determine actual support
        Ok(true)
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        // Return static chain list for Velodrome
        vec!["optimism", "base"]
    }
}