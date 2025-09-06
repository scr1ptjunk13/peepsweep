use bralaladex_backend::dexes::manager::DexManager;
use bralaladex_backend::types::QuoteParams;

#[tokio::main]
async fn main() {
    println!("🔥 TIER 1 DEX AGGREGATION TEST - POLYGON");
    println!("=========================================\n");

    // Initialize DexManager with Tier 1 Direct Routes
    println!("🚀 Initializing DexManager with Tier 1 Direct Routes...");
    let dex_manager = match DexManager::init_tier1_direct_routes().await {
        Ok(manager) => {
            println!("   ✅ DexManager initialized successfully");
            manager
        }
        Err(e) => {
            println!("   ❌ Failed to initialize DexManager: {}", e);
            return;
        }
    };

    // Display loaded DEXes
    let dex_names = dex_manager.get_dex_names();
    println!("   📊 Total DEXes loaded: {}", dex_names.len());
    
    for (i, name) in dex_names.iter().enumerate() {
        println!("      {}. {}", i + 1, name);
    }

    println!("\n💰 TESTING: 1 WETH → USDC on Polygon");
    println!("=====================================");

    // Test parameters - 1 WETH with proper 18 decimals
    let quote_params = QuoteParams {
        token_in: "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string(), // WETH on Polygon
        token_out: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string(), // USDC on Polygon
        amount_in: "1000000000000000000".to_string(), // 1 WETH (18 decimals)
        slippage: Some(0.005), // 0.5%
    };

    println!("📋 Quote Parameters:");
    println!("   Token In:  {} (WETH)", quote_params.token_in);
    println!("   Token Out: {} (USDC)", quote_params.token_out);
    println!("   Amount In: {} (1 WETH)", quote_params.amount_in);
    println!("   Slippage:  {}%", quote_params.slippage.unwrap_or(0.005) * 100.0);

    println!("\n🔍 INDIVIDUAL DEX QUOTES:");
    println!("=========================");

    // Get individual quotes from each DEX for debugging
    let all_quotes = dex_manager.get_all_quotes(&quote_params).await;
    
    for (i, (dex_name, quote_result)) in all_quotes.iter().enumerate() {
        println!("{}. Testing {}...", i + 1, dex_name);
        
        match quote_result {
            Ok(quote) => {
                let amount_out_f64 = quote.amount_out.parse::<f64>().unwrap_or(0.0);
                let usdc_amount = amount_out_f64 / 1_000_000.0; // Convert from 6 decimals to human readable
                
                println!("   ✅ {}: {} USDC (raw: {})", 
                    dex_name, 
                    format!("{:.2}", usdc_amount),
                    quote.amount_out
                );
                println!("      Gas: {}, DEX: {}, Percentage: {}%", 
                    quote.gas_used, 
                    quote.dex,
                    quote.percentage
                );
            }
            Err(e) => {
                println!("   ❌ {}: FAILED - {}", dex_name, e);
            }
        }
        println!();
    }

    println!("🏆 TIER 1 AGGREGATED QUOTE:");
    println!("============================");

    // Get the final aggregated quote
    match dex_manager.get_tier1_direct_quote(&quote_params).await {
        Ok(final_quote) => {
            let amount_out_f64 = final_quote.amount_out.parse::<f64>().unwrap_or(0.0);
            let usdc_amount = amount_out_f64 / 1_000_000.0;
            
            println!("🎯 BEST QUOTE SELECTED:");
            println!("   DEX:        {}", final_quote.dex);
            println!("   Amount Out: {} USDC", format!("{:.2}", usdc_amount));
            println!("   Raw Amount: {}", final_quote.amount_out);
            println!("   Gas Est:    {}", final_quote.gas_used);
            println!("   Percentage: {}%", final_quote.percentage);
            
            // Calculate implied ETH price
            if amount_out_f64 > 0.0 {
                let eth_price = usdc_amount; // 1 ETH = X USDC
                println!("   Implied ETH Price: ${:.2}", eth_price);
            }
        }
        Err(e) => {
            println!("❌ TIER 1 AGGREGATION FAILED: {}", e);
        }
    }

    println!("\n📊 AGGREGATION ANALYSIS:");
    println!("========================");
    
    // Show all quotes sorted by amount
    let mut successful_quotes: Vec<_> = all_quotes.iter()
        .filter_map(|(name, result)| {
            if let Ok(quote) = result {
                Some((name.clone(), quote.clone()))
            } else {
                None
            }
        })
        .collect();
    
    // Sort by amount descending
    successful_quotes.sort_by(|a, b| {
        let amount_a = a.1.amount_out.parse::<f64>().unwrap_or(0.0);
        let amount_b = b.1.amount_out.parse::<f64>().unwrap_or(0.0);
        amount_b.partial_cmp(&amount_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    println!("📈 Quote Ranking (Amount Descending):");
    
    for (i, (dex_name, quote)) in successful_quotes.iter().enumerate() {
        let amount_out_f64 = quote.amount_out.parse::<f64>().unwrap_or(0.0);
        let usdc_amount = amount_out_f64 / 1_000_000.0;
        
        let rank_indicator = if i == 0 { "🥇" } else if i == 1 { "🥈" } else if i == 2 { "🥉" } else { "  " };
        
        println!("   {} {}. {} - {} USDC (raw: {})", 
            rank_indicator,
            i + 1, 
            dex_name, 
            format!("{:.2}", usdc_amount),
            quote.amount_out
        );
    }

    println!("\n✅ TIER 1 AGGREGATION TEST COMPLETE");
    println!("===================================");
}
