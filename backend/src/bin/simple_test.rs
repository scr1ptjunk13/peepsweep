// src/bin/simple_test.rs - Minimal test without database dependencies
use std::sync::Arc;
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use alloy::primitives::Address;
use alloy::contract::Contract;
use alloy::sol;

// Uniswap V3 NonfungiblePositionManager ABI (minimal)
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract NonfungiblePositionManager {
        function balanceOf(address owner) external view returns (uint256);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
        function positions(uint256 tokenId) external view returns (
            uint96 nonce,
            address operator,
            address token0,
            address token1,
            uint24 fee,
            int24 tickLower,
            int24 tickUpper,
            uint128 liquidity,
            uint256 feeGrowthInside0LastX128,
            uint256 feeGrowthInside1LastX128,
            uint128 tokensOwed0,
            uint128 tokensOwed1
        );
    }
}

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

    // Setup HTTP provider
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string());
    
    println!("🌐 Using RPC: {}", rpc_url);
    
    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse()?);

    // Uniswap V3 NonfungiblePositionManager address on Ethereum
    let nft_manager_address: Address = "0xC36442b4a4522E871399CD717aBDD847Ab11FE88".parse()?;
    
    // Create contract instance
    let contract = NonfungiblePositionManager::new(nft_manager_address, &provider);

    println!("🚀 Fetching NFT balance for vitalik.eth...");
    
    // Get NFT balance
    let balance = contract.balanceOf(vitalik_address).call().await?;
    println!("✅ Found {} Uniswap V3 positions", balance._0);

    if balance._0 > 0 {
        println!("📋 Fetching position details...");
        
        // Get first few token IDs
        let max_positions = std::cmp::min(balance._0.to::<u64>(), 5);
        
        for i in 0..max_positions {
            match contract.tokenOfOwnerByIndex(vitalik_address, alloy::primitives::U256::from(i)).call().await {
                Ok(token_id) => {
                    println!("🏷️  Token ID {}: {}", i, token_id._0);
                    
                    // Get position details
                    match contract.positions(token_id._0).call().await {
                        Ok(position) => {
                            println!("   Token0: {:?}", position.token0);
                            println!("   Token1: {:?}", position.token1);
                            println!("   Fee: {}", position.fee);
                            println!("   Liquidity: {}", position.liquidity);
                            println!("   Tick Range: {} to {}", position.tickLower, position.tickUpper);
                            println!("   ---");
                        }
                        Err(e) => println!("   ❌ Failed to get position details: {}", e),
                    }
                }
                Err(e) => println!("❌ Failed to get token ID {}: {}", i, e),
            }
        }
    }

    println!("🎉 Test completed!");
    Ok(())
}
