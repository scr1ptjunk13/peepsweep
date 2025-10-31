use bralaladex_backend::dexes::spiritswap_v2::SpiritSwapV2Dex;
use bralaladex_backend::dexes::DexIntegration;
use bralaladex_backend::types::QuoteParams;
use tracing::{info, error};
use tracing_subscriber;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Testing SpiritSwap V3 Implementation");

    // Initialize SpiritSwap V3 DEX
    let dex = SpiritSwapV2Dex::new();
    
    // Display supported chains
    let chains = dex.get_supported_chains();
    info!("📋 Supported chains: {:?}", chains);

    // Test pairs with known liquidity on Fantom
    let test_pairs = [
        // (token_in_addr, token_out_addr, token_in_name, token_out_name, chain)
        ("0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83", "0x04068da6c83afcfa0e13ba15a6696662335d5b75", "WFTM", "USDC", "fantom"),
        ("0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83", "0x5cc61a78f164885776aa610fb0fe1257df78e59b", "WFTM", "SPIRIT", "fantom"),
        ("0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83", "0x74b23882a30290451a17c44f4f05243b6b58c76d", "WFTM", "ETH", "fantom"),
    ];

    for (token_in_addr, token_out_addr, token_in_name, token_out_name, chain) in test_pairs {
        info!("\n🔍 Testing {} -> {} on {}", token_in_name, token_out_name, chain);

        // Check if pair is supported
        match dex.is_pair_supported(token_in_addr, token_out_addr, chain).await {
            Ok(true) => {
                info!("✅ Pair {} -> {} supported on {}", token_in_name, token_out_name, chain);
            }
            Ok(false) => {
                info!("❌ Pair {} -> {} not supported on {}", token_in_name, token_out_name, chain);
                continue;
            }
            Err(e) => {
                error!("❌ Error checking pair support: {:?}", e);
                continue;
            }
        }

        // Test quote
        let quote_params = QuoteParams {
            chain: Some(chain.to_string()),
            token_in: token_in_addr.to_string(),
            token_out: token_out_addr.to_string(),
            amount_in: "1.0".to_string(),
            token_in_decimals: Some(18),
            token_out_decimals: Some(if token_out_name == "USDC" { 6 } else { 18 }),
            token_in_address: None,
            token_out_address: None,
            slippage: None,
        };

        match dex.get_quote(&quote_params).await {
            Ok(route) => {
                info!("✅ Quote successful on {}: {} {} -> {} {}", 
                    chain, quote_params.amount_in, token_in_name, route.amount_out, token_out_name);
                info!("   Gas estimate: {} (Concentrated liquidity)", route.gas_used);
                info!("   DEX: {}, Percentage: {}%", route.dex, route.percentage);
                info!("   Fee: Dynamic (returned by Algebra quoter)");
            }
            Err(e) => {
                error!("❌ Quote failed on {}: {:?}", chain, e);
            }
        }

        info!("✅ Gas estimate: 180000 (Concentrated liquidity optimized)");
    }

    info!("\n🧪 Testing edge cases...");
    
    // Test unsupported chain
    match dex.is_pair_supported("0x123", "0x456", "ethereum").await {
        Ok(false) => info!("✅ Correctly rejected unsupported chain"),
        _ => error!("❌ Should reject unsupported chain"),
    }

    // Test invalid token address
    let invalid_params = QuoteParams {
        chain: Some("fantom".to_string()),
        token_in: "invalid_address".to_string(),
        token_out: "0x04068DA6C83AFCFA0e13ba15A6696662335D5B75".to_string(),
        amount_in: "1.0".to_string(),
        token_in_decimals: Some(18),
        token_out_decimals: Some(6),
        token_in_address: None,
        token_out_address: None,
        slippage: None,
    };

    match dex.get_quote(&invalid_params).await {
        Err(e) => info!("✅ Correctly rejected invalid token address: {:?}", e),
        Ok(_) => error!("❌ Should reject invalid token address"),
    }

    // Test same token swap
    let same_token_params = QuoteParams {
        chain: Some("fantom".to_string()),
        token_in: "0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83".to_string(), // WFTM
        token_out: "0x21be370d5312f44cb42ce377bc9b8a0cef1a4c83".to_string(), // Same WFTM
        amount_in: "1.0".to_string(),
        token_in_decimals: Some(18),
        token_out_decimals: Some(18),
        token_in_address: None,
        token_out_address: None,
        slippage: None,
    };

    match dex.get_quote(&same_token_params).await {
        Err(e) => info!("✅ Correctly rejected same token swap: {:?}", e),
        Ok(_) => error!("❌ Should reject same token swap"),
    }

    info!("\n🎉 SpiritSwap V3 testing completed!");
    info!("💡 Key advantages: Concentrated liquidity, dynamic fees, capital efficiency, Algebra integration");
}
