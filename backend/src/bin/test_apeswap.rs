use bralaladex_backend::dexes::{DexIntegration, apeswap::ApeSwapDex};
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

    info!("🚀 Testing ApeSwap Implementation");

    // Test cases covering BSC and Polygon chains from ApeSwap research
    let test_cases = vec![
        // BSC (Binance Smart Chain)
        ("bsc", "WBNB", "USDT", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c", "0x55d398326f99059fF775485246999027B3197955"),
        ("bsc", "BANANA", "WBNB", "0x603c7f932ED1fc6575303D8Fb018fDCBb0f39a95", "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"),
        ("bsc", "USDC", "BUSD", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d", "0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56"),
        // Polygon
        ("polygon", "WMATIC", "USDC", "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270", "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"),
        ("polygon", "BANANA", "WMATIC", "0x5d47bAbA0d66083C52009271faF3F50DCc01023C", "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"),
        ("polygon", "WETH", "USDT", "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619", "0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
    ];

    let dex = ApeSwapDex::new();
    
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
    
    // Test unsupported chain (Ethereum - not supported by ApeSwap)
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
        token_out: "USDT".to_string(),
        token_out_address: Some("0x55d398326f99059fF775485246999027B3197955".to_string()),
        token_out_decimals: Some(6),
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

    info!("\n🎉 ApeSwap testing completed!");
    Ok(())
}
