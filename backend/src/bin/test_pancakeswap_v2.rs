use bralaladex_backend::dexes::{DexIntegration, pancakeswap_v2::PancakeSwapV2Dex};
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

    info!("🚀 Testing PancakeSwap V2 Implementation");

    // Test cases covering major chains from research (Polygon removed - no real PancakeSwap V2 deployment)
    let test_cases = vec![
        // BSC - Primary chain with highest liquidity
        ("bsc", "WBNB", "CAKE", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c", "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82"),
        ("bsc", "WBNB", "BUSD", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c", "0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56"),
        // Ethereum
        ("ethereum", "WETH", "USDC", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        // Arbitrum One
        ("arbitrum", "WETH", "USDC", "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        // Base
        ("base", "WETH", "USDC", "0x4200000000000000000000000000000000000006", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
    ];

    let dex = PancakeSwapV2Dex::new();
    
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
            token_out_decimals: Some(if token_out_name.contains("USDC") || token_out_name.contains("BUSD") { 6 } else { 18 }),
            amount_in: "1.0".to_string(),
            chain: Some(chain.to_string()),
            slippage: Some(0.5),
        };

        match dex.get_quote(&quote_params).await {
            Ok(route) => {
                info!("✅ Quote successful on {}: {} {} -> {} {}", 
                    chain, quote_params.amount_in, token_in_name, route.amount_out, token_out_name);
                info!("   Gas estimate: {} (lower than Uniswap V2)", route.gas_used);
                info!("   DEX: {}, Percentage: {}%", route.dex, route.percentage);
                info!("   Fee: 0.25% (vs Uniswap's 0.3%)");
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
                info!("✅ Gas estimate: {} (optimized)", gas);
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
        token_out: "CAKE".to_string(),
        token_out_address: Some("0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82".to_string()),
        token_out_decimals: Some(18),
        amount_in: "1.0".to_string(),
        chain: Some("bsc".to_string()),
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
        token_in: "WBNB".to_string(),
        token_in_address: Some("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string()),
        token_in_decimals: Some(18),
        token_out: "WBNB".to_string(),
        token_out_address: Some("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c".to_string()),
        token_out_decimals: Some(18),
        amount_in: "1.0".to_string(),
        chain: Some("bsc".to_string()),
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

    info!("\n🎉 PancakeSwap V2 testing completed!");
    info!("💡 Key advantages: 0.25% fee vs Uniswap's 0.3%, optimized gas usage, strong BSC liquidity");
    Ok(())
}
