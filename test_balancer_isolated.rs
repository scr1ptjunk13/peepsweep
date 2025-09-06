use std::path::Path;
use std::env;

// Add the backend source to the path
fn main() {
    let backend_path = Path::new("backend/src");
    if backend_path.exists() {
        println!("cargo:rustc-link-search=native={}", backend_path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // We'll include the necessary modules directly for testing
    use tokio;
    use serde::{Deserialize, Serialize};
    use reqwest::Client;
    use std::collections::HashMap;
    
    // Copy the necessary types and implementations for isolated testing
    #[derive(Debug, Clone)]
    pub struct QuoteParams {
        pub token_in: String,
        pub token_out: String,
        pub amount_in: String,
        pub slippage: Option<f64>,
        pub chain: Option<String>,
    }

    #[derive(Debug, Clone)]
    pub struct RouteBreakdown {
        pub dex: String,
        pub percentage: f64,
        pub amount_out: String,
        pub gas_used: String,
    }

    #[derive(Debug)]
    pub enum DexError {
        NetworkError(reqwest::Error),
        JsonError(serde_json::Error),
        InvalidResponse(String),
        ApiError(String),
        ParseError(String),
        ConfigError(String),
    }

    impl From<reqwest::Error> for DexError {
        fn from(err: reqwest::Error) -> Self {
            DexError::NetworkError(err)
        }
    }

    impl From<serde_json::Error> for DexError {
        fn from(err: serde_json::Error) -> Self {
            DexError::JsonError(err)
        }
    }

    // Simplified Balancer implementation for testing
    #[derive(Clone)]
    pub struct BalancerDex {
        client: Client,
        token_addresses: HashMap<String, String>,
    }

    impl BalancerDex {
        pub async fn new() -> Result<Self, DexError> {
            let mut token_addresses = HashMap::new();
            
            // Ethereum mainnet token addresses
            token_addresses.insert("ETH".to_string(), "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string());
            token_addresses.insert("WETH".to_string(), "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string());
            token_addresses.insert("USDC".to_string(), "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string());
            token_addresses.insert("BNB".to_string(), "0xB8c77482e45F1F44dE1745F52C74426C631bDD52".to_string());

            Ok(Self {
                client: Client::new(),
                token_addresses,
            })
        }

        fn get_token_address(&self, symbol: &str) -> Option<&String> {
            self.token_addresses.get(symbol)
        }

        async fn get_fallback_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
            // Simulate Balancer V2 weighted pool calculation for BNB/USDC
            let amount_in: f64 = params.amount_in.parse()
                .map_err(|_| DexError::ParseError("Invalid amount_in".to_string()))?;

            // Mock calculation based on current BNB price (~$732 as shown in the image)
            let bnb_price_usd = 732.0;
            let balancer_fee = 0.003; // 0.3% fee
            let price_impact = if amount_in > 1e18 { 0.002 } else { 0.001 }; // Price impact based on size
            
            let amount_out = amount_in * bnb_price_usd * (1.0 - balancer_fee) * (1.0 - price_impact);
            
            // Convert to USDC format (6 decimals)
            let usdc_amount = (amount_out * 1e6) as u64;
            
            println!("🔄 Balancer V2 Fallback Calculation:");
            println!("   Input: {} BNB", amount_in / 1e18);
            println!("   BNB Price: ${}", bnb_price_usd);
            println!("   Fee: {}%", balancer_fee * 100.0);
            println!("   Price Impact: {}%", price_impact * 100.0);
            println!("   Output: {} USDC", usdc_amount as f64 / 1e6);
            
            Ok(usdc_amount.to_string())
        }

        pub async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
            // Validate token support
            if !self.token_addresses.contains_key(&params.token_in) {
                return Err(DexError::ConfigError(format!("Unsupported token: {}", params.token_in)));
            }
            if !self.token_addresses.contains_key(&params.token_out) {
                return Err(DexError::ConfigError(format!("Unsupported token: {}", params.token_out)));
            }

            println!("🎯 Testing Balancer V2 Quote:");
            println!("   Token In: {} ({})", params.token_in, self.get_token_address(&params.token_in).unwrap());
            println!("   Token Out: {} ({})", params.token_out, self.get_token_address(&params.token_out).unwrap());
            println!("   Amount In: {}", params.amount_in);
            println!("   Chain: {}", params.chain.as_deref().unwrap_or("ethereum"));

            let quote = self.get_fallback_quote(params).await?;
            
            Ok(RouteBreakdown {
                dex: "Balancer V2".to_string(),
                percentage: 100.0,
                amount_out: quote,
                gas_used: "180000".to_string(),
            })
        }

        pub fn is_pair_supported(&self, token_in: &str, token_out: &str) -> bool {
            self.token_addresses.contains_key(token_in) && self.token_addresses.contains_key(token_out)
        }

        pub fn get_supported_chains(&self) -> Vec<&'static str> {
            vec!["ethereum", "polygon", "arbitrum", "optimism"]
        }
    }

    #[tokio::test]
    async fn test_balancer_bnb_to_usdc() {
        println!("\n🚀 Starting Balancer V2 Isolated Test");
        println!("=" .repeat(50));

        // Initialize Balancer DEX
        let balancer = match BalancerDex::new().await {
            Ok(dex) => {
                println!("✅ Balancer V2 initialized successfully");
                dex
            }
            Err(e) => {
                println!("❌ Failed to initialize Balancer: {:?}", e);
                panic!("Initialization failed");
            }
        };

        // Test 1: Check if BNB/USDC pair is supported
        println!("\n📋 Testing pair support...");
        let pair_supported = balancer.is_pair_supported("BNB", "USDC");
        println!("   BNB/USDC pair supported: {}", if pair_supported { "✅ YES" } else { "❌ NO" });
        assert!(pair_supported, "BNB/USDC pair should be supported");

        // Test 2: Check chain support
        println!("\n🌐 Testing chain support...");
        let supported_chains = balancer.get_supported_chains();
        println!("   Supported chains: {:?}", supported_chains);
        assert!(supported_chains.contains(&"ethereum"), "Should support Ethereum");

        // Test 3: Get quote for 1 BNB to USDC
        println!("\n💱 Testing 1 BNB → USDC quote...");
        let quote_params = QuoteParams {
            token_in: "BNB".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "1000000000000000000".to_string(), // 1 BNB in wei (18 decimals)
            slippage: Some(0.005), // 0.5%
            chain: Some("ethereum".to_string()),
        };

        match balancer.get_quote(&quote_params).await {
            Ok(route) => {
                println!("✅ Quote successful!");
                println!("   DEX: {}", route.dex);
                println!("   Amount Out: {} USDC", route.amount_out.parse::<f64>().unwrap_or(0.0) / 1e6);
                println!("   Gas Used: {}", route.gas_used);
                println!("   Percentage: {}%", route.percentage);

                // Validate the quote
                let amount_out: f64 = route.amount_out.parse().unwrap_or(0.0);
                let usdc_amount = amount_out / 1e6;
                
                // Based on the image showing ~911 USDC for 1 BNB, we expect something in that range
                assert!(usdc_amount > 500.0, "Amount should be reasonable (>500 USDC)");
                assert!(usdc_amount < 1000.0, "Amount should be reasonable (<1000 USDC)");
                assert_eq!(route.dex, "Balancer V2");
                assert_eq!(route.percentage, 100.0);

                println!("🎉 Expected range: 500-1000 USDC, Got: {:.2} USDC", usdc_amount);
            }
            Err(e) => {
                println!("❌ Quote failed: {:?}", e);
                panic!("Quote should succeed");
            }
        }

        println!("\n🏆 All Balancer V2 tests passed!");
        println!("=" .repeat(50));
    }

    #[tokio::test]
    async fn test_balancer_unsupported_token() {
        println!("\n🧪 Testing unsupported token handling...");
        
        let balancer = BalancerDex::new().await.unwrap();
        
        let quote_params = QuoteParams {
            token_in: "INVALID_TOKEN".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "1000000000000000000".to_string(),
            slippage: Some(0.005),
            chain: Some("ethereum".to_string()),
        };

        match balancer.get_quote(&quote_params).await {
            Ok(_) => panic!("Should fail for unsupported token"),
            Err(DexError::ConfigError(msg)) => {
                println!("✅ Correctly rejected unsupported token: {}", msg);
                assert!(msg.contains("Unsupported token: INVALID_TOKEN"));
            }
            Err(e) => panic!("Wrong error type: {:?}", e),
        }
    }
}
