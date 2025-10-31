use crate::dexes::{DexError, utils::ProviderCache};
use crate::types::QuoteParams;
use alloy::primitives::U256;
use alloy::providers::Provider;
use std::sync::Arc;

/// Advanced slippage estimation system
/// Calculates dynamic slippage based on:
/// 1. Price impact from AMM curves
/// 2. Liquidity depth analysis  
/// 3. Market conditions (gas, mempool)
/// 4. Historical volatility patterns
#[derive(Clone)]
pub struct SlippageEstimator {
    provider_cache: Arc<ProviderCache>,
}

#[derive(Debug, Clone)]
pub struct SlippageAnalysis {
    pub recommended_slippage: f64,      // Final recommended slippage %
    pub minimum_slippage: f64,          // Absolute minimum based on price impact
    pub conservative_slippage: f64,     // Conservative estimate for safety
    pub aggressive_slippage: f64,       // Aggressive estimate for speed
    pub liquidity_score: f64,           // 0-100 liquidity depth score
    pub volatility_factor: f64,         // Market volatility multiplier
    pub gas_pressure_factor: f64,       // Gas price impact on slippage
    pub confidence_level: f64,          // Confidence in the estimate (0-1)
    pub reasoning: String,              // Human-readable explanation
}

#[derive(Debug, Clone)]
pub struct LiquidityMetrics {
    pub total_liquidity_usd: f64,
    pub depth_at_1_percent: f64,       // Liquidity available within 1% price range
    pub depth_at_5_percent: f64,       // Liquidity available within 5% price range
    pub bid_ask_spread: f64,            // Effective spread
    pub market_impact_coefficient: f64, // How much price moves per $ traded
}

impl SlippageEstimator {
    pub fn new(provider_cache: Arc<ProviderCache>) -> Self {
        Self {
            provider_cache,
        }
    }

    /// Calculate comprehensive slippage analysis for a trade
    pub async fn analyze_slippage(
        &self,
        params: &QuoteParams,
        price_impact: f64,
        reserve0: U256,
        reserve1: U256,
        dex_name: &str,
    ) -> Result<SlippageAnalysis, DexError> {
        // 1. Analyze liquidity depth
        let liquidity_metrics = self.calculate_liquidity_metrics(
            params, reserve0, reserve1, dex_name
        ).await?;

        // 2. Get current market conditions
        let market_conditions = self.get_market_conditions(params).await?;

        // 3. Calculate base slippage from price impact
        let base_slippage = self.calculate_base_slippage_from_impact(price_impact);

        // 4. Apply liquidity depth adjustments
        let liquidity_adjusted = self.apply_liquidity_adjustments(
            base_slippage, &liquidity_metrics
        );

        // 5. Apply market condition adjustments
        let market_adjusted = self.apply_market_adjustments(
            liquidity_adjusted, &market_conditions
        );

        // 6. Calculate volatility factor
        let volatility_factor = self.calculate_volatility_factor(params, dex_name).await?;

        // 7. Generate final recommendations
        let analysis = self.generate_slippage_recommendations(
            market_adjusted,
            &liquidity_metrics,
            &market_conditions,
            volatility_factor,
            price_impact,
        );

        Ok(analysis)
    }

    /// Calculate liquidity depth metrics for the trading pair
    async fn calculate_liquidity_metrics(
        &self,
        params: &QuoteParams,
        reserve0: U256,
        reserve1: U256,
        dex_name: &str,
    ) -> Result<LiquidityMetrics, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Convert reserves to human readable format
        let reserve0_f64 = self.u256_to_f64_with_decimals(reserve0, params.token_in_decimals.unwrap_or(18))?;
        let reserve1_f64 = self.u256_to_f64_with_decimals(reserve1, params.token_out_decimals.unwrap_or(6))?;

        // Estimate USD values (using approximate prices)
        let (reserve0_usd, reserve1_usd) = self.estimate_reserve_usd_values(
            reserve0_f64, reserve1_f64, params, chain
        ).await?;

        let total_liquidity_usd = reserve0_usd + reserve1_usd;

        // Calculate market depth at different price levels
        let depth_1_percent = self.calculate_depth_at_price_level(
            reserve0, reserve1, 0.01, params
        )?;
        
        let depth_5_percent = self.calculate_depth_at_price_level(
            reserve0, reserve1, 0.05, params
        )?;

        // Calculate bid-ask spread (for AMMs, this is based on fee tier)
        let bid_ask_spread = self.calculate_effective_spread(dex_name);

        // Calculate market impact coefficient
        let market_impact_coefficient = self.calculate_market_impact_coefficient(
            total_liquidity_usd, reserve0, reserve1
        );

        Ok(LiquidityMetrics {
            total_liquidity_usd,
            depth_at_1_percent: depth_1_percent,
            depth_at_5_percent: depth_5_percent,
            bid_ask_spread,
            market_impact_coefficient,
        })
    }

    /// Calculate how much liquidity is available within a price range
    fn calculate_depth_at_price_level(
        &self,
        reserve0: U256,
        reserve1: U256,
        price_change_percent: f64,
        params: &QuoteParams,
    ) -> Result<f64, DexError> {
        // Using constant product formula: x * y = k
        // Calculate how much can be traded before price moves by price_change_percent
        
        let x = self.u256_to_f64_with_decimals(reserve0, params.token_in_decimals.unwrap_or(18))?;
        let y = self.u256_to_f64_with_decimals(reserve1, params.token_out_decimals.unwrap_or(6))?;
        let k = x * y;

        // Current price: y/x
        let current_price = y / x;
        let target_price = current_price * (1.0 + price_change_percent);

        // Solve for new x when price = target_price
        // target_price = (k/x_new) / x_new = k / x_new^2
        // x_new^2 = k / target_price
        let x_new = (k / target_price).sqrt();
        let tradeable_amount = x - x_new;

        Ok(tradeable_amount.max(0.0))
    }

    /// Get current market conditions affecting slippage
    async fn get_market_conditions(&self, params: &QuoteParams) -> Result<MarketConditions, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        let provider = self.provider_cache.get_provider(chain).await?;

        // Get current gas price
        let gas_price = provider.get_gas_price().await
            .map_err(|e| DexError::ContractCallFailed(format!("Failed to get gas price: {:?}", e)))?;

        let gas_price_gwei = (gas_price / 1_000_000_000) as u64;

        // Get latest block to estimate network congestion
        let latest_block = provider.get_block_number().await
            .map_err(|e| DexError::ContractCallFailed(format!("Failed to get block number: {:?}", e)))?;

        // Estimate mempool congestion (simplified - in production would use more sophisticated metrics)
        let mempool_congestion = self.estimate_mempool_congestion(gas_price_gwei);

        Ok(MarketConditions {
            gas_price_gwei,
            mempool_congestion,
            network_utilization: self.estimate_network_utilization(gas_price_gwei),
            block_number: latest_block,
        })
    }

    /// Calculate base slippage from price impact using mathematical models
    fn calculate_base_slippage_from_impact(&self, price_impact: f64) -> f64 {
        // Base slippage should be higher than price impact to account for:
        // 1. Price movement during transaction execution
        // 2. MEV/arbitrage activity
        // 3. Other traders competing for the same liquidity
        
        if price_impact < 0.01 {
            // Very low impact trades: minimal slippage needed
            price_impact * 1.5 + 0.05 // 1.5x impact + 0.05% base
        } else if price_impact < 0.1 {
            // Low impact trades: moderate slippage buffer
            price_impact * 2.0 + 0.1 // 2x impact + 0.1% base
        } else if price_impact < 1.0 {
            // Medium impact trades: higher slippage buffer
            price_impact * 2.5 + 0.2 // 2.5x impact + 0.2% base
        } else if price_impact < 5.0 {
            // High impact trades: significant slippage buffer
            price_impact * 3.0 + 0.5 // 3x impact + 0.5% base
        } else {
            // Very high impact trades: maximum protection
            price_impact * 4.0 + 1.0 // 4x impact + 1.0% base
        }
    }

    /// Apply liquidity depth adjustments to slippage
    fn apply_liquidity_adjustments(&self, base_slippage: f64, metrics: &LiquidityMetrics) -> f64 {
        let mut adjusted_slippage = base_slippage;

        // Liquidity depth factor
        let liquidity_factor = if metrics.total_liquidity_usd < 100_000.0 {
            2.5 // Very low liquidity - high slippage risk
        } else if metrics.total_liquidity_usd < 1_000_000.0 {
            1.8 // Low liquidity - moderate slippage risk
        } else if metrics.total_liquidity_usd < 10_000_000.0 {
            1.3 // Medium liquidity - some slippage risk
        } else if metrics.total_liquidity_usd < 100_000_000.0 {
            1.1 // High liquidity - low slippage risk
        } else {
            1.0 // Very high liquidity - minimal slippage risk
        };

        adjusted_slippage *= liquidity_factor;

        // Market impact coefficient adjustment
        if metrics.market_impact_coefficient > 0.001 {
            adjusted_slippage *= 1.5; // High market impact
        } else if metrics.market_impact_coefficient > 0.0001 {
            adjusted_slippage *= 1.2; // Medium market impact
        }

        // Bid-ask spread adjustment
        adjusted_slippage += metrics.bid_ask_spread * 0.5;

        adjusted_slippage
    }

    /// Apply market condition adjustments
    fn apply_market_adjustments(&self, base_slippage: f64, conditions: &MarketConditions) -> f64 {
        let mut adjusted_slippage = base_slippage;

        // Gas price factor - higher gas = more competition = more slippage
        let gas_factor = if conditions.gas_price_gwei > 100 {
            1.8 // Very high gas - extreme competition
        } else if conditions.gas_price_gwei > 50 {
            1.5 // High gas - high competition
        } else if conditions.gas_price_gwei > 30 {
            1.2 // Medium gas - moderate competition
        } else {
            1.0 // Low gas - normal competition
        };

        adjusted_slippage *= gas_factor;

        // Mempool congestion factor
        let congestion_factor = 1.0 + (conditions.mempool_congestion * 0.01);
        adjusted_slippage *= congestion_factor;

        // Network utilization factor
        let network_factor = 1.0 + (conditions.network_utilization * 0.005);
        adjusted_slippage *= network_factor;

        adjusted_slippage
    }

    /// Calculate volatility factor for the trading pair
    async fn calculate_volatility_factor(&self, params: &QuoteParams, dex_name: &str) -> Result<f64, DexError> {
        // In a full implementation, this would analyze:
        // 1. Historical price movements
        // 2. Recent trading volume patterns
        // 3. Cross-DEX price differences
        // 4. Time-of-day volatility patterns
        
        // For now, use token-specific volatility estimates
        let base_volatility = match params.token_in.as_str() {
            "ETH" | "WETH" => 1.2,  // ETH is moderately volatile
            "BTC" | "WBTC" => 1.1,  // BTC is slightly less volatile
            "USDC" | "USDT" | "DAI" => 0.8, // Stablecoins are less volatile
            _ => 1.5, // Unknown tokens are assumed more volatile
        };

        let dex_volatility = match dex_name {
            "UniswapV2" | "UniswapV3" => 1.0, // Baseline
            "SushiSwapV2" => 1.1, // Slightly more volatile
            "PancakeSwapV2" => 1.2, // More volatile on BSC
            _ => 1.3, // Smaller DEXes tend to be more volatile
        };

        Ok(base_volatility * dex_volatility)
    }

    /// Generate final slippage recommendations
    fn generate_slippage_recommendations(
        &self,
        base_slippage: f64,
        liquidity_metrics: &LiquidityMetrics,
        market_conditions: &MarketConditions,
        volatility_factor: f64,
        price_impact: f64,
    ) -> SlippageAnalysis {
        let final_slippage = base_slippage * volatility_factor;

        // Calculate different slippage levels
        let minimum_slippage = (price_impact * 1.1).max(0.05); // Just above price impact
        let conservative_slippage = final_slippage * 1.5; // 50% buffer
        let aggressive_slippage = final_slippage * 0.8; // 20% less for speed

        // Calculate confidence level
        let confidence_level = self.calculate_confidence_level(
            liquidity_metrics, market_conditions, price_impact
        );

        // Generate liquidity score
        let liquidity_score = self.calculate_liquidity_score(liquidity_metrics);

        // Generate reasoning
        let reasoning = self.generate_reasoning(
            final_slippage, liquidity_metrics, market_conditions, price_impact
        );

        SlippageAnalysis {
            recommended_slippage: final_slippage,
            minimum_slippage,
            conservative_slippage,
            aggressive_slippage,
            liquidity_score,
            volatility_factor,
            gas_pressure_factor: market_conditions.gas_price_gwei as f64 / 30.0, // Normalized to 30 gwei
            confidence_level,
            reasoning,
        }
    }

    /// Helper functions for calculations
    fn u256_to_f64_with_decimals(&self, value: U256, decimals: u8) -> Result<f64, DexError> {
        let divisor = U256::from(10).pow(U256::from(decimals));
        let result = value.checked_div(divisor)
            .ok_or_else(|| DexError::ContractCallFailed("Division overflow in u256_to_f64".into()))?;
        
        // Convert to f64 safely
        if result > U256::from(u64::MAX) {
            return Err(DexError::ContractCallFailed("Value too large for f64 conversion".into()));
        }
        
        Ok(result.to::<u64>() as f64)
    }

    async fn estimate_reserve_usd_values(
        &self,
        reserve0: f64,
        reserve1: f64,
        params: &QuoteParams,
        _chain: &str,
    ) -> Result<(f64, f64), DexError> {
        // Simplified USD estimation - in production would use price oracles
        let (token0_usd_price, token1_usd_price) = match (
            params.token_in.as_str(),
            params.token_out.as_str()
        ) {
            ("ETH" | "WETH", "USDC" | "USDT") => (3800.0, 1.0), // ETH ~$3800, USDC ~$1
            ("USDC" | "USDT", "ETH" | "WETH") => (1.0, 3800.0),
            ("BTC" | "WBTC", "USDC" | "USDT") => (70000.0, 1.0), // BTC ~$70k
            _ => (1.0, 1.0), // Default to $1 each if unknown
        };

        Ok((reserve0 * token0_usd_price, reserve1 * token1_usd_price))
    }

    fn calculate_effective_spread(&self, dex_name: &str) -> f64 {
        match dex_name {
            "UniswapV2" | "SushiSwapV2" => 0.003, // 0.3% fee
            "UniswapV3" => 0.0005, // Variable, assume 0.05% average
            "PancakeSwapV2" => 0.0025, // 0.25% fee
            _ => 0.003, // Default to 0.3%
        }
    }

    fn calculate_market_impact_coefficient(&self, total_liquidity_usd: f64, _reserve0: U256, _reserve1: U256) -> f64 {
        // Simplified market impact calculation
        if total_liquidity_usd > 0.0 {
            1.0 / total_liquidity_usd.sqrt() * 1000.0
        } else {
            1.0
        }
    }

    fn estimate_mempool_congestion(&self, gas_price_gwei: u64) -> f64 {
        // Estimate congestion based on gas price
        if gas_price_gwei > 100 {
            0.9 // 90% congestion
        } else if gas_price_gwei > 50 {
            0.6 // 60% congestion
        } else if gas_price_gwei > 30 {
            0.3 // 30% congestion
        } else {
            0.1 // 10% congestion
        }
    }

    fn estimate_network_utilization(&self, gas_price_gwei: u64) -> f64 {
        // Estimate network utilization
        (gas_price_gwei as f64 / 200.0).min(1.0)
    }

    fn calculate_confidence_level(
        &self,
        liquidity_metrics: &LiquidityMetrics,
        market_conditions: &MarketConditions,
        price_impact: f64,
    ) -> f64 {
        let mut confidence: f64 = 1.0;

        // Reduce confidence for low liquidity
        if liquidity_metrics.total_liquidity_usd < 100_000.0 {
            confidence *= 0.6;
        } else if liquidity_metrics.total_liquidity_usd < 1_000_000.0 {
            confidence *= 0.8;
        }

        // Reduce confidence for high price impact
        if price_impact > 5.0 {
            confidence *= 0.5;
        } else if price_impact > 1.0 {
            confidence *= 0.7;
        }

        // Reduce confidence for extreme gas conditions
        if market_conditions.gas_price_gwei > 100 {
            confidence *= 0.7;
        }

        confidence.max(0.1).min(1.0)
    }

    fn calculate_liquidity_score(&self, metrics: &LiquidityMetrics) -> f64 {
        let base_score = if metrics.total_liquidity_usd > 100_000_000.0 {
            95.0
        } else if metrics.total_liquidity_usd > 10_000_000.0 {
            85.0
        } else if metrics.total_liquidity_usd > 1_000_000.0 {
            70.0
        } else if metrics.total_liquidity_usd > 100_000.0 {
            50.0
        } else {
            20.0
        };

        // Adjust based on market impact
        let impact_adjustment = if metrics.market_impact_coefficient < 0.0001 {
            5.0
        } else if metrics.market_impact_coefficient < 0.001 {
            0.0
        } else {
            -10.0
        };

        {
            let result: f64 = base_score + impact_adjustment;
            result.max(0.0).min(100.0)
        }
    }

    fn generate_reasoning(
        &self,
        slippage: f64,
        liquidity_metrics: &LiquidityMetrics,
        market_conditions: &MarketConditions,
        price_impact: f64,
    ) -> String {
        let mut reasons = Vec::new();

        if price_impact > 5.0 {
            reasons.push("Very high price impact detected".to_string());
        } else if price_impact > 1.0 {
            reasons.push("High price impact".to_string());
        }

        if liquidity_metrics.total_liquidity_usd < 100_000.0 {
            reasons.push("Low liquidity pool".to_string());
        } else if liquidity_metrics.total_liquidity_usd > 10_000_000.0 {
            reasons.push("High liquidity pool".to_string());
        }

        if market_conditions.gas_price_gwei > 50 {
            reasons.push("High gas prices increase competition".to_string());
        }

        if market_conditions.mempool_congestion > 0.5 {
            reasons.push("Network congestion detected".to_string());
        }

        if reasons.is_empty() {
            format!("Recommended {:.2}% slippage based on normal market conditions", slippage)
        } else {
            format!("Recommended {:.2}% slippage due to: {}", slippage, reasons.join(", "))
        }
    }
}

#[derive(Debug, Clone)]
struct MarketConditions {
    gas_price_gwei: u64,
    mempool_congestion: f64,
    network_utilization: f64,
    block_number: u64,
}
