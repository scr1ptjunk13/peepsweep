use alloy::primitives::{U256, Address, keccak256, FixedBytes};
use alloy::providers::Provider;
use std::sync::Arc;
use std::str::FromStr;
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

    /// Get REAL reserves from Uniswap V2 pair contract
    /// Returns (reserve0, reserve1, timestamp)
    pub async fn get_v2_reserves(
        &self,
        token0: &str,
        token1: &str,
        chain: &str,
    ) -> Result<(U256, U256, u32), DexError> {
        // Imports already at top of file
        
        // Parse token addresses
        let token0_addr = Address::from_str(token0)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token0 address: {}", token0)))?;
        let token1_addr = Address::from_str(token1)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid token1 address: {}", token1)))?;
        
        // Sort tokens (Uniswap V2 requirement)
        let (token_a, token_b) = if token0_addr < token1_addr {
            (token0_addr, token1_addr)
        } else {
            (token1_addr, token0_addr)
        };
        
        // Calculate pair address using Uniswap V2 CREATE2 formula
        let pair_address = self.calculate_pair_address(token_a, token_b, chain)?;
        
        // Get provider for the chain
        let provider = self.provider_cache.get_provider(chain).await?;
        
        // Call getReserves() on the pair contract
        // function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        let get_reserves_selector = [0x09, 0x02, 0xf1, 0xac]; // getReserves()
        
        let call_data = alloy::primitives::Bytes::from(get_reserves_selector.to_vec());
        let call_request = alloy::rpc::types::TransactionRequest::default()
            .to(pair_address)
            .input(call_data.into());
        
        // Make the call with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            provider.call(&call_request).block(alloy::rpc::types::BlockId::latest())
        ).await
        .map_err(|_| DexError::Timeout("Reserve fetch timeout".into()))?
        .map_err(|e| DexError::ContractCallFailed(format!("getReserves call failed: {:?}", e)))?;
        
        // Decode the result
        // Returns: (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        // Total: 32 + 32 + 32 = 96 bytes (each padded to 32 bytes)
        if result.len() < 96 {
            return Err(DexError::ContractCallFailed("Invalid getReserves response length".into()));
        }
        
        // Extract reserves - uint112 values are in the LAST 14 bytes of each 32-byte word
        // ABI encoding pads left, so uint112 occupies bytes [18..32] of each 32-byte slot
        let reserve0_bytes = &result[18..32]; // Last 14 bytes of first 32-byte word
        let reserve1_bytes = &result[50..64]; // Last 14 bytes of second 32-byte word
        let timestamp_bytes = &result[92..96]; // Last 4 bytes of third word
        
        // Debug: Print raw bytes
        println!("🔍 RAW RESPONSE DEBUG:");
        println!("   Total length: {} bytes", result.len());
        println!("   Raw hex: 0x{}", hex::encode(&result));
        println!("   Reserve0 bytes: {:?}", reserve0_bytes);
        println!("   Reserve1 bytes: {:?}", reserve1_bytes);
        
        let reserve0 = U256::from_be_slice(reserve0_bytes);
        let reserve1 = U256::from_be_slice(reserve1_bytes);
        let timestamp = u32::from_be_bytes([
            timestamp_bytes[0], timestamp_bytes[1], 
            timestamp_bytes[2], timestamp_bytes[3]
        ]);
        
        // Debug: Print the reserves we got
        println!("🔍 REAL RESERVES FETCHED:");
        println!("   Pair Address: {:?}", pair_address);
        println!("   Reserve0: {} ({})", reserve0, Self::u256_to_f64(reserve0).unwrap_or(0.0));
        println!("   Reserve1: {} ({})", reserve1, Self::u256_to_f64(reserve1).unwrap_or(0.0));
        println!("   Timestamp: {}", timestamp);
        
        Ok((reserve0, reserve1, timestamp))
    }
    
    /// Calculate Uniswap V2 pair address using CREATE2
    fn calculate_pair_address(&self, token_a: Address, token_b: Address, chain: &str) -> Result<Address, DexError> {
        // Uniswap V2 Factory addresses
        let factory_address = match chain {
            "ethereum" => "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
            "polygon" => "0x5757371414417b8C6CAad45bAeF941aBc7d3Ab32", 
            "bsc" => "0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73",
            _ => return Err(DexError::UnsupportedChain(format!("No Uniswap V2 factory for chain: {}", chain))),
        };
        
        let factory_addr = Address::from_str(factory_address)
            .map_err(|_| DexError::InvalidAddress("Invalid factory address".into()))?;
        
        // INIT_CODE_HASH for Uniswap V2
        let init_code_hash = match chain {
            "ethereum" => "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
            "polygon" => "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f",
            "bsc" => "0x00fb7f630766e6a796048ea87d01acd3068e8ff67d078148a3fa3f4a84f69bd5", // PancakeSwap
            _ => "0x96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f", // Default Uniswap
        };
        
        let init_hash: FixedBytes<32> = init_code_hash.parse()
            .map_err(|_| DexError::InvalidAddress("Invalid init code hash".into()))?;
        
        // CREATE2 formula: keccak256(0xff + factory + salt + init_code_hash)
        // salt = keccak256(abi.encodePacked(token0, token1))
        let mut salt_data = Vec::new();
        salt_data.extend_from_slice(token_a.as_slice());
        salt_data.extend_from_slice(token_b.as_slice());
        let salt = keccak256(&salt_data);
        
        // Construct CREATE2 data
        let mut create2_data = Vec::new();
        create2_data.push(0xff);
        create2_data.extend_from_slice(factory_addr.as_slice());
        create2_data.extend_from_slice(salt.as_slice());
        create2_data.extend_from_slice(init_hash.as_slice());
        
        let pair_hash = keccak256(&create2_data);
        let pair_address = Address::from_slice(&pair_hash[12..32]); // Take last 20 bytes
        
        Ok(pair_address)
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
