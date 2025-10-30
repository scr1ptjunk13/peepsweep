use alloy::primitives::U256;
use std::sync::Arc;
use crate::dexes::{DexError, utils::ProviderCache};
use crate::types::QuoteParams;

/// Price impact calculator for Uniswap V2 style AMMs
/// Single file implementation (~150 lines) as per implementation plan
#[derive(Clone)]
pub struct PriceImpactCalculator {
    provider_cache: Arc<ProviderCache>,
}

impl PriceImpactCalculator {
    pub fn new(provider_cache: Arc<ProviderCache>) -> Self {
        Self { provider_cache }
    }

    /// Calculate price impact for Uniswap V2 style AMM
    /// Formula: (Execution Price - Market Price) / Market Price × 100
    /// Based on research from RareSkills and DailyDeFi
    pub fn calculate_v2_impact(
        amount_in: U256,
        reserve_in: U256,
        reserve_out: U256,
    ) -> Result<f64, DexError> {
        // Validate reserves
        if reserve_in.is_zero() || reserve_out.is_zero() {
            return Err(DexError::InvalidAmount("Reserves cannot be zero".into()));
        }

        // Apply 0.3% fee (997/1000) - only on amount going in
        let amount_in_with_fee = (amount_in * U256::from(997)) / U256::from(1000);
        
        // Calculate amount out using x*y=k formula
        let numerator = amount_in_with_fee * reserve_out;
        let denominator = reserve_in + amount_in_with_fee;
        
        if denominator.is_zero() {
            return Err(DexError::InvalidAmount("Invalid denominator in AMM calculation".into()));
        }
        
        let amount_out = numerator / denominator;
        
        // Convert to f64 for calculations
        let amount_in_f64 = Self::u256_to_f64(amount_in)?;
        let amount_out_f64 = Self::u256_to_f64(amount_out)?;
        let reserve_in_f64 = Self::u256_to_f64(reserve_in)?;
        let reserve_out_f64 = Self::u256_to_f64(reserve_out)?;
        
        // Market price = how much token_out you get per token_in at current ratio
        let market_price = reserve_out_f64 / reserve_in_f64;
        
        // Execution price = how much token_out you actually get per token_in in this trade
        let execution_price = amount_out_f64 / amount_in_f64;
        
        // Price impact = (market_price - execution_price) / market_price * 100
        // This shows how much worse the execution price is compared to market price
        
        if market_price == 0.0 {
            return Err(DexError::InvalidAmount("Market price cannot be zero".into()));
        }
        
        let price_impact = ((market_price - execution_price) / market_price * 100.0).abs();
        
        Ok(price_impact)
    }

    /// Get reserves for a Uniswap V2 pair
    /// Returns (reserve0, reserve1, timestamp)
    pub async fn get_v2_reserves(
        &self,
        token0: &str,
        token1: &str,
        chain: &str,
    ) -> Result<(U256, U256, u32), DexError> {
        // For now, return mock reserves for testing
        // TODO: Implement actual pair contract calls
        
        // Mock reserves that represent realistic ETH/USDC pool
        // ~1000 ETH and ~3.7M USDC (price ~$3700/ETH)
        let reserve_eth = U256::from(1000) * U256::from(10).pow(U256::from(18)); // 1000 ETH
        let reserve_usdc = U256::from(3700000) * U256::from(10).pow(U256::from(6)); // 3.7M USDC
        
        // Return reserves in correct order based on token addresses
        if token0 < token1 {
            Ok((reserve_eth, reserve_usdc, 1698765432)) // Mock timestamp
        } else {
            Ok((reserve_usdc, reserve_eth, 1698765432)) // Mock timestamp
        }
    }

    /// Calculate price impact for a specific trade
    pub async fn calculate_trade_impact(
        &self,
        params: &QuoteParams,
    ) -> Result<f64, DexError> {
        let chain = params.chain.as_deref().unwrap_or("ethereum");
        
        // Parse token addresses
        let token_in = params.token_in_address.as_ref()
            .ok_or_else(|| DexError::InvalidAddress("Missing token in address".into()))?;
        let token_out = params.token_out_address.as_ref()
            .ok_or_else(|| DexError::InvalidAddress("Missing token out address".into()))?;
        
        // Parse amount
        let amount_in = Self::parse_amount(&params.amount_in, params.token_in_decimals.unwrap_or(18))?;
        
        // Get reserves
        let (reserve0, reserve1, _timestamp) = self.get_v2_reserves(token_in, token_out, chain).await?;
        
        // Determine which reserve is which based on token order
        let (reserve_in, reserve_out) = if token_in < token_out {
            (reserve0, reserve1)
        } else {
            (reserve1, reserve0)
        };
        
        // Calculate price impact
        Self::calculate_v2_impact(amount_in, reserve_in, reserve_out)
    }

    /// Convert U256 to f64 safely
    fn u256_to_f64(value: U256) -> Result<f64, DexError> {
        // Convert to u128 first, then to f64
        let value_u128: u128 = value.try_into()
            .map_err(|_| DexError::InvalidAmount("Value too large for conversion".into()))?;
        
        Ok(value_u128 as f64)
    }

    /// Parse amount string to U256 with decimals
    fn parse_amount(amount_str: &str, decimals: u8) -> Result<U256, DexError> {
        let amount_f64: f64 = amount_str.parse()
            .map_err(|_| DexError::InvalidAmount(format!("Invalid amount: {}", amount_str)))?;
        
        if amount_f64 < 0.0 {
            return Err(DexError::InvalidAmount("Amount cannot be negative".into()));
        }
        
        // Convert to wei/smallest unit
        let multiplier = 10_u64.pow(decimals as u32) as f64;
        let amount_wei = (amount_f64 * multiplier) as u128;
        
        Ok(U256::from(amount_wei))
    }

    /// Categorize price impact severity
    pub fn categorize_impact(price_impact: f64) -> &'static str {
        match price_impact {
            x if x < 0.1 => "Minimal",
            x if x < 1.0 => "Low", 
            x if x < 3.0 => "Medium",
            x if x < 5.0 => "High",
            _ => "Very High",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_price_impact_calculation() {
        // Test with realistic ETH/USDC pool
        let amount_in = U256::from(10).pow(U256::from(18)); // 1 ETH
        let reserve_eth = U256::from(1000) * U256::from(10).pow(U256::from(18)); // 1000 ETH
        let reserve_usdc = U256::from(3700000) * U256::from(10).pow(U256::from(6)); // 3.7M USDC
        
        let impact = PriceImpactCalculator::calculate_v2_impact(
            amount_in,
            reserve_eth,
            reserve_usdc,
        ).unwrap();
        
        // 1 ETH in 1000 ETH pool should have ~0.4% impact (0.1% of pool size)
        // This is correct based on AMM math - small trades still have measurable impact
        assert!(impact > 0.3 && impact < 0.5, "Impact should be around 0.4%, got {}", impact);
        println!("Small trade impact: {:.4}%", impact);
    }

    #[test]
    fn test_large_trade_impact() {
        // Test with large trade (10% of pool)
        let amount_in = U256::from(100) * U256::from(10).pow(U256::from(18)); // 100 ETH
        let reserve_eth = U256::from(1000) * U256::from(10).pow(U256::from(18)); // 1000 ETH
        let reserve_usdc = U256::from(3700000) * U256::from(10).pow(U256::from(6)); // 3.7M USDC
        
        let impact = PriceImpactCalculator::calculate_v2_impact(
            amount_in,
            reserve_eth,
            reserve_usdc,
        ).unwrap();
        
        // 100 ETH in 1000 ETH pool should have significant impact (>5%)
        assert!(impact > 5.0, "Large trade should have >5% impact, got {}", impact);
        println!("Large trade impact: {:.4}%", impact);
    }

    #[test]
    fn test_impact_categorization() {
        assert_eq!(PriceImpactCalculator::categorize_impact(0.05), "Minimal");
        assert_eq!(PriceImpactCalculator::categorize_impact(0.5), "Low");
        assert_eq!(PriceImpactCalculator::categorize_impact(2.0), "Medium");
        assert_eq!(PriceImpactCalculator::categorize_impact(4.0), "High");
        assert_eq!(PriceImpactCalculator::categorize_impact(10.0), "Very High");
    }

    #[test]
    fn test_amount_parsing() {
        assert_eq!(
            PriceImpactCalculator::parse_amount("1.0", 18).unwrap(),
            U256::from(10).pow(U256::from(18))
        );
        
        assert_eq!(
            PriceImpactCalculator::parse_amount("1000.123456", 6).unwrap(),
            U256::from(1000123456)
        );
    }
}
