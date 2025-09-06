use alloy::primitives::{Address, U256};
use std::str::FromStr;
use std::num::ParseIntError;
use crate::dexes::DexError;

// Add missing error types if they don't exist
#[allow(dead_code)]
mod error_extensions {
    use crate::dexes::DexError;
    
    impl DexError {
        pub fn invalid_pair(msg: String) -> Self {
            DexError::UnsupportedPair(msg)
        }
        
        pub fn invalid_address(msg: String) -> Self {
            DexError::ParseError(msg)
        }
        
        pub fn invalid_amount(msg: String) -> Self {
            DexError::InvalidAmount(msg)
        }
    }
}

/// Universal DEX utilities for safe amount parsing, address resolution, and formatting
pub struct DexUtils;

impl DexUtils {
    /// Parse amount string to U256 with proper decimal handling - NO FLOATING POINT
    pub fn parse_amount_safe(amount: &str, decimals: u8) -> Result<U256, DexError> {
        if amount.is_empty() {
            return Err(DexError::InvalidAmount("Empty amount".into()));
        }

        // Split on decimal point
        let parts: Vec<&str> = amount.split('.').collect();
        if parts.len() > 2 {
            return Err(DexError::InvalidAmount("Multiple decimal points".into()));
        }

        // Parse whole part
        let whole_part = parts[0].parse::<u128>()
            .map_err(|e: ParseIntError| DexError::InvalidAmount(format!("Invalid whole number: {}", e)))?;

        // Parse decimal part
        let decimal_part = if parts.len() > 1 {
            let decimal_str = parts[1];
            if decimal_str.len() > decimals as usize {
                return Err(DexError::InvalidAmount("Too many decimal places".into()));
            }
            // Pad with zeros to match decimals
            let padded = format!("{:0<width$}", decimal_str, width = decimals as usize);
            padded.parse::<u128>()
                .map_err(|e: ParseIntError| DexError::InvalidAmount(format!("Invalid decimal part: {}", e)))?
        } else {
            0
        };

        // Calculate final amount: whole * 10^decimals + decimal_part
        let multiplier = U256::from(10).pow(U256::from(decimals));
        let whole_wei = U256::from(whole_part) * multiplier;
        let decimal_wei = U256::from(decimal_part);
        
        Ok(whole_wei + decimal_wei)
    }

    /// Format U256 amount to human-readable string with proper decimal handling
    pub fn format_amount_safe(amount: U256, decimals: u8) -> String {
        if decimals > 77 { // U256 max safe decimal places
            return "0".to_string();
        }

        let divisor = U256::from(10).pow(U256::from(decimals));
        let whole = amount / divisor;
        let remainder = amount % divisor;

        if remainder.is_zero() {
            whole.to_string()
        } else {
            // Format remainder with leading zeros
            let remainder_str = format!("{:0width$}", remainder, width = decimals as usize);
            // Trim trailing zeros
            let trimmed = remainder_str.trim_end_matches('0');
            if trimmed.is_empty() {
                whole.to_string()
            } else {
                format!("{}.{}", whole, trimmed)
            }
        }
    }

    /// Resolve ETH to WETH address for chains that need it
    pub fn resolve_eth_to_weth(token_address: &str, chain: &str) -> Result<Address, DexError> {
        let addr_str = if token_address.to_lowercase() == "eth" || 
                          token_address == "0x0000000000000000000000000000000000000000" {
            match chain {
                "ethereum" => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", // WETH
                "optimism" => "0x4200000000000000000000000000000000000006", // WETH
                "arbitrum" => "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1", // WETH
                "polygon" => "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619", // WETH
                "base" => "0x4200000000000000000000000000000000000006", // WETH
                "avalanche" => "0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB", // WETH.e
                _ => return Err(DexError::UnsupportedChain(format!("No WETH for chain: {}", chain)))
            }
        } else {
            token_address
        };

        Address::from_str(addr_str)
            .map_err(|_| DexError::InvalidAddress(format!("Invalid address: {}", addr_str)))
    }

    /// Validate token pair for basic sanity checks (string version)
    pub fn validate_token_pair(token_a: &str, token_b: &str) -> Result<(), DexError> {
        if token_a == token_b {
            return Err(DexError::InvalidPair("Identical tokens".into()));
        }
        
        // Check for zero address
        if token_a == "0x0000000000000000000000000000000000000000" || 
           token_b == "0x0000000000000000000000000000000000000000" {
            return Err(DexError::InvalidAddress("Zero address not allowed".into()));
        }

        Ok(())
    }

    /// Validate token pair for basic sanity checks (Address version)
    pub fn validate_token_pair_addresses(token_a: &Address, token_b: &Address) -> Result<(), DexError> {
        if token_a == token_b {
            return Err(DexError::InvalidPair("Identical tokens".into()));
        }
        
        // Check for zero address
        let zero_addr = Address::from_str("0x0000000000000000000000000000000000000000").unwrap();
        if *token_a == zero_addr || *token_b == zero_addr {
            return Err(DexError::InvalidAddress("Zero address not allowed".into()));
        }

        Ok(())
    }

    /// Get standard token decimals for common tokens
    pub fn get_standard_decimals(token_address: &Address, chain: &str) -> u8 {
        let addr_str = format!("{:?}", token_address).to_lowercase();
        
        match chain {
            "ethereum" => match addr_str.as_str() {
                "0xa0b86a33e6411c8c5e0b8621c0b4b5b6c4b4b4b4" => 6,  // USDC
                "0xdac17f958d2ee523a2206206994597c13d831ec7" => 6,  // USDT
                "0x6b175474e89094c44da98b954eedeac495271d0f" => 18, // DAI
                "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599" => 8,  // WBTC
                _ => 18 // Default ERC20
            },
            "optimism" => match addr_str.as_str() {
                "0x7f5c764cbc14f9669b88837ca1490cca17c31607" => 6,  // USDC
                "0x94b008aa00579c1307b0ef2c499ad98a8ce58e58" => 6,  // USDT
                "0xda10009cbd5d07dd0cecc66161fc93d7c9000da1" => 18, // DAI
                "0x68f180fcce6836688e9084f035309e29bf0a2095" => 8,  // WBTC
                _ => 18
            },
            _ => 18 // Default for unknown chains
        }
    }

    /// Calculate minimum amount out with slippage protection
    pub fn calculate_min_amount_out(amount_out: U256, slippage_bps: u16) -> U256 {
        if slippage_bps >= 10000 {
            return U256::ZERO; // 100%+ slippage = no minimum
        }
        
        let slippage_factor = U256::from(10000 - slippage_bps);
        (amount_out * slippage_factor) / U256::from(10000)
    }

    /// Validate amount is not zero and within reasonable bounds
    pub fn validate_amount(amount: U256, min_amount: Option<U256>, max_amount: Option<U256>) -> Result<(), DexError> {
        if amount.is_zero() {
            return Err(DexError::InvalidAmount("Amount cannot be zero".into()));
        }

        if let Some(min) = min_amount {
            if amount < min {
                return Err(DexError::InvalidAmount(format!("Amount {} below minimum {}", amount, min)));
            }
        }

        if let Some(max) = max_amount {
            if amount > max {
                return Err(DexError::InvalidAmount(format!("Amount {} above maximum {}", amount, max)));
            }
        }

        Ok(())
    }

    /// Get WETH address for a given chain
    pub fn get_weth_address(chain: &str) -> Result<String, DexError> {
        match chain {
            "ethereum" => Ok("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            "optimism" => Ok("0x4200000000000000000000000000000000000006".to_string()),
            "arbitrum" => Ok("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
            "polygon" => Ok("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
            "base" => Ok("0x4200000000000000000000000000000000000006".to_string()),
            "avalanche" => Ok("0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB".to_string()),
            _ => Err(DexError::UnsupportedChain(format!("No WETH for chain: {}", chain)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amount_safe() {
        // Test whole numbers
        assert_eq!(DexUtils::parse_amount_safe("100", 18).unwrap(), U256::from(100) * U256::from(10).pow(U256::from(18)));
        
        // Test decimals
        assert_eq!(DexUtils::parse_amount_safe("1.5", 18).unwrap(), U256::from(15) * U256::from(10).pow(U256::from(17)));
        
        // Test USDC (6 decimals)
        assert_eq!(DexUtils::parse_amount_safe("1000.123456", 6).unwrap(), U256::from(1000123456));
        
        // Test precision
        assert_eq!(DexUtils::parse_amount_safe("0.000001", 6).unwrap(), U256::from(1));
    }

    #[test]
    fn test_format_amount_safe() {
        // Test whole numbers
        let amount = U256::from(100) * U256::from(10).pow(U256::from(18));
        assert_eq!(DexUtils::format_amount_safe(amount, 18), "100");
        
        // Test decimals
        let amount = U256::from(15) * U256::from(10).pow(U256::from(17));
        assert_eq!(DexUtils::format_amount_safe(amount, 18), "1.5");
        
        // Test USDC
        assert_eq!(DexUtils::format_amount_safe(U256::from(1000123456), 6), "1000.123456");
    }

    #[test]
    fn test_eth_to_weth_resolution() {
        let weth_eth = DexUtils::resolve_eth_to_weth("eth", "ethereum").unwrap();
        let weth_opt = DexUtils::resolve_eth_to_weth("ETH", "optimism").unwrap();
        
        assert_eq!(format!("{:?}", weth_eth), "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        assert_eq!(format!("{:?}", weth_opt), "0x4200000000000000000000000000000000000006");
    }

    #[test]
    fn test_get_weth_address() {
        assert_eq!(DexUtils::get_weth_address("ethereum").unwrap(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        assert_eq!(DexUtils::get_weth_address("optimism").unwrap(), "0x4200000000000000000000000000000000000006");
        assert!(DexUtils::get_weth_address("unsupported").is_err());
    }

    #[test]
    fn test_slippage_calculation() {
        let amount = U256::from(1000);
        
        // 1% slippage (100 bps)
        let min_out = DexUtils::calculate_min_amount_out(amount, 100);
        assert_eq!(min_out, U256::from(990));
        
        // 0.5% slippage (50 bps)
        let min_out = DexUtils::calculate_min_amount_out(amount, 50);
        assert_eq!(min_out, U256::from(995));
    }

    #[test]
    fn test_validate_token_pair() {
        // Valid pair
        assert!(DexUtils::validate_token_pair("0x1234567890123456789012345678901234567890", "0x0987654321098765432109876543210987654321").is_ok());
        
        // Identical tokens
        assert!(DexUtils::validate_token_pair("0x1234567890123456789012345678901234567890", "0x1234567890123456789012345678901234567890").is_err());
        
        // Zero address
        assert!(DexUtils::validate_token_pair("0x0000000000000000000000000000000000000000", "0x1234567890123456789012345678901234567890").is_err());
    }
}
