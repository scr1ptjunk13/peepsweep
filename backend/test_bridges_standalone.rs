use std::collections::HashMap;

// Simple standalone test for bridge system
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌉 Testing Bridge System Standalone...");
    
    // Test bridge trait compilation and basic functionality
    test_bridge_trait_system().await?;
    
    println!("🎉 Bridge system standalone test completed successfully!");
    Ok(())
}

async fn test_bridge_trait_system() -> Result<(), Box<dyn std::error::Error>> {
    use bralaladex_backend::bridges::{
        BridgeAggregator, BridgePreferences,
        hop_protocol::HopProtocol,
        across_protocol::AcrossProtocol,
        stargate_finance::StargateFinance,
        synapse_protocol::SynapseProtocol,
        polygon_bridge::PolygonBridge,
    };
    
    println!("✅ Bridge imports successful");
    
    // Initialize bridge aggregator
    let mut aggregator = BridgeAggregator::new();
    
    // Add bridge implementations
    aggregator.add_bridge(Box::new(HopProtocol::new()));
    aggregator.add_bridge(Box::new(AcrossProtocol::new()));
    aggregator.add_bridge(Box::new(StargateFinance::new()));
    aggregator.add_bridge(Box::new(SynapseProtocol::new()));
    aggregator.add_bridge(Box::new(PolygonBridge::new()));
    
    println!("✅ Added 5 bridge implementations");
    
    // Test getting quotes from all bridges
    let quotes = aggregator.get_all_quotes(
        1,     // Ethereum
        137,   // Polygon
        "0xA0b86a33E6441E6e80A7e1d6C3F5E3b4e6b6c8e1", // USDC
        "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", // USDC on Polygon
        "1000000", // 1 USDC (6 decimals)
        "0x742d35Cc6634C0532925a3b8D8c8c8c8c8c8c8c8", // User address
    ).await;
    
    println!("📊 Received {} quote responses", quotes.len());
    
    let mut successful_quotes = 0;
    for (i, quote_result) in quotes.iter().enumerate() {
        match quote_result {
            Ok(quote) => {
                println!("  ✅ {}: {} -> {} ({:.4}% fee, {}s)", 
                    quote.bridge_name,
                    quote.amount_in,
                    quote.amount_out,
                    quote.fee_percentage,
                    quote.estimated_time_seconds
                );
                successful_quotes += 1;
            }
            Err(e) => {
                println!("  ❌ Bridge {}: {}", i, e);
            }
        }
    }
    
    println!("✅ {} successful quotes out of {}", successful_quotes, quotes.len());
    
    // Test getting best quote with default preferences
    let preferences = BridgePreferences::default();
    match aggregator.get_best_quote(
        1,     // Ethereum
        137,   // Polygon
        "0xA0b86a33E6441E6e80A7e1d6C3F5E3b4e6b6c8e1", // USDC
        "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", // USDC on Polygon
        "1000000", // 1 USDC
        "0x742d35Cc6634C0532925a3b8D8c8c8c8c8c8c8c8",
        &preferences,
    ).await {
        Ok(best_quote) => {
            println!("🏆 Best quote: {} (Fee: {:.4}%, Time: {}s)", 
                best_quote.bridge_name,
                best_quote.fee_percentage,
                best_quote.estimated_time_seconds
            );
        }
        Err(e) => {
            println!("❌ Failed to get best quote: {}", e);
        }
    }
    
    // Test supported routes
    let supported_routes = aggregator.get_supported_routes(1, 137).await;
    println!("🛣️  Supported routes between Ethereum and Polygon: {:?}", supported_routes);
    
    // Test individual bridge functionality
    let hop = HopProtocol::new();
    let chains = hop.supported_chains();
    println!("🔗 Hop Protocol supports {} chains", chains.len());
    
    let across = AcrossProtocol::new();
    let is_supported = across.is_route_supported(
        1, 137, "0xA0b86a33E6441E6e80A7e1d6C3F5E3b4e6b6c8e1"
    ).await?;
    println!("🔗 Across Protocol ETH->Polygon USDC route supported: {}", is_supported);
    
    Ok(())
}
