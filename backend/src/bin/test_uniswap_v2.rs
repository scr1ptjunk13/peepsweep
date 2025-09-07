use bralaladex_backend::dexes::{DexIntegration, uniswap_v2::UniswapV2Dex};
use bralaladex_backend::types::QuoteParams;
use tokio;
use tracing::{info, error, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🚀 Testing Uniswap V2 Implementation");

    // Test cases covering major chains from research
    let test_cases = vec![
        // Ethereum Mainnet
        ("ethereum", "WETH", "USDC", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        // Arbitrum
        ("arbitrum", "WETH", "USDC", "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        // Polygon
        ("polygon", "WMATIC", "USDC", "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270", "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"),
        // Base
        ("base", "WETH", "USDC", "0x4200000000000000000000000000000000000006", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        // Optimism
        ("optimism", "WETH", "USDC", "0x4200000000000000000000000000000000000006", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
        // BSC
        ("bsc", "WBNB", "USDT", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c", "0x55d398326f99059fF775485246999027B3197955"),
    ];

    let dex = UniswapV2Dex::new();
    
    info!("📋 Supported chains: {:?}", dex.get_supported_chains());

    for (chain, token_in_name, token_out_name, token_in_addr, token_out_addr) in test_cases {
        info!("\n🔍 Testing {} -> {} on {}", token_in_name, token_out_name, chain);
        
        // Test pair support
        match dex.is_pair_supported(token_in_addr, token_out_addr, chain).await {
            Ok(supported) => {
                if supported {
                    info!("✅ Pair {} -> {} supported on {}", token_in_name, token_out_name, chain);
                } else {
                    info!("❌ Pair {} -> {} not supported on {}", token_in_name, token_out_name, chain);
                    continue;
                }
            }
            Err(e) => {
                error!("❌ Error checking pair support: {:?}", e);
                continue;
            }
        }

        // Test quote
        let quote_params = QuoteParams {
            token_in: token_in_name.to_string(),
            token_in_address: Some(token_in_addr.to_string()),
            token_in_decimals: Some(18),
            token_out: token_out_name.to_string(),
            token_out_address: Some(token_out_addr.to_string()),
            token_out_decimals: Some(if token_out_name.contains("USDC") || token_out_name.contains("USDT") { 6 } else { 18 }),
            amount_in: "1.0".to_string(),
            chain: Some(chain.to_string()),
            slippage: Some(0.5),
        };

        match dex.get_quote(&quote_params).await {
            Ok(route) => {
                info!("✅ Quote successful on {}: {} {} -> {} {}", 
                    chain, quote_params.amount_in, token_in_name, route.amount_out, token_out_name);
                info!("   Gas estimate: {}", route.gas_used);
                info!("   DEX: {}, Percentage: {}%", route.dex, route.percentage);
            }
            Err(e) => {
                error!("❌ Quote failed on {}: {:?}", chain, e);
            }
        }

        // Test gas estimate
        let dummy_swap_params = bralaladex_backend::types::SwapParams {
            token_in: token_in_name.to_string(),
            token_out: token_out_name.to_string(),
            amount_in: "1.0".to_string(),
            amount_out_min: "0.9".to_string(),
            routes: vec![],
            user_address: "0x1234567890123456789012345678901234567890".to_string(),
            slippage: 0.5,
        };
        
        match dex.get_gas_estimate(&dummy_swap_params).await {
            Ok(gas) => {
                info!("✅ Gas estimate: {}", gas);
            }
            Err(e) => {
                error!("❌ Gas estimate failed: {:?}", e);
            }
        }
    }

    // Test edge cases
    info!("\n🧪 Testing edge cases...");
    
    // Test unsupported chain
    match dex.is_pair_supported("0x123", "0x456", "unsupported_chain").await {
        Ok(supported) => {
            if !supported {
                info!("✅ Correctly rejected unsupported chain");
            } else {
                error!("❌ Should have rejected unsupported chain");
            }
        }
        Err(e) => {
            info!("✅ Correctly errored on unsupported chain: {:?}", e);
        }
    }

    // Test invalid token addresses
    let invalid_quote_params = QuoteParams {
        token_in: "INVALID".to_string(),
        token_in_address: Some("invalid_address".to_string()),
        token_in_decimals: Some(18),
        token_out: "USDC".to_string(),
        token_out_address: Some("0xA0b86a33E6441b8C0b5d8cF8E9e9C8c8c8c8c8c8".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };

    match dex.get_quote(&invalid_quote_params).await {
        Ok(_) => {
            error!("❌ Should have failed with invalid token address");
        }
        Err(e) => {
            info!("✅ Correctly rejected invalid token address: {:?}", e);
        }
    }

    // Test same token swap
    let same_token_params = QuoteParams {
        token_in: "WETH".to_string(),
        token_in_address: Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
        token_in_decimals: Some(18),
        token_out: "WETH".to_string(),
        token_out_address: Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
        token_out_decimals: Some(18),
        amount_in: "1.0".to_string(),
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };

    match dex.get_quote(&same_token_params).await {
        Ok(_) => {
            error!("❌ Should have failed with same token swap");
        }
        Err(e) => {
            info!("✅ Correctly rejected same token swap: {:?}", e);
        }
    }

    info!("\n🎉 Uniswap V2 testing completed!");
    Ok(())
}
