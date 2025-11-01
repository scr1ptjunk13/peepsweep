use bralaladex_backend::token_db::TokenDatabase;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 TOKEN DATABASE TEST");
    println!("======================");
    
    // Initialize the token database
    let db = TokenDatabase::new();
    
    println!("✅ Database initialized successfully");
    
    // Test search functionality
    println!("\n🔍 Testing search functionality:");
    
    // Search for ETH
    let eth_results = db.search("eth", Some(1), 5);
    println!("📊 Search 'eth' on Ethereum: {} results", eth_results.len());
    for result in &eth_results {
        println!("   - {} ({}): {} - Score: {}", 
            result.token.symbol, 
            result.token.address,
            result.token.name,
            result.score
        );
    }
    
    // Search for USDC
    let usdc_results = db.search("usdc", Some(1), 3);
    println!("\n📊 Search 'usdc' on Ethereum: {} results", usdc_results.len());
    for result in &usdc_results {
        println!("   - {} ({}): {} - Score: {}", 
            result.token.symbol, 
            result.token.address,
            result.token.name,
            result.score
        );
    }
    
    // Test direct token lookup
    println!("\n🎯 Testing direct token lookup:");
    if let Some(weth) = db.get_token("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", 1) {
        println!("✅ Found WETH: {} - {}", weth.symbol, weth.name);
        if let Some(market_cap) = weth.market_cap_usd {
            println!("   💰 Market Cap: ${:.2}B", market_cap / 1_000_000_000.0);
        }
    } else {
        println!("❌ WETH not found");
    }
    
    // Test popular tokens
    println!("\n🏆 Top popular tokens on Ethereum:");
    let popular = db.get_popular_tokens(1);
    for (i, token) in popular.iter().take(5).enumerate() {
        println!("   {}. {} - {}", i + 1, token.symbol, token.name);
        if let Some(market_cap) = token.market_cap_usd {
            println!("      💰 ${:.2}B market cap", market_cap / 1_000_000_000.0);
        }
    }
    
    println!("\n🎉 All tests completed successfully!");
    
    Ok(())
}
