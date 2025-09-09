use bralaladex_backend::dexes::{DexIntegration, aerodrome::AerodromeDex};
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

    info!("🚀 Testing Aerodrome Implementation");

    // Test cases covering Base chain from Aerodrome research
    let test_cases = vec![
        // Base mainnet - Volatile pools
        ("base", "ETH", "USDC", "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        ("base", "WETH", "USDC", "0x4200000000000000000000000000000000000006", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        ("base", "AERO", "WETH", "0x940181a94A35A4569E4529A3CDfB74e38FD98631", "0x4200000000000000000000000000000000000006"),
        
        // Base mainnet - Stable pools
        ("base", "USDC", "USDT", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
        ("base", "USDT", "DAI", "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2", "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
        
        // Base mainnet - Other pairs
        ("base", "cbETH", "WETH", "0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22", "0x4200000000000000000000000000000000000006"),
    ];

    let dex = AerodromeDex::new();
    
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
            token_in_decimals: Some(if token_in_name.contains("USDC") || token_in_name.contains("USDT") { 6 } else { 18 }),
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
    
    // Test unsupported chain (Ethereum - not supported by Aerodrome)
    match dex.is_pair_supported("0x123", "0x456", "ethereum").await {
        Ok(supported) => {
            if !supported {
                info!("✅ Correctly rejected unsupported chain (ethereum)");
            } else {
                error!("❌ Should have rejected unsupported chain (ethereum)");
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
        token_out_address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("base".to_string()),
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
        token_in_address: Some("0x4200000000000000000000000000000000000006".to_string()),
        token_in_decimals: Some(18),
        token_out: "WETH".to_string(),
        token_out_address: Some("0x4200000000000000000000000000000000000006".to_string()),
        token_out_decimals: Some(18),
        amount_in: "1.0".to_string(),
        chain: Some("base".to_string()),
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

    info!("\n🎉 Aerodrome testing completed!");
    Ok(())
}
