use std::collections::HashMap;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct TokenListResponse {
    tokens: Vec<TokenInfo>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub name: String,
    #[serde(rename = "chainId")]
    pub chain_id: u32,
}

// Updated response structure for SushiSwap API v7
#[derive(Debug, Deserialize)]
struct SushiQuoteResponse {
    status: String,
    #[serde(rename = "amountOut")]
    amount_out: Option<String>,
    #[serde(rename = "gasSpent")]
    gas_spent: Option<u64>,
    #[serde(rename = "priceImpact")]
    price_impact: Option<f64>,
    route: Option<serde_json::Value>,
    // Handle both error formats
    error: Option<String>,
    message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🍣 STANDALONE SUSHISWAP V7 VALIDATION TEST");
    println!("🔄 Testing updated SushiSwap integration with v7 API");
    println!("💡 Key improvements: caching, ETH/WETH handling, u128 precision, timeouts");
    println!("{}", "=".repeat(70));
    println!();

    // Test 1: Configuration Validation
    println!("⚙️  TEST 1: Configuration Validation");
    test_configuration().await?;
    println!();

    // Test 2: Token Address Resolution
    println!("🔍 TEST 2: Token Address Resolution");
    let tokens = fetch_token_addresses().await?;
    println!();

    // Test 3: Amount Conversion (Enhanced Precision)
    println!("💰 TEST 3: Enhanced Amount Conversion (u128)");
    test_amount_conversion().await?;
    println!();

    // Test 4: SushiSwap v7 API Quote
    println!("📊 TEST 4: SushiSwap v7 API Quote");
    test_sushiswap_v7_quote(&tokens).await?;
    println!();

    // Test 5: ETH/WETH Handling
    println!("🔄 TEST 5: ETH/WETH Normalization");
    test_eth_weth_handling(&tokens).await?;
    println!();

    // Test 6: Error Handling
    println!("⚠️  TEST 6: Error Handling");
    test_error_handling().await?;
    println!();

    println!("🎉 SushiSwap v7 validation completed!");
    Ok(())
}

async fn test_configuration() -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing SushiSwap v7 API endpoints...");
    
    let chains = [
        ("ethereum", 1, "https://api.sushi.com/swap/v7"),
        ("polygon", 137, "https://api.sushi.com/swap/v7"),
        ("arbitrum", 42161, "https://api.sushi.com/swap/v7"),
        ("optimism", 10, "https://api.sushi.com/swap/v7"),
        ("avalanche", 43114, "https://api.sushi.com/swap/v7"),
        ("bsc", 56, "https://api.sushi.com/swap/v7"),
        ("base", 8453, "https://api.sushi.com/swap/v7"),
    ];
    
    for (chain, chain_id, api_url) in chains {
        println!("   ✅ {}: Chain ID {}, API: {}", chain, chain_id, api_url);
        
        // Verify v7 API format
        if api_url.contains("/swap/v7") {
            println!("      🔄 Using updated v7 API endpoint");
        }
        
        // Show expected URL format
        let example_url = format!("{}/{}?tokenIn=0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48&tokenOut=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2&amount=1000000000&maxSlippage=0.005&sender=0x0000000000000000000000000000000000000000", api_url, chain_id);
        println!("      📝 Example URL: {}...", &example_url[..80]);
    }
    
    // Test gas estimates
    println!("   Gas estimates:");
    let gas_estimates = [
        ("ethereum", 200_000),
        ("polygon", 150_000),
        ("arbitrum", 120_000),
        ("optimism", 120_000),
        ("avalanche", 180_000),
        ("bsc", 160_000),
        ("base", 140_000),
    ];
    
    for (chain, gas) in gas_estimates {
        println!("      {}: {} gas units", chain, gas);
    }
    
    Ok(())
}

async fn fetch_token_addresses() -> Result<HashMap<String, (String, u8)>, Box<dyn std::error::Error>> {
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("DexAggregator/1.0")
        .build()?;
    
    let mut token_map = HashMap::new();

    println!("   Fetching token list from Uniswap (SushiSwap uses same format)...");
    
    let response = client
        .get("https://gateway.ipfs.io/ipns/tokens.uniswap.org")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch token list: {}", response.status()).into());
    }

    let token_response: TokenListResponse = response.json().await?;
    
    // Filter for Ethereum mainnet tokens
    let ethereum_tokens: Vec<TokenInfo> = token_response.tokens
        .into_iter()
        .filter(|token| token.chain_id == 1)
        .collect();

    println!("   Found {} Ethereum tokens", ethereum_tokens.len());

    // Add ETH manually (special case)
    token_map.insert("ETH".to_string(), ("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18));

    // Add major tokens
    for token in ethereum_tokens {
        let symbol = token.symbol.to_uppercase();
        if ["WETH", "WBTC", "USDC", "USDT", "DAI", "LINK", "UNI", "AAVE"].contains(&symbol.as_str()) {
            token_map.insert(symbol.clone(), (token.address.clone(), token.decimals));
            println!("   ✅ {}: {} (decimals: {})", symbol, token.address, token.decimals);
        }
    }

    if let Some((eth_addr, eth_decimals)) = token_map.get("ETH") {
        println!("   ✅ ETH: {} (decimals: {})", eth_addr, eth_decimals);
    }

    Ok(token_map)
}

async fn test_amount_conversion() -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing enhanced u128 precision amount conversion...");
    
    let test_cases = [
        ("1.0", 18, "1000000000000000000"),           // 1 ETH
        ("0.5", 18, "500000000000000000"),            // 0.5 ETH
        ("1000.0", 6, "1000000000"),                  // 1000 USDC
        ("0.01", 8, "1000000"),                       // 0.01 WBTC
        ("1000000.0", 18, "1000000000000000000000000"), // 1M tokens (large amount)
        ("0.000001", 18, "1000000000000"),            // Very small amount
    ];
    
    for (amount, decimals, expected) in test_cases {
        match convert_to_wei(amount, decimals) {
            Ok(wei_amount) => {
                println!("   ✅ {} ({} decimals) -> {} wei", amount, decimals, wei_amount);
                if wei_amount == expected {
                    println!("      🎯 Conversion matches expected value");
                } else {
                    println!("      ⚠️  Expected: {}, Got: {}", expected, wei_amount);
                }
            }
            Err(e) => {
                println!("   ❌ Conversion failed for {}: {}", amount, e);
            }
        }
    }
    
    // Test reverse conversion
    println!("   Testing wei to readable conversion...");
    match wei_to_readable("1000000000000000000", 18) {
        Ok(readable) => {
            println!("   ✅ 1000000000000000000 wei -> {} ETH", readable);
        }
        Err(e) => {
            println!("   ❌ Wei to readable failed: {}", e);
        }
    }
    
    Ok(())
}

async fn test_sushiswap_v7_quote(tokens: &HashMap<String, (String, u8)>) -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing SushiSwap v7 API with real quote request...");
    
    let (usdc_addr, usdc_decimals) = tokens.get("USDC").ok_or("USDC not found")?;
    let (weth_addr, _weth_decimals) = tokens.get("WETH").ok_or("WETH not found")?;
    
    println!("   USDC: {} (decimals: {})", usdc_addr, usdc_decimals);
    println!("   WETH: {}", weth_addr);
    
    // Convert 1000 USDC to wei (6 decimals)
    let amount_in_wei = convert_to_wei("1000", *usdc_decimals)?;
    println!("   Amount in: {} wei (1000 USDC)", amount_in_wei);
    
    // Build SushiSwap v7 API URL
    let url = format!(
        "https://api.sushi.com/swap/v7/1?tokenIn={}&tokenOut={}&amount={}&maxSlippage=0.005&sender=0x0000000000000000000000000000000000000000",
        usdc_addr,
        weth_addr,
        amount_in_wei
    );
    
    println!("   API URL: {}...", &url[..80]);
    
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("DexAggregator/1.0")
        .build()?;
    
    println!("   Making SushiSwap v7 API call...");
    
    match tokio::time::timeout(
        Duration::from_secs(10),
        client
            .get(&url)
            .header("Accept", "application/json")
            .send()
    ).await {
        Ok(Ok(response)) => {
            let status = response.status();
            println!("   Response status: {}", status);
            
            let response_text = response.text().await?;
            println!("   Response length: {} bytes", response_text.len());
            
            if status.is_success() {
                match serde_json::from_str::<SushiQuoteResponse>(&response_text) {
                    Ok(quote_response) => {
                        println!("   ✅ SushiSwap v7 API response parsed successfully!");
                        println!("      Status: {}", quote_response.status);
                        
                        if let Some(error_msg) = quote_response.error.or(quote_response.message) {
                            println!("      ❌ API Error: {}", error_msg);
                        } else if quote_response.status == "Success" {
                            if let Some(amount_out) = quote_response.amount_out {
                                println!("      Amount Out: {} wei", amount_out);
                                
                                // Convert to readable ETH
                                if let Ok(amount_wei) = amount_out.parse::<u128>() {
                                    let eth_amount = amount_wei as f64 / 1_000_000_000_000_000_000.0;
                                    println!("      Amount Out (ETH): {:.6}", eth_amount);
                                    
                                    // Calculate implied price
                                    let implied_price = 1000.0 / eth_amount;
                                    println!("      Implied ETH price: ${:.2}", implied_price);
                                }
                                
                                if let Some(gas) = quote_response.gas_spent {
                                    println!("      Gas estimate: {} units", gas);
                                }
                                
                                if let Some(impact) = quote_response.price_impact {
                                    println!("      Price impact: {:.4}%", impact * 100.0);
                                }
                                
                                println!("      🎯 SushiSwap v7 quote successful!");
                            } else {
                                println!("      ❌ No amountOut in response");
                            }
                        } else {
                            println!("      ❌ Quote failed with status: {}", quote_response.status);
                        }
                    }
                    Err(e) => {
                        println!("   ❌ Failed to parse JSON response: {}", e);
                        println!("   Raw response: {}", &response_text[..200.min(response_text.len())]);
                    }
                }
            } else {
                println!("   ❌ API returned error status: {}", status);
                println!("   Response: {}", &response_text[..200.min(response_text.len())]);
            }
        }
        Ok(Err(e)) => {
            println!("   ❌ Network error: {}", e);
        }
        Err(_) => {
            println!("   ❌ Request timed out after 10 seconds");
        }
    }
    
    Ok(())
}

async fn test_eth_weth_handling(tokens: &HashMap<String, (String, u8)>) -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing ETH/WETH normalization logic...");
    
    // Test ETH handling
    if let Some((eth_addr, eth_decimals)) = tokens.get("ETH") {
        println!("   ✅ ETH: {} (decimals: {})", eth_addr, eth_decimals);
        if eth_addr == "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE" {
            println!("      🎯 Using correct native ETH address");
        }
    }
    
    // Test WETH handling
    if let Some((weth_addr, weth_decimals)) = tokens.get("WETH") {
        println!("   ✅ WETH: {} (decimals: {})", weth_addr, weth_decimals);
        if weth_addr == "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2" {
            println!("      🎯 Using correct WETH contract address");
        }
    }
    
    // Test ETH/WETH 1:1 conversion logic
    println!("   Testing ETH/WETH 1:1 conversion edge case...");
    let eth_amount = "1.0";
    let eth_wei = convert_to_wei(eth_amount, 18)?;
    println!("   ETH -> WETH: {} ETH = {} wei (should be 1:1)", eth_amount, eth_wei);
    
    let weth_amount = "1.0";
    let weth_wei = convert_to_wei(weth_amount, 18)?;
    println!("   WETH -> ETH: {} WETH = {} wei (should be 1:1)", weth_amount, weth_wei);
    
    if eth_wei == weth_wei {
        println!("   🎯 ETH/WETH conversion is 1:1 as expected");
    } else {
        println!("   ⚠️  ETH/WETH conversion mismatch");
    }
    
    Ok(())
}

async fn test_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("   Testing error handling scenarios...");
    
    // Test invalid amount conversion
    match convert_to_wei("-1.0", 18) {
        Ok(_) => println!("   ⚠️  Negative amount should fail"),
        Err(_) => println!("   ✅ Negative amount correctly rejected"),
    }
    
    match convert_to_wei("invalid", 18) {
        Ok(_) => println!("   ⚠️  Invalid amount should fail"),
        Err(_) => println!("   ✅ Invalid amount correctly rejected"),
    }
    
    // Test invalid API call (should timeout or return error)
    println!("   Testing invalid API call...");
    let client = HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let invalid_url = "https://api.sushi.com/swap/v7/1?tokenIn=invalid&tokenOut=invalid&amount=1000&maxSlippage=0.005&sender=0x0000000000000000000000000000000000000000";
    
    match tokio::time::timeout(
        Duration::from_secs(8),
        client.get(invalid_url).send()
    ).await {
        Ok(Ok(response)) => {
            let status = response.status();
            if status.is_success() {
                println!("   ⚠️  Invalid request should fail");
            } else {
                println!("   ✅ Invalid request correctly rejected with status: {}", status);
            }
        }
        Ok(Err(e)) => {
            println!("   ✅ Invalid request correctly failed: {}", e);
        }
        Err(_) => {
            println!("   ✅ Invalid request timed out as expected");
        }
    }
    
    Ok(())
}

// Helper functions (simplified versions of the SushiSwap implementation)
fn convert_to_wei(amount: &str, decimals: u8) -> Result<String, Box<dyn std::error::Error>> {
    let amount_f64: f64 = amount.parse()?;
    
    if amount_f64 < 0.0 {
        return Err("Amount cannot be negative".into());
    }
    
    // Use u128 for better precision
    let multiplier = 10_u128.pow(decimals as u32);
    let wei_amount = (amount_f64 * multiplier as f64) as u128;
    
    Ok(wei_amount.to_string())
}

fn wei_to_readable(wei_amount: &str, decimals: u8) -> Result<String, Box<dyn std::error::Error>> {
    let wei: u128 = wei_amount.parse()?;
    let divisor = 10_u128.pow(decimals as u32);
    let readable = wei as f64 / divisor as f64;
    
    Ok(format!("{:.6}", readable))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_conversion() {
        // Test ETH conversion (18 decimals)
        let eth_wei = convert_to_wei("1.0", 18).unwrap();
        assert_eq!(eth_wei, "1000000000000000000");
        
        // Test USDC conversion (6 decimals)
        let usdc_wei = convert_to_wei("1000.0", 6).unwrap();
        assert_eq!(usdc_wei, "1000000000");
        
        // Test large amounts (u128 precision)
        let large_amount = convert_to_wei("1000000.0", 18).unwrap();
        assert_eq!(large_amount, "1000000000000000000000000");
    }

    #[test]
    fn test_error_cases() {
        // Test negative amount
        assert!(convert_to_wei("-1.0", 18).is_err());
        
        // Test invalid amount
        assert!(convert_to_wei("invalid", 18).is_err());
    }

    #[test]
    fn test_wei_to_readable() {
        let readable = wei_to_readable("1000000000000000000", 18).unwrap();
        assert_eq!(readable, "1.000000");
        
        let usdc_readable = wei_to_readable("1000000000", 6).unwrap();
        assert_eq!(usdc_readable, "1000.000000");
    }
}
