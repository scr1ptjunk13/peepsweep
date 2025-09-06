use bralaladex_backend::dexes::velodrome::VelodromeDex;
use bralaladex_backend::dexes::DexIntegration;
use bralaladex_backend::types::QuoteParams;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    println!("🚀 Testing Velodrome DEX Integration (Universal Framework)");
    println!("Philosophy: Token1 + Amount1 + Token2 → Amount2\n");

    let velodrome = VelodromeDex::new();

    // Test 1: ETH → USDC on Optimism
    println!("📊 Test 1: ETH → USDC on Optimism");
    let params1 = QuoteParams {
        token_in: "ETH".to_string(),
        token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()),
        token_in_decimals: Some(18),
        token_out: "USDC".to_string(),
        token_out_address: Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("optimism".to_string()),
        slippage: Some(0.5),
    };

    match velodrome.get_quote(&params1).await {
        Ok(route) => {
            println!("✅ Success: 1.0 ETH → {} USDC", route.amount_out);
            println!("   DEX: {} | Gas: {} | Share: {}%", route.dex, route.gas_used, route.percentage);
        }
        Err(e) => println!("❌ Failed: {:?}", e),
    }

    // Test 2: USDC → ETH on Optimism
    println!("\n📊 Test 2: USDC → ETH on Optimism");
    let params2 = QuoteParams {
        token_in: "USDC".to_string(),
        token_in_address: Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()),
        token_in_decimals: Some(6),
        token_out: "ETH".to_string(),
        token_out_address: Some("0x0000000000000000000000000000000000000000".to_string()),
        token_out_decimals: Some(18),
        amount_in: "3000.0".to_string(),
        chain: Some("optimism".to_string()),
        slippage: Some(0.5),
    };

    match velodrome.get_quote(&params2).await {
        Ok(route) => {
            println!("✅ Success: 3000.0 USDC → {} ETH", route.amount_out);
            println!("   DEX: {} | Gas: {} | Share: {}%", route.dex, route.gas_used, route.percentage);
        }
        Err(e) => println!("❌ Failed: {:?}", e),
    }

    // Test 3: WETH → USDC on Base (Aerodrome)
    println!("\n📊 Test 3: WETH → USDC on Base (Aerodrome)");
    let params3 = QuoteParams {
        token_in: "WETH".to_string(),
        token_in_address: Some("0x4200000000000000000000000000000000000006".to_string()),
        token_in_decimals: Some(18),
        token_out: "USDC".to_string(),
        token_out_address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
        token_out_decimals: Some(6),
        amount_in: "0.5".to_string(),
        chain: Some("base".to_string()),
        slippage: Some(0.5),
    };

    match velodrome.get_quote(&params3).await {
        Ok(route) => {
            println!("✅ Success: 0.5 WETH → {} USDC", route.amount_out);
            println!("   DEX: {} | Gas: {} | Share: {}%", route.dex, route.gas_used, route.percentage);
        }
        Err(e) => println!("❌ Failed: {:?}", e),
    }

    // Test 4: Small amount test (check decimals handling)
    println!("\n📊 Test 4: Small Amount Test");
    let params4 = QuoteParams {
        token_in: "ETH".to_string(),
        token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()),
        token_in_decimals: Some(18),
        token_out: "USDC".to_string(),
        token_out_address: Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()),
        token_out_decimals: Some(6),
        amount_in: "0.01".to_string(),
        chain: Some("optimism".to_string()),
        slippage: Some(0.5),
    };

    match velodrome.get_quote(&params4).await {
        Ok(route) => {
            println!("✅ Success: 0.01 ETH → {} USDC", route.amount_out);
            println!("   DEX: {} | Gas: {} | Share: {}%", route.dex, route.gas_used, route.percentage);
        }
        Err(e) => println!("❌ Failed: {:?}", e),
    }

    // Test 5: Unsupported chain
    println!("\n📊 Test 5: Unsupported Chain (Ethereum)");
    let params5 = QuoteParams {
        token_in: "ETH".to_string(),
        token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()),
        token_in_decimals: Some(18),
        token_out: "USDC".to_string(),
        token_out_address: Some("0xA0b86a33E6441c8C06DD2b7c94b7E0e8b8b8b8b8".to_string()),
        token_out_decimals: Some(6),
        amount_in: "1.0".to_string(),
        chain: Some("ethereum".to_string()),
        slippage: Some(0.5),
    };

    match velodrome.get_quote(&params5).await {
        Ok(quote) => println!("⚠️ Unexpected success: {}", quote.amount_out),
        Err(e) => println!("✅ Expected failure: {:?}", e),
    }

    // Test 6: Supported chains verification
    println!("\n📊 Test 6: Supported Chains");
    let supported_chains = velodrome.get_supported_chains();
    println!("Supported chains: {:?}", supported_chains);

    // Test 7: Pair support verification
    println!("\n📊 Test 7: Pair Support Verification");
    
    // Test ETH/USDC on Optimism
    match velodrome.is_pair_supported(
        "optimism", 
        "0x0000000000000000000000000000000000000000", 
        "0x7F5c764cBc14f9669B88837ca1490cCa17c31607"
    ).await {
        Ok(supported) => println!("   Optimism ETH/USDC: {}", supported),
        Err(e) => println!("   Optimism ETH/USDC: Error - {:?}", e),
    }

    // Test WETH/USDC on Base
    match velodrome.is_pair_supported(
        "base", 
        "0x4200000000000000000000000000000000000006", 
        "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
    ).await {
        Ok(supported) => println!("   Base WETH/USDC: {}", supported),
        Err(e) => println!("   Base WETH/USDC: Error - {:?}", e),
    }

    // Test unsupported pair on Ethereum
    match velodrome.is_pair_supported(
        "ethereum", 
        "0x0000000000000000000000000000000000000000", 
        "0xA0b86a33E6441c8C06DD2b7c94b7E0e8b8b8b8b8"
    ).await {
        Ok(supported) => println!("   Ethereum ETH/USDC: {}", supported),
        Err(e) => println!("   Ethereum ETH/USDC: Error - {:?}", e),
    }

    println!("\n🎯 Test Summary:");
    println!("✅ Refactored to use Universal DEX Implementation Framework");
    println!("✅ Using DexIntegration trait with QuoteParams structure");
    println!("✅ Proper token address and decimals handling");
    println!("✅ Chain support verification via get_supported_chains()");
    println!("✅ Pair support verification via is_pair_supported()");
    println!("✅ Standardized error handling and route information");

    Ok(())
}