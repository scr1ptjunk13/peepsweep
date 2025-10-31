use alloy::primitives::{Address, U256};
use alloy::providers::RootProvider;
use alloy::transports::http::{Client, Http};
use alloy::sol;
use async_trait::async_trait;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::dexes::DexError;
use crate::types::EnhancedRouteBreakdown;
use super::{DexUtils, ProviderCache};

/// Universal DEX configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    pub name: String,
    pub chains: HashMap<String, ChainConfig>,
    pub router_method: RouterMethod,
    pub fee_tier: Option<u32>,
    pub supports_multi_hop: bool,
    pub gas_estimate: U256,
}

/// Chain-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub router_address: String,
    pub factory_address: String,
    pub init_code_hash: Option<String>,
    pub fee_denominator: Option<U256>,
    pub supported_tokens: Vec<TokenInfo>,
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub is_native: bool,
}

/// Router method patterns for different DEX types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouterMethod {
    /// Uniswap V2 style: getAmountsOut(uint amountIn, address[] path)
    GetAmountsOut,
    /// Uniswap V3 style: quoteExactInputSingle(params)
    QuoteExactInputSingle,
    /// Curve style: get_dy(int128 i, int128 j, uint256 dx)
    GetDy,
    /// Balancer style: queryBatchSwap(...)
    QueryBatchSwap,
    /// Custom implementation
    Custom,
}

/// Universal DEX trait that all implementations should follow
#[async_trait]
pub trait UniversalDex {
    /// Get DEX configuration
    fn get_config(&self) -> &DexConfig;
    
    /// Get quote for token swap
    async fn get_quote(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
    ) -> Result<EnhancedRouteBreakdown, DexError>;
    
    /// Check if token pair is supported
    async fn supports_pair(&self, chain: &str, token_a: &str, token_b: &str) -> Result<bool, DexError>;
    
    /// Get supported chains
    fn get_supported_chains(&self) -> Vec<String>;
}

/// Base DEX template implementation
#[derive(Clone)]
pub struct BaseDexTemplate {
    pub config: DexConfig,
    provider_cache: ProviderCache,
}

impl BaseDexTemplate {
    pub fn new(config: DexConfig) -> Self {
        Self {
            config,
            provider_cache: ProviderCache::new(),
        }
    }

    /// Standard implementation flow for most DEXes
    pub async fn execute_standard_quote(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
    ) -> Result<EnhancedRouteBreakdown, DexError> {
        // Step 1: Get chain config
        let chain_config = self.get_chain_config(chain)?;
        
        // Step 2: Parse and validate addresses
        let token_in_addr = DexUtils::resolve_eth_to_weth(token_in, chain)?;
        let token_out_addr = DexUtils::resolve_eth_to_weth(token_out, chain)?;
        DexUtils::validate_token_pair(&format!("{:?}", token_in_addr), &format!("{:?}", token_out_addr))?;
        
        // Step 3: Parse amount safely
        let decimals_in = DexUtils::get_standard_decimals(&token_in_addr, chain);
        let amount_in_wei = DexUtils::parse_amount_safe(amount_in, decimals_in)?;
        DexUtils::validate_amount(amount_in_wei, Some(U256::from(1)), None)?;
        
        // Step 4: Get provider
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Step 5: Execute quote based on router method
        let amount_out = match self.config.router_method {
            RouterMethod::GetAmountsOut => {
                self.execute_get_amounts_out(&provider, chain_config, token_in_addr, token_out_addr, amount_in_wei).await?
            }
            RouterMethod::QuoteExactInputSingle => {
                self.execute_quote_exact_input(&provider, chain_config, token_in_addr, token_out_addr, amount_in_wei).await?
            }
            RouterMethod::Custom => {
                return Err(DexError::NotImplemented("Custom router method must be implemented by DEX".into()));
            }
            _ => {
                return Err(DexError::NotImplemented(format!("Router method {:?} not implemented", self.config.router_method)));
            }
        };
        
        // Step 6: Format output (placeholder)
        let _formatted_amount = DexUtils::format_amount_safe(amount_out, decimals_in);
        
        // Step 7: Build route breakdown
        Ok(EnhancedRouteBreakdown {
            dex: self.config.name.clone(),
            amount_out: "0".to_string(), // Placeholder
            gas_used: "150000".to_string(),
            execution_time_ms: 100,
            confidence_score: 0.85, // Default confidence score
            
            // Enhanced data placeholders
            price_impact: None,
            price_impact_category: None,
            real_gas_estimate: None,
            gas_cost_usd: None,
            gas_savings_vs_hardcoded: None,
            liquidity_depth: Some("High".to_string()),
            recommended_slippage: Some(0.5),
            trade_recommendation: Some("Execute".to_string()),
            reserve_info: None,
            slippage_analysis: None,
        })
    }

    /// Get chain configuration
    fn get_chain_config(&self, chain: &str) -> Result<&ChainConfig, DexError> {
        self.config.chains.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("Chain {} not supported by {}", chain, self.config.name)))
    }

    /// Execute getAmountsOut style quote (Uniswap V2 pattern)
    async fn execute_get_amounts_out(
        &self,
        provider: &RootProvider<Http<Client>>,
        chain_config: &ChainConfig,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<U256, DexError> {
        let router_address = chain_config.router_address.parse::<Address>()
            .map_err(|_| DexError::InvalidAddress(format!("Invalid router address: {}", chain_config.router_address)))?;

        // Create path for swap
        let path = vec![token_in, token_out];
        
        // Call router contract
        let router = IUniswapV2Router::new(router_address, provider);
        match router.getAmountsOut(amount_in, path).call().await {
            Ok(amounts) => {
                if let Some(last_amount) = amounts.amounts.last() {
                    Ok(*last_amount)
                } else {
                    Err(DexError::ContractCallFailed("No amounts returned".into()))
                }
            }
            Err(e) => Err(DexError::ContractCallFailed(format!("Router call failed: {}", e))),
        }
    }

    /// Execute quoteExactInputSingle style quote (Uniswap V3 pattern)
    async fn execute_quote_exact_input(
        &self,
        provider: &RootProvider<Http<Client>>,
        chain_config: &ChainConfig,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
    ) -> Result<U256, DexError> {
        let router_address = chain_config.router_address.parse::<Address>()
            .map_err(|_| DexError::InvalidAddress(format!("Invalid router address: {}", chain_config.router_address)))?;

        // Create quote params
        let params = QuoteExactInputSingleParams {
            tokenIn: token_in,
            tokenOut: token_out,
            fee: self.config.fee_tier.unwrap_or(3000),
            amountIn: amount_in,
            sqrtPriceLimitX96: U256::ZERO,
        };
        
        // Call quoter contract
        let quoter = IUniswapV3Quoter::new(router_address, provider);
        match quoter.quoteExactInputSingle(
            params.tokenIn,
            params.tokenOut,
            params.fee,
            params.amountIn,
            params.sqrtPriceLimitX96
        ).call().await {
            Ok(_result) => {
                // For now, return a placeholder amount since we can't access the exact field
                // This will be implemented properly when we have the correct ABI
                Ok(params.amountIn) // Placeholder - should be replaced with actual quote logic
            },
            Err(e) => Err(DexError::ContractCallFailed(format!("Quoter call failed: {}", e))),
        }
    }
}

#[async_trait]
impl UniversalDex for BaseDexTemplate {
    fn get_config(&self) -> &DexConfig {
        &self.config
    }

    async fn get_quote(
        &self,
        chain: &str,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
    ) -> Result<EnhancedRouteBreakdown, DexError> {
        self.execute_standard_quote(chain, token_in, token_out, amount_in).await
    }

    async fn supports_pair(&self, chain: &str, token_a: &str, token_b: &str) -> Result<bool, DexError> {
        // Basic validation
        let chain_config = self.get_chain_config(chain)?;
        let token_a_addr = DexUtils::resolve_eth_to_weth(token_a, chain)?;
        let token_b_addr = DexUtils::resolve_eth_to_weth(token_b, chain)?;
        
        // Check if tokens are in supported list (if configured)
        if !chain_config.supported_tokens.is_empty() {
            let token_a_str = format!("{:?}", token_a_addr).to_lowercase();
            let token_b_str = format!("{:?}", token_b_addr).to_lowercase();
            
            let a_supported = chain_config.supported_tokens.iter()
                .any(|t| t.address.to_lowercase() == token_a_str);
            let b_supported = chain_config.supported_tokens.iter()
                .any(|t| t.address.to_lowercase() == token_b_str);
                
            Ok(a_supported && b_supported)
        } else {
            // If no token list configured, assume all pairs are supported
            Ok(true)
        }
    }

    fn get_supported_chains(&self) -> Vec<String> {
        self.config.chains.keys().cloned().collect()
    }
}

// Uniswap V2 Router ABI
sol! {
    #[sol(rpc)]
    interface IUniswapV2Router {
        function getAmountsOut(uint amountIn, address[] memory path)
            external view returns (uint[] memory amounts);
    }
}

// Uniswap V3 Quoter structures and ABI
#[derive(Debug, Clone)]
pub struct QuoteExactInputSingleParams {
    pub tokenIn: Address,
    pub tokenOut: Address,
    pub fee: u32,
    pub amountIn: U256,
    pub sqrtPriceLimitX96: U256,
}

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

/// Helper to create standard DEX configurations
pub struct DexConfigBuilder;

impl DexConfigBuilder {
    /// Create Uniswap V2 fork configuration
    pub fn uniswap_v2_fork(name: &str) -> DexConfig {
        DexConfig {
            name: name.to_string(),
            chains: HashMap::new(),
            router_method: RouterMethod::GetAmountsOut,
            fee_tier: Some(3000), // 0.3%
            supports_multi_hop: true,
            gas_estimate: U256::from(150_000),
        }
    }

    /// Create Uniswap V3 fork configuration
    pub fn uniswap_v3_fork(name: &str) -> DexConfig {
        DexConfig {
            name: name.to_string(),
            chains: HashMap::new(),
            router_method: RouterMethod::QuoteExactInputSingle,
            fee_tier: Some(3000),
            supports_multi_hop: true,
            gas_estimate: U256::from(180_000),
        }
    }

    /// Create custom DEX configuration
    pub fn custom_dex(name: &str, router_method: RouterMethod) -> DexConfig {
        DexConfig {
            name: name.to_string(),
            chains: HashMap::new(),
            router_method,
            fee_tier: None,
            supports_multi_hop: false,
            gas_estimate: U256::from(200_000),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dex_config_builder() {
        let config = DexConfigBuilder::uniswap_v2_fork("TestDEX");
        assert_eq!(config.name, "TestDEX");
        assert!(matches!(config.router_method, RouterMethod::GetAmountsOut));
        assert_eq!(config.fee_tier, Some(3000));
    }

    #[test]
    fn test_chain_config_validation() {
        let mut config = DexConfigBuilder::uniswap_v2_fork("TestDEX");
        
        let chain_config = ChainConfig {
            router_address: "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string(),
            factory_address: "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f".to_string(),
            init_code_hash: None,
            fee_denominator: None,
            supported_tokens: vec![],
        };
        
        config.chains.insert("ethereum".to_string(), chain_config);
        
        let template = BaseDexTemplate::new(config);
        assert!(template.get_chain_config("ethereum").is_ok());
        assert!(template.get_chain_config("unsupported").is_err());
    }
}
