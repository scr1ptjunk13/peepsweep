use std::sync::Arc;
use tokio;
use reqwest::Client;

// Import our modules
use bralaladex_backend::crosschain::portfolio_manager::PortfolioManager;
use bralaladex_backend::crosschain::arbitrage_detector::ArbitrageDetector;
use bralaladex_backend::bridges::BridgeManager;
use bralaladex_backend::dexes::DexManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing REAL functionality with REAL blockchain data...\n");

    // Test 1: Real Price Data from CoinGecko
    println!("📊 Testing Real Price Data from CoinGecko API:");
    let client = Client::new();
    let url = "https://api.coingecko.com/api/v3/simple/price?ids=ethereum,bitcoin,usd-coin&vs_currencies=usd";
    
    let response = client.get(url).send().await?;
    let price_data: serde_json::Value = response.json().await?;
    
    println!("✅ ETH Price: ${:.2}", price_data["ethereum"]["usd"].as_f64().unwrap_or(0.0));
    println!("✅ BTC Price: ${:.2}", price_data["bitcoin"]["usd"].as_f64().unwrap_or(0.0));
    println!("✅ USDC Price: ${:.4}", price_data["usd-coin"]["usd"].as_f64().unwrap_or(0.0));
    println!();

    // Test 2: Real Ethereum Balance Check
    println!("🔗 Testing Real Ethereum RPC Call:");
    let eth_rpc_url = "https://eth.llamarpc.com";
    let vitalik_address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // Vitalik's address
    
    let rpc_payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getBalance",
        "params": [vitalik_address, "latest"],
        "id": 1
    });

    let response = client
        .post(eth_rpc_url)
        .json(&rpc_payload)
        .send()
        .await?;

    let rpc_result: serde_json::Value = response.json().await?;
    
    if let Some(balance_hex) = rpc_result["result"].as_str() {
        let balance_wei = u128::from_str_radix(&balance_hex[2..], 16)?;
        let balance_eth = balance_wei as f64 / 1e18;
        println!("✅ Vitalik's ETH Balance: {:.4} ETH", balance_eth);
    }
    println!();

    // Test 3: Real Portfolio Manager with Live Data
    println!("💼 Testing Real Portfolio Manager:");
    let portfolio_manager = PortfolioManager::new();
    
    // Test getting chain balance for a real address
    match portfolio_manager.get_chain_balance_detailed(vitalik_address, 1).await {
        Ok(balance_response) => {
            println!("✅ Chain: {}", balance_response.chain_name);
            println!("✅ Native Token: {} {}", 
                balance_response.native_balance.balance_formatted, 
                balance_response.native_balance.symbol
            );
            println!("✅ Native Value: ${:.2}", balance_response.native_balance.value_usd);
            println!("✅ Total Value: ${:.2}", balance_response.total_value_usd);
            
            if !balance_response.token_balances.is_empty() {
                println!("✅ ERC-20 Tokens found: {}", balance_response.token_balances.len());
                for token in &balance_response.token_balances[..3.min(balance_response.token_balances.len())] {
                    println!("   - {} {}: ${:.2}", 
                        token.balance_formatted, 
                        token.symbol, 
                        token.value_usd
                    );
                }
            }
        },
        Err(e) => println!("❌ Portfolio test failed: {}", e),
    }
    println!();

    // Test 4: Real Arbitrage Detection
    println!("🔍 Testing Real Arbitrage Detection:");
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    let mut arbitrage_detector = ArbitrageDetector::new(bridge_manager, dex_manager);
    
    match arbitrage_detector.update_price_data().await {
        Ok(_) => {
            println!("✅ Price data updated successfully");
            
            // Check for real price differences
            let anomalies = arbitrage_detector.detect_price_anomalies("USDC", 0.5);
            if !anomalies.is_empty() {
                println!("✅ Found {} price anomalies:", anomalies.len());
                for anomaly in &anomalies[..3.min(anomalies.len())] {
                    println!("   - {}: {:.4}% deviation on {}", 
                        anomaly.token, 
                        anomaly.deviation_percentage * 100.0,
                        anomaly.chain_name
                    );
                }
            } else {
                println!("✅ No significant price anomalies detected");
            }
        },
        Err(e) => println!("❌ Arbitrage detection failed: {}", e),
    }
    println!();

    // Test 5: Real Cross-Chain Price Comparison
    println!("🌉 Testing Real Cross-Chain Price Comparison:");
    
    // Get USDC prices on different chains
    let chains_to_test = vec![
        (1, "Ethereum"),
        (137, "Polygon"),
        (56, "BSC"),
    ];
    
    for (chain_id, chain_name) in chains_to_test {
        match arbitrage_detector.get_token_price_from_dex(chain_id, "USDC").await {
            Ok(price) => println!("✅ USDC on {}: ${:.6}", chain_name, price),
            Err(e) => println!("❌ Failed to get USDC price on {}: {}", chain_name, e),
        }
    }
    
    println!("\n🎉 REAL FUNCTIONALITY TEST COMPLETE!");
    println!("All tests use LIVE blockchain data and REAL API calls - NO MOCKS!");
    
    Ok(())
}
