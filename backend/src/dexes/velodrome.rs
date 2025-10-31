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
        tracing::info!("🔄 Velodrome quote request: {} {} -> {} on {}", 
            params.amount_in, params.token_in, params.token_out, 
            params.chain.as_deref().unwrap_or("unknown"));
        
        let chain = params.chain.as_ref().ok_or_else(|| {
            tracing::error!("❌ Velodrome: Missing chain in params");
            DexError::InvalidInput("Missing chain".to_string())
        })?;
        
        // Get token addresses with detailed logging
        let token_in_addr = match self.get_token_address(&params.token_in, chain) {
            Ok(addr) => {
                tracing::debug!("✅ Token IN resolved: {} -> {}", params.token_in, addr);
                addr
            }
            Err(e) => {
                tracing::error!("❌ Failed to resolve token IN {}: {:?}", params.token_in, e);
                return Err(e);
            }
        };
        
        let token_out_addr = match self.get_token_address(&params.token_out, chain) {
            Ok(addr) => {
                tracing::debug!("✅ Token OUT resolved: {} -> {}", params.token_out, addr);
                addr
            }
            Err(e) => {
                tracing::error!("❌ Failed to resolve token OUT {}: {:?}", params.token_out, e);
                return Err(e);
            }
        };
        
        // Parse amount safely with logging
        let amount_in_wei = match DexUtils::parse_amount_safe(&params.amount_in, params.token_in_decimals.unwrap_or(18)) {
            Ok(amount) => {
                tracing::debug!("✅ Amount parsed: {} -> {} wei", params.amount_in, amount);
                amount
            }
            Err(e) => {
                tracing::error!("❌ Failed to parse amount {}: {:?}", params.amount_in, e);
                return Err(e);
            }
        };
        
        // Real contract call - NO FALLBACK
        match self.call_velodrome_router(chain, &token_in_addr, &token_out_addr, amount_in_wei).await {
            Ok(result) => {
                tracing::info!("✅ Velodrome quote successful: {} wei output", result);
                Ok(result)
            }
            Err(e) => {
                tracing::error!("❌ Velodrome contract call failed: {:?}", e);
                Err(e)
            }
        }
    }

    fn get_token_address(&self, token_symbol: &str, chain: &str) -> Result<String, DexError> {
        let address = match (chain, token_symbol.to_uppercase().as_str()) {
            // Optimism addresses
            ("optimism", "USDC") => "0x7F5c764cBc14f9669B88837ca1490cCa17c31607",
            ("optimism", "USDT") => "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58", 
            ("optimism", "DAI") => "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
            ("optimism", "WETH") => "0x4200000000000000000000000000000000000006",
            ("optimism", "ETH") => "0x4200000000000000000000000000000000000006",
            ("optimism", "OP") => "0x4200000000000000000000000000000000000042",
            ("optimism", "VELO") => "0x3c8B650257cFb5f272f799F5e2b4e65093a11a05",
            ("optimism", "WBTC") => "0x68f180fcCe6836688e9084f035309E29Bf0A2095",
            
            // Base addresses  
            ("base", "USDC") => "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            ("base", "USDbC") => "0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA", 
            ("base", "WETH") => "0x4200000000000000000000000000000000000006",
            ("base", "ETH") => "0x4200000000000000000000000000000000000006",
            ("base", "cbETH") => "0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22",
            ("base", "AERO") => "0x940181a94A35A4569E4529A3CDfB74e38FD98631",
            ("base", "DAI") => "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb",
            
            _ => return Err(DexError::InvalidPair(format!("Token {} not supported on {}", token_symbol, chain)))
        };
        
        Ok(address.to_string())
    }

    async fn call_velodrome_router(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: U256,
    ) -> Result<U256, DexError> {
        let chain_config = self.get_chain_config(chain)?;
        
        // Parse addresses
        let token_in_addr = Address::from_str(token_in)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token_in address: {}", token_in)))?;
        let token_out_addr = Address::from_str(token_out)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token_out address: {}", token_out)))?;
        let router_addr = Address::from_str(&chain_config.router_address)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid router address: {}", chain_config.router_address)))?;
        let factory_addr = Address::from_str(&chain_config.factory_address)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid factory address: {}", chain_config.factory_address)))?;

        // Get provider
        let provider = self.provider_cache.get_provider(chain).await?;

        // Generate route strategies (direct + WETH routing)
        let weth_addr = match chain {
            "optimism" | "base" => "0x4200000000000000000000000000000000000006",
            _ => return Err(DexError::UnsupportedChain(format!("Chain {} not supported", chain)))
        };
        
        let route_strategies = self.generate_routes(token_in_addr, token_out_addr, factory_addr, weth_addr);

        // Try each route strategy
        for routes in route_strategies {
            match self.call_router_with_routes(&provider, router_addr, amount_in, routes).await {
                Ok(amount_out) if amount_out > U256::ZERO => {
                    return Ok(amount_out);
                }
                Ok(_) => continue, // Zero output, try next route
                Err(e) => {
                    tracing::debug!("Route failed: {:?}", e);
                    continue;
                }
            }
        }

        Err(DexError::UnsupportedPair("No viable Velodrome route found".into()))
    }

    async fn call_router_with_routes(
        &self,
        provider: &alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>,
        router_address: Address,
        amount_in: U256,
        routes: Vec<Route>,
    ) -> Result<U256, DexError> {
        // Create contract instance using #[sol(rpc)]
        let router = IVelodromeRouter::new(router_address, provider);

        // Call getAmountsOut - Alloy handles ABI encoding/decoding
        match router.getAmountsOut(amount_in, routes).call().await {
            Ok(amounts) => {
                if let Some(last_amount) = amounts.amounts.last() {
                    Ok(*last_amount)
                } else {
                    Ok(U256::ZERO)
                }
            }
            Err(e) => {
                tracing::debug!("Router call failed: {:?}", e);
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
}

#[async_trait]
impl DexIntegration for VelodromeDex {
    fn get_name(&self) -> &str {
        "Velodrome"
    }

    fn get_supported_chains(&self) -> Vec<&str> {
        vec!["optimism", "base"]
    }

    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync> {
        Box::new(self.clone())
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let amount_out_wei = self.get_velodrome_quote(params).await?;
        let decimals_out = params.token_out_decimals.unwrap_or(18);
        let amount_out = DexUtils::format_amount_safe(amount_out_wei, decimals_out);

        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out,
            gas_used: "165000".to_string(), // Velodrome gas usage
            confidence_score: 0.86, // Good confidence for Velodrome
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

    async fn execute_swap(&self, _params: &SwapParams) -> Result<String, DexError> {
        // For now, return a placeholder transaction hash
        // In production, this would interact with the Velodrome router contract
        Err(DexError::NotImplemented("Swap execution not yet implemented for Velodrome".to_string()))
    }

    async fn get_gas_estimate(&self, _params: &SwapParams) -> Result<u64, DexError> {
        // Return estimated gas for Velodrome swaps
        // Based on typical Velodrome v2 swap gas usage
        Ok(self.config.gas_estimate.to::<u64>())
    }

}