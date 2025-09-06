use std::env;
use tokio;

// Add the backend crate as a dependency
use bralaladex_backend::dexes::{UniswapDex, DexIntegration};
use bralaladex_backend::types::QuoteParams;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    println!("🚀 Testing Uniswap DEX Integration Individually");
    println!("=" .repeat(50));
    
    // Test 1: Initialize Uniswap DEX
    println!("\n📊 Test 1: Initializing Uniswap DEX...");
    let uniswap = match UniswapDex::new().await {
        Ok(dex) => {
            println!("✅ Uniswap DEX initialized successfully");
            println!("   Name: {}", dex.get_name());
            println!("   Supported chains: {:?}", dex.get_supported_chains());
            dex
        }
        Err(e) => {
            println!("❌ Failed to initialize Uniswap DEX: {}", e);
            return Err(e.into());
        }
    };
    
    // Test 2: Check chain support
    println!("\n🌐 Test 2: Testing chain support...");
    let test_chains = vec!["ethereum", "polygon", "arbitrum", "optimism", "base", "solana"];
    for chain in test_chains {
        let supported = uniswap.supports_chain(chain);
        println!("   {}: {}", chain, if supported { "✅ Supported" } else { "❌ Not supported" });
    }
    
    // Test 3: Test amount conversion
    println!("\n💰 Test 3: Testing amount conversion...");
    let test_amounts = vec![
        ("1.0", 18, "ETH"),
        ("100.0", 6, "USDC"),
        ("0.5", 8, "WBTC"),
    ];
    
    for (amount, decimals, token) in test_amounts {
        match uniswap.convert_to_wei(amount, decimals) {
            Ok(wei_amount) => {
                println!("   {} {} -> {} wei ✅", amount, token, wei_amount);
            }
            Err(e) => {
                println!("   {} {} -> Error: {} ❌", amount, token, e);
            }
        }
    }
    
    // Test 4: Test chain configuration
    println!("\n⚙️  Test 4: Testing chain configurations...");
    let chains = vec!["ethereum", "polygon", "arbitrum", "optimism", "base"];
    for chain in chains {
        match uniswap.get_chain_config(chain) {
            Ok(config) => {
                println!("   {} ✅", chain);
                println!("     Chain ID: {}", config.chain_id);
                println!("     Quoter: {}", config.quoter_address);
                println!("     RPC: {}", config.rpc_url);
            }
            Err(e) => {
                println!("   {} ❌ Error: {}", chain, e);
            }
        }
    }
    
    // Test 5: Test token list fetching (with timeout)
    println!("\n🪙 Test 5: Testing token list fetching...");
    let test_chain = "ethereum";
    
    println!("   Fetching token list for {}...", test_chain);
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        uniswap.fetch_token_list(test_chain)
    ).await {
        Ok(Ok(tokens)) => {
            println!("   ✅ Successfully fetched {} tokens", tokens.len());
            
            // Show first 5 tokens
            println!("   First 5 tokens:");
            for (i, token) in tokens.iter().take(5).enumerate() {
                println!("     {}. {} ({}) - {}", i+1, token.symbol, token.address, token.decimals);
            }
            
            // Look for common tokens
            let common_tokens = vec!["ETH", "WETH", "USDC", "USDT", "DAI"];
            println!("   Common tokens found:");
            for symbol in common_tokens {
                if let Some(token) = tokens.iter().find(|t| t.symbol.to_uppercase() == symbol) {
                    println!("     {} ✅ {} ({})", symbol, token.address, token.decimals);
                } else {
                    println!("     {} ❌ Not found", symbol);
                }
            }
        }
        Ok(Err(e)) => {
            println!("   ❌ Token list fetch failed: {}", e);
        }
        Err(_) => {
            println!("   ⏰ Token list fetch timed out (>10s)");
        }
    }
    
    // Test 6: Test token address lookup
    println!("\n🔍 Test 6: Testing token address lookup...");
    let test_tokens = vec!["WETH", "USDC", "USDT", "DAI"];
    
    for token_symbol in test_tokens {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            uniswap.get_token_address(token_symbol, "ethereum")
        ).await {
            Ok(Ok((address, decimals))) => {
                println!("   {} ✅ {} (decimals: {})", token_symbol, address, decimals);
            }
            Ok(Err(e)) => {
                println!("   {} ❌ Error: {}", token_symbol, e);
            }
            Err(_) => {
                println!("   {} ⏰ Lookup timed out", token_symbol);
            }
        }
    }
    
    // Test 7: Test pair support
    println!("\n🔗 Test 7: Testing pair support...");
    let test_pairs = vec![
        ("WETH", "USDC"),
        ("USDC", "USDT"),
        ("DAI", "USDC"),
        ("WETH", "DAI"),
        ("INVALID", "TOKEN"),
    ];
    
    for (token_in, token_out) in test_pairs {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            uniswap.is_pair_supported(token_in, token_out)
        ).await {
            Ok(supported) => {
                println!("   {}/{} {}", token_in, token_out, if supported { "✅ Supported" } else { "❌ Not supported" });
            }
            Err(_) => {
                println!("   {}/{} ⏰ Check timed out", token_in, token_out);
            }
        }
    }
    
    // Test 8: Test quote generation (if we have working token addresses)
    println!("\n💱 Test 8: Testing quote generation...");
    
    let quote_params = QuoteParams {
        token_in: "USDC".to_string(),
        token_out: "WETH".to_string(),
        amount_in: "1000".to_string(), // 1000 USDC
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };
    
    println!("   Requesting quote: {} {} -> {} on {}", 
             quote_params.amount_in, quote_params.token_in, 
             quote_params.token_out, quote_params.chain.as_ref().unwrap());
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        uniswap.get_quote(&quote_params)
    ).await {
        Ok(Ok(route)) => {
            println!("   ✅ Quote successful!");
            println!("     DEX: {}", route.dex);
            println!("     Amount out: {}", route.amount_out);
            println!("     Gas estimate: {}", route.gas_used);
            println!("     Percentage: {}%", route.percentage);
        }
        Ok(Err(e)) => {
            println!("   ❌ Quote failed: {}", e);
        }
        Err(_) => {
            println!("   ⏰ Quote request timed out (>15s)");
        }
    }
    
    // Test 9: Test gas estimation
    println!("\n⛽ Test 9: Testing gas estimation...");
    let chains = vec!["ethereum", "polygon", "arbitrum", "optimism", "base"];
    for chain in chains {
        let gas_estimate = uniswap.estimated_gas(chain);
        println!("   {}: {} gas", chain, gas_estimate);
    }
    
    println!("\n🎉 Uniswap DEX Individual Testing Complete!");
    println!("=" .repeat(50));
    
    Ok(())
}
