// src/bin/test_vitalik.rs - Direct test of Uniswap V3 position fetching for vitalik.eth
use std::sync::Arc;
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use alloy::primitives::Address;
use peepsweep_backend::fetchers::{
    config_parser::ConfigParser,
    generic_fetcher::GenericFetcher,
    orchestrator::PositionOrchestrator,
};
use peepsweep_backend::cache::CacheManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    println!("🔍 Testing Uniswap V3 position fetching for vitalik.eth");
    
    // vitalik.eth address
    let vitalik_address: Address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".parse()?;
    println!("📍 Address: {:?}", vitalik_address);

    // Setup HTTP provider (no database needed)
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string());
    
    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse()?);
    let provider = Arc::new(provider);

    // Setup cache (in-memory for testing)
    let cache_config = peepsweep_backend::cache::CacheConfig {
        redis_url: "redis://localhost:6379".to_string(),
        default_ttl: 300,
        max_connections: 10,
    };
    
    let cache_manager = match CacheManager::new(cache_config).await {
        Ok(cache) => Arc::new(cache),
        Err(_) => {
            println!("⚠️  Redis not available, using mock cache");
            Arc::new(peepsweep_backend::cache::MockCacheManager::new())
        }
    };

    // Load protocol configs
    let config_parser = ConfigParser::new("configs/protocols".to_string());
    let configs = config_parser.load_all_configs().await?;
    println!("📋 Loaded {} protocol configs", configs.len());

    // Create generic fetcher
    let generic_fetcher = GenericFetcher::new(
        provider.clone(),
        cache_manager.clone(),
        configs,
    );

    // Create orchestrator
    let orchestrator = PositionOrchestrator::new(
        generic_fetcher,
        cache_manager.clone(),
    );

    println!("🚀 Fetching positions for vitalik.eth on Ethereum mainnet...");
    
    // Test Uniswap V3 specifically
    match orchestrator.get_user_positions(1, vitalik_address).await {
        Ok(summary) => {
            println!("✅ SUCCESS! Found {} positions", summary.positions.len());
            println!("📊 Protocols: {:?}", summary.protocol_stats.keys().collect::<Vec<_>>());
            println!("💰 Total Value: ${:.2}", summary.total_value_usd);
            
            // Show first few positions
            for (i, position) in summary.positions.iter().take(3).enumerate() {
                println!("Position {}: {} - {}/{} - ${:.2}", 
                    i + 1,
                    position.protocol,
                    position.token0.symbol,
                    position.token1.symbol,
                    position.value_usd
                );
            }
        }
        Err(e) => {
            println!("❌ FAILED: {}", e);
            return Err(e.into());
        }
    }

    println!("🎉 Test completed successfully!");
    Ok(())
}
