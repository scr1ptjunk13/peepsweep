use bralaladex_backend::dexes::sushiswap::SushiSwapV2Dex;
use bralaladex_backend::dexes::DexIntegration;
use bralaladex_backend::types::QuoteParams;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    info!("🚀 Testing SushiSwap V2 Implementation");

    let test_cases = vec![
        // Ethereum mainnet - Major pairs
        ("ethereum", "ETH", "USDC", "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        ("ethereum", "WETH", "USDC", "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        ("ethereum", "USDC", "USDT", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", "0xdac17f958d2ee523a2206206994597c13d831ec7"),
        
        // Polygon mainnet - Major pairs  
        ("polygon", "WMATIC", "USDC", "0x0d500b1d8e8ef31e21c99d1db9a6444d3adf1270", "0x2791bca1f2de4661ed88a30c99a7a9449aa84174"),
        ("polygon", "WETH", "USDC", "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619", "0x2791bca1f2de4661ed88a30c99a7a9449aa84174"),
        
        // Arbitrum mainnet - Major pairs  
        ("arbitrum", "WETH", "USDC", "0x82af49447d8a07e3bd95bd0d56f35241523fbab1", "0xff970a61a04b1ca14834a43f5de4533ebddb5cc8"),
        ("arbitrum", "ARB", "WETH", "0x912ce59144191c1204e64559fe8253a0e49e6548", "0x82af49447d8a07e3bd95bd0d56f35241523fbab1"),
        
        // Base mainnet - Major pairs
        ("base", "ETH", "USDC", "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE", "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
        ("base", "WETH", "USDC", "0x4200000000000000000000000000000000000006", "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
    ];

    let dex = SushiSwapV2Dex::new();
    
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
        token_out_address: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };

    match dex.get_quote(&invalid_quote_params).await {
        Ok(_) => {
            error!("❌ Should have rejected invalid token address");
        }
        Err(e) => {
            info!("✅ Correctly rejected invalid token address: {:?}", e);
        }
    }

    // Test same token swap
    let same_token_params = QuoteParams {
        token_in: "USDC".to_string(),
        token_in_address: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()),
        token_in_decimals: Some(6),
        token_out: "USDC".to_string(),
        token_out_address: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };

    match dex.get_quote(&same_token_params).await {
        Ok(_) => {
            error!("❌ Should have rejected same token swap");
        }
        Err(e) => {
            info!("✅ Correctly rejected same token swap: {:?}", e);
        }
    }
    
    info!("\n🎉 SushiSwap V2 testing completed!");
}
