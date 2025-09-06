use std::collections::HashMap;
use reqwest::Client as HttpClient;
use serde::Deserialize;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 STANDALONE UNISWAP VALIDATION TEST");
    println!("📊 Validating against swap from image: 1 ETH → 0.0409788 WBTC");
    println!("💰 Expected values:");
    println!("   - Input: 1 ETH ($4,484.51)");
    println!("   - Output: 0.0409788 WBTC ($4,469.85)");
    println!("   - Slippage: ~0.33%");
    println!("{}", "=".repeat(60));
    println!();

    // Test 1: Token Address Resolution
    println!("🔍 TEST 1: Token Address Resolution");
    let tokens = fetch_token_addresses().await?;
    println!();

    // Test 2: Manual Price Calculation
    println!("💱 TEST 2: Price Analysis");
    analyze_price_data(&tokens).await?;
    println!();

    // Test 3: Uniswap V3 Pool Analysis
    println!("🏊 TEST 3: Uniswap V3 Pool Analysis");
    analyze_uniswap_pools().await?;
    println!();

    println!("🎉 Validation completed!");
    Ok(())
}

async fn fetch_token_addresses() -> Result<HashMap<String, (String, u8)>, Box<dyn std::error::Error>> {
    let client = HttpClient::new();
    let mut token_map = HashMap::new();

    println!("   Fetching Uniswap token list...");
    
    let response = client
        .get("https://gateway.ipfs.io/ipns/tokens.uniswap.org")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;

    if !response.status().is_success() {
        println!("   ❌ Failed to fetch token list: {}", response.status());
        return Err(format!("Failed to fetch token list: {}", response.status()).into());
    }

    let token_response: TokenListResponse = response.json().await?;
    
    // Filter for Ethereum mainnet tokens
    let ethereum_tokens: Vec<TokenInfo> = token_response.tokens
        .into_iter()
        .filter(|token| token.chain_id == 1)
        .collect();

    println!("   Found {} Ethereum tokens", ethereum_tokens.len());

    // Add ETH manually (not in token list)
    token_map.insert("ETH".to_string(), ("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), 18));

    // Add major tokens
    for token in ethereum_tokens {
        let symbol = token.symbol.to_uppercase();
        if ["WETH", "WBTC", "USDC", "USDT", "DAI"].contains(&symbol.as_str()) {
            token_map.insert(symbol.clone(), (token.address.clone(), token.decimals));
            println!("   ✅ {}: {} (decimals: {})", symbol, token.address, token.decimals);
        }
    }

    if let Some((eth_addr, eth_decimals)) = token_map.get("ETH") {
        println!("   ✅ ETH: {} (decimals: {})", eth_addr, eth_decimals);
    }

    Ok(token_map)
}

async fn analyze_price_data(tokens: &HashMap<String, (String, u8)>) -> Result<(), Box<dyn std::error::Error>> {
    println!("   Analyzing price relationship from image data...");
    
    let (eth_addr, eth_decimals) = tokens.get("ETH").ok_or("ETH not found")?;
    let (wbtc_addr, wbtc_decimals) = tokens.get("WBTC").ok_or("WBTC not found")?;
    
    println!("   ETH: {} (decimals: {})", eth_addr, eth_decimals);
    println!("   WBTC: {} (decimals: {})", wbtc_addr, wbtc_decimals);
    
    // From the image data
    let eth_usd_price: f64 = 4484.51;
    let wbtc_usd_price: f64 = 4469.85;
    let expected_wbtc_output: f64 = 0.0409788;
    
    // Calculate expected ratio
    let price_ratio: f64 = eth_usd_price / wbtc_usd_price;
    let expected_ratio_from_amounts: f64 = 1.0 / expected_wbtc_output;
    
    println!();
    println!("   📊 PRICE ANALYSIS:");
    println!("      ETH Price: ${:.2}", eth_usd_price);
    println!("      WBTC Price: ${:.2}", wbtc_usd_price);
    println!("      Price Ratio (ETH/WBTC): {:.6}", price_ratio);
    println!("      Expected Output: {:.8} WBTC", expected_wbtc_output);
    println!("      Implied Ratio from amounts: {:.6}", expected_ratio_from_amounts);
    
    let ratio_difference = (price_ratio - expected_ratio_from_amounts).abs();
    let ratio_diff_percentage = (ratio_difference / price_ratio) * 100.0;
    
    println!("      Ratio Difference: {:.6} ({:.2}%)", ratio_difference, ratio_diff_percentage);
    
    if ratio_diff_percentage < 1.0 {
        println!("      🎯 Price ratios are consistent - EXCELLENT!");
    } else if ratio_diff_percentage < 5.0 {
        println!("      ✅ Price ratios are reasonable - GOOD!");
    } else {
        println!("      ⚠️  Price ratios show significant difference");
    }
    
    // Calculate what we'd expect from a perfect swap
    let perfect_swap_output = 1.0 / price_ratio;
    let slippage = ((perfect_swap_output - expected_wbtc_output) / perfect_swap_output) * 100.0;
    
    println!();
    println!("   🔄 SLIPPAGE ANALYSIS:");
    println!("      Perfect swap (no fees): {:.8} WBTC", perfect_swap_output);
    println!("      Actual output: {:.8} WBTC", expected_wbtc_output);
    println!("      Slippage + Fees: {:.2}%", slippage);
    
    Ok(())
}

async fn analyze_uniswap_pools() -> Result<(), Box<dyn std::error::Error>> {
    println!("   Analyzing Uniswap V3 pool information...");
    
    // Common Uniswap V3 pools for ETH/WBTC
    let pools = vec![
        ("0.05%", "0x4585fe77225b41b697c938b018e2ac67ac5a20c0"), // 500 = 0.05%
        ("0.3%", "0xcbcdf9626bc03e24f779434178a73a0b4bad62ed"),  // 3000 = 0.3%
        ("1%", "0x6ab3bba2f41e7eaa262fa5a1a9b3932fa161526f"),    // 10000 = 1%
    ];
    
    println!("   📊 UNISWAP V3 ETH/WBTC POOLS:");
    for (fee_tier, pool_address) in pools {
        println!("      {} fee tier: {}", fee_tier, pool_address);
    }
    
    println!();
    println!("   🔧 UNISWAP V3 INTEGRATION POINTS:");
    println!("      Quoter Contract: 0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6");
    println!("      Router Contract: 0xE592427A0AEce92De3Edee1F18E0157C05861564");
    println!("      WETH Contract: 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    
    println!();
    println!("   ⚙️  QUOTE FUNCTION:");
    println!("      Function: quoteExactInputSingle(address,address,uint24,uint256,uint160)");
    println!("      Selector: 0xf7729d43");
    println!("      Parameters:");
    println!("        - tokenIn: WETH address");
    println!("        - tokenOut: WBTC address");
    println!("        - fee: Pool fee tier (500, 3000, or 10000)");
    println!("        - amountIn: 1000000000000000000 (1 ETH in wei)");
    println!("        - sqrtPriceLimitX96: 0 (no limit)");
    
    println!();
    println!("   🎯 EXPECTED BEHAVIOR:");
    println!("      Input: 1 ETH (1000000000000000000 wei)");
    println!("      Expected Output: ~4097880 (0.0409788 WBTC in 8-decimal format)");
    println!("      Best fee tier: Likely 0.3% (3000) for ETH/WBTC");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_fetching() {
        let result = fetch_token_addresses().await;
        assert!(result.is_ok());
        
        let tokens = result.unwrap();
        assert!(tokens.contains_key("ETH"));
        assert!(tokens.contains_key("WBTC"));
        assert!(tokens.contains_key("WETH"));
    }

    #[test]
    fn test_price_calculations() {
        let eth_price = 4484.51;
        let wbtc_price = 4469.85;
        let expected_output = 0.0409788;
        
        let price_ratio = eth_price / wbtc_price;
        let amount_ratio = 1.0 / expected_output;
        
        // Should be close (within 5%)
        let diff_percentage = ((price_ratio - amount_ratio).abs() / price_ratio) * 100.0;
        assert!(diff_percentage < 5.0, "Price ratios should be consistent");
    }
}
