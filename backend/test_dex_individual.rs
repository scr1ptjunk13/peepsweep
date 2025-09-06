use reqwest::Client;
use std::time::Duration;
use tokio;

#[tokio::main]
async fn main() {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    println!("🔍 RUTHLESS DEX INTEGRATION AUDIT");
    println!("=====================================\n");

    // Test 1inch API
    println!("1️⃣  Testing 1inch API...");
    let oneinch_url = "https://api.1inch.dev/swap/v6.0/1/quote?src=0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE&dst=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&amount=1000000000000000000";
    match client.get(oneinch_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ 1inch API: WORKING");
            } else {
                println!("   ❌ 1inch API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ 1inch API: FAILED - {}", e),
    }

    // Test SushiSwap API
    println!("2️⃣  Testing SushiSwap API...");
    let sushi_url = "https://api.sushi.com/quote/v7/1?tokenIn=0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE&tokenOut=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&amount=1000000000000000000&maxSlippage=0.005";
    match client.get(sushi_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ SushiSwap API: WORKING");
            } else {
                println!("   ❌ SushiSwap API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ SushiSwap API: FAILED - {}", e),
    }

    // Test Curve API
    println!("3️⃣  Testing Curve API...");
    let curve_url = "https://api.curve.fi/api/getPools/ethereum/main";
    match client.get(curve_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ Curve API: WORKING");
            } else {
                println!("   ❌ Curve API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ Curve API: FAILED - {}", e),
    }

    // Test Balancer API
    println!("4️⃣  Testing Balancer API...");
    let balancer_url = "https://api.balancer.fi/pools";
    match client.get(balancer_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ Balancer API: WORKING");
            } else {
                println!("   ❌ Balancer API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ Balancer API: FAILED - {}", e),
    }

    // Test 0x Protocol API
    println!("5️⃣  Testing 0x Protocol API...");
    let zeroex_url = "https://api.0x.org/swap/v1/quote?sellToken=ETH&buyToken=USDC&sellAmount=1000000000000000000";
    match client.get(zeroex_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ 0x Protocol API: WORKING");
            } else {
                println!("   ❌ 0x Protocol API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ 0x Protocol API: FAILED - {}", e),
    }

    // Test Paraswap API  
    println!("6️⃣  Testing Paraswap API...");
    let paraswap_url = "https://apiv5.paraswap.io/prices?srcToken=0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE&destToken=0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48&amount=1000000000000000000&srcDecimals=18&destDecimals=6&network=1";
    match client.get(paraswap_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("   ✅ Paraswap API: WORKING");
            } else {
                println!("   ❌ Paraswap API: FAILED - Status {}", resp.status());
            }
        }
        Err(e) => println!("   ❌ Paraswap API: FAILED - {}", e),
    }

    // Test CoW Protocol API
    println!("7️⃣  Testing CoW Protocol API...");
    let cow_url = "https://api.cow.fi/mainnet/api/v1/quote";
    match client.get(cow_url).send().await {
        Ok(resp) => {
            println!("   ⚠️  CoW Protocol: Status {} (requires POST)", resp.status());
        }
        Err(e) => println!("   ❌ CoW Protocol API: FAILED - {}", e),
    }

    println!("\n🚨 REALITY CHECK:");
    println!("Most of these integrations are MOCK implementations!");
    println!("They return hardcoded fallback quotes, not real API data.");
    println!("Only 1inch, SushiSwap, and 0x have real API calls.");
}
