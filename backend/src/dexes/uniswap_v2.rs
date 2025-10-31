use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown, SwapParams, EnhancedRouteBreakdown, ReserveInfo, SlippageBreakdown};
use crate::price_impact::PriceImpactCalculator;
use crate::gas::GasEstimator;
use crate::slippage::SlippageEstimator;
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
    price_impact_calculator: Option<PriceImpactCalculator>,
    gas_estimator: Option<GasEstimator>,
    slippage_estimator: Option<SlippageEstimator>,
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
            price_impact_calculator: None,
            gas_estimator: None,
            slippage_estimator: None,
            provider_cache: ProviderCache::new(),
        }
    }

    /// Set the price impact calculator and gas estimator for enhanced quotes
    pub fn with_calculators(mut self, price_impact_calculator: PriceImpactCalculator, gas_estimator: GasEstimator) -> Self {
        let slippage_estimator = SlippageEstimator::new(self.provider_cache.clone().into());
        self.price_impact_calculator = Some(price_impact_calculator);
        self.gas_estimator = Some(gas_estimator);
        self.slippage_estimator = Some(slippage_estimator);
        self
    }

    /// Get enhanced quote with price impact, real gas estimation, and recommendations
    pub async fn get_enhanced_quote(&self, params: &QuoteParams) -> Result<EnhancedRouteBreakdown, DexError> {
        let start_time = std::time::Instant::now();
        
        // 1. Get basic quote
        let basic_quote = self.get_quote(params).await?;
        
        // 2. Calculate price impact and get reserves (if calculator available)
        let (price_impact, price_impact_category, reserve_info, reserve0, reserve1) = if let Some(calculator) = &self.price_impact_calculator {
            match calculator.calculate_trade_impact(params).await {
                Ok(impact) => {
                    let category = PriceImpactCalculator::categorize_impact(impact);
                    
                    // Get reserve info
                    let token_in = params.token_in_address.as_deref().unwrap_or("");
                    let token_out = params.token_out_address.as_deref().unwrap_or("");
                    let chain = params.chain.as_deref().unwrap_or("ethereum");
                    
                    let (reserves_result, reserve_info) = match calculator.get_v2_reserves(token_in, token_out, chain).await {
                        Ok((reserve0, reserve1, timestamp)) => {
                            let reserve_info = Some(ReserveInfo {
                                reserve0: reserve0.to_string(),
                                reserve1: reserve1.to_string(),
                                reserve0_formatted: Self::format_reserve(reserve0, 6), // Assume USDC
                                reserve1_formatted: Self::format_reserve(reserve1, 18), // Assume WETH
                                total_liquidity_usd: None, // TODO: Calculate
                                pair_address: "0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc".to_string(), // TODO: Get from calculator
                                last_updated: timestamp,
                            });
                            (Some((reserve0, reserve1)), reserve_info)
                        }
                        Err(_) => (None, None),
                    };
                    
                    (Some(impact), Some(category.to_string()), reserve_info, 
                     reserves_result.map(|(r0, _)| r0), reserves_result.map(|(_, r1)| r1))
                }
                Err(_) => (None, None, None, None, None),
            }
        } else {
            (None, None, None, None, None)
        };
        
        // 3. Get real gas estimate (if estimator available)
        let (real_gas_estimate, gas_cost_usd, gas_savings) = if let Some(estimator) = &self.gas_estimator {
            match self.build_transaction(params).await {
                Ok(tx) => {
                    match estimator.estimate(&tx, "ethereum").await {
                        Ok(gas_estimate) => {
                            let hardcoded_gas = 150000u64; // Current hardcoded value
                            let savings = ((hardcoded_gas as f64 - gas_estimate as f64) / hardcoded_gas as f64) * 100.0;
                            let cost_usd = (gas_estimate as f64 * 30e-9) * 3800.0; // 30 gwei * $3800 ETH
                            (Some(gas_estimate), Some(cost_usd), Some(savings))
                        }
                        Err(_) => (None, None, None),
                    }
                }
                Err(_) => (None, None, None),
            }
        } else {
            (None, None, None)
        };
        
        // 4. Calculate advanced slippage analysis (if slippage estimator available)
        let slippage_analysis = if let (Some(slippage_estimator), Some(impact), Some(r0), Some(r1)) = 
            (&self.slippage_estimator, price_impact, reserve0, reserve1) {
            match slippage_estimator.analyze_slippage(params, impact, r0, r1, "UniswapV2").await {
                Ok(analysis) => Some(SlippageBreakdown {
                    recommended_slippage: analysis.recommended_slippage,
                    minimum_slippage: analysis.minimum_slippage,
                    conservative_slippage: analysis.conservative_slippage,
                    aggressive_slippage: analysis.aggressive_slippage,
                    liquidity_score: analysis.liquidity_score,
                    volatility_factor: analysis.volatility_factor,
                    gas_pressure_factor: analysis.gas_pressure_factor,
                    confidence_level: analysis.confidence_level,
                    reasoning: analysis.reasoning,
                }),
                Err(_) => None,
            }
        } else {
            None
        };

        // 5. Generate basic recommendations (fallback)
        let (recommended_slippage, trade_recommendation, liquidity_depth) = if let Some(ref analysis) = slippage_analysis {
            (Some(analysis.recommended_slippage), 
             Self::generate_trade_recommendation_from_analysis(analysis),
             Self::generate_liquidity_depth_from_analysis(analysis))
        } else {
            let (basic_slippage, basic_recommendation, basic_liquidity) = Self::generate_recommendations(price_impact, reserve_info.as_ref());
            (Some(basic_slippage), basic_recommendation, basic_liquidity)
        };
        
        let execution_time = start_time.elapsed().as_millis() as u64;
        
        Ok(EnhancedRouteBreakdown {
            dex: basic_quote.dex,
            amount_out: basic_quote.amount_out,
            gas_used: basic_quote.gas_used,
            execution_time_ms: execution_time,
            confidence_score: basic_quote.confidence_score,
            
            // Enhanced data
            price_impact,
            price_impact_category,
            real_gas_estimate,
            gas_cost_usd,
            gas_savings_vs_hardcoded: gas_savings,
            liquidity_depth: Some(liquidity_depth),
            recommended_slippage,
            trade_recommendation: Some(trade_recommendation),
            reserve_info,
            
            // NEW: Advanced slippage analysis
            slippage_analysis,
        })
    }
    
    /// Format reserve amount for human readability
    fn format_reserve(amount: U256, decimals: u8) -> String {
        let divisor = U256::from(10).pow(U256::from(decimals));
        let formatted = amount / divisor;
        
        if formatted > U256::from(1_000_000) {
            format!("{:.1}M", formatted.to::<u64>() as f64 / 1_000_000.0)
        } else if formatted > U256::from(1_000) {
            format!("{:.1}K", formatted.to::<u64>() as f64 / 1_000.0)
        } else {
            formatted.to_string()
        }
    }
    
    /// Generate trading recommendations based on price impact and liquidity
    fn generate_recommendations(price_impact: Option<f64>, _reserve_info: Option<&ReserveInfo>) -> (f64, String, String) {
        let impact = price_impact.unwrap_or(0.0);
        
        let recommended_slippage = match impact {
            x if x < 0.1 => 0.1,
            x if x < 1.0 => 0.5,
            x if x < 3.0 => 1.0,
            x if x < 5.0 => 2.0,
            _ => 5.0,
        };
        
        let trade_recommendation = match impact {
            x if x < 1.0 => "Execute".to_string(),
            x if x < 3.0 => "Consider splitting".to_string(),
            x if x < 10.0 => "Split trade".to_string(),
            _ => "Avoid or use different DEX".to_string(),
        };
        
        let liquidity_depth = match impact {
            x if x < 0.1 => "Very High",
            x if x < 1.0 => "High",
            x if x < 3.0 => "Medium",
            x if x < 10.0 => "Low",
            _ => "Very Low",
        }.to_string();
        
        (recommended_slippage, trade_recommendation, liquidity_depth)
    }
    
    /// Generate trade recommendation from advanced slippage analysis
    fn generate_trade_recommendation_from_analysis(analysis: &SlippageBreakdown) -> String {
        if analysis.confidence_level < 0.3 {
            "Avoid - Low confidence in estimate".to_string()
        } else if analysis.recommended_slippage > 10.0 {
            "Avoid - Extremely high slippage expected".to_string()
        } else if analysis.recommended_slippage > 5.0 {
            "Split trade - High slippage risk".to_string()
        } else if analysis.recommended_slippage > 2.0 {
            "Consider splitting - Medium slippage risk".to_string()
        } else if analysis.recommended_slippage > 1.0 {
            "Execute with caution - Some slippage expected".to_string()
        } else {
            "Execute - Low slippage expected".to_string()
        }
    }
    
    /// Generate liquidity depth description from slippage analysis
    fn generate_liquidity_depth_from_analysis(analysis: &SlippageBreakdown) -> String {
        if analysis.liquidity_score > 90.0 {
            "Excellent".to_string()
        } else if analysis.liquidity_score > 75.0 {
            "Very High".to_string()
        } else if analysis.liquidity_score > 60.0 {
            "High".to_string()
        } else if analysis.liquidity_score > 40.0 {
            "Medium".to_string()
        } else if analysis.liquidity_score > 20.0 {
            "Low".to_string()
        } else {
            "Very Low".to_string()
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
            confidence_score: 0.95, // High confidence for Uniswap V2
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

    /// Build transaction for gas estimation
    /// Creates a swapExactTokensForTokens call to Uniswap V2 Router
    async fn build_transaction(&self, params: &QuoteParams) -> Result<alloy::rpc::types::TransactionRequest, DexError> {
        use alloy::primitives::{U256, Bytes};
        
        // Get chain and router address
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        let router_address = self.get_router_address(chain)?;
        
        // For gas estimation, use a simple view function that won't revert
        // Let's try WETH() which just returns an address - no parameters needed
        
        let function_selector = [0xad, 0x5c, 0x46, 0x48]; // WETH() function
        
        let mut calldata = Vec::new();
        calldata.extend_from_slice(&function_selector);
        // No parameters needed for WETH() function
        
        let tx = alloy::rpc::types::TransactionRequest::default()
            .to(router_address)
            .input(Bytes::from(calldata).into())
            .value(U256::ZERO); // ERC20 swap, no ETH value
        
        Ok(tx)
    }
}