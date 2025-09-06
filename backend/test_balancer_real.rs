#[cfg(test)]
mod tests {
    use super::*;
    use tokio;
    
    #[tokio::test]
    async fn test_balancer_real_api() {
        // Test with actual BNB token address on Ethereum
        let params = crate::types::QuoteParams {
            token_in: "0xB8c77482e45F1F44dE1745F52C74426C631bDD52".to_string(), // BNB on Ethereum
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(), // USDC on Ethereum
            amount_in: "1000000000000000000".to_string(), // 1 BNB
            slippage: Some(0.005),
            chain: Some("ethereum".to_string()),
        };

        let balancer = crate::dexes::balancer::BalancerDex::new().await.unwrap();
        
        match balancer.get_quote(&params).await {
            Ok(quote) => {
                println!("✅ Balancer Quote Success:");
                println!("   Amount Out: {}", quote.amount_out);
                println!("   Gas Used: {}", quote.gas_used);
            }
            Err(e) => {
                println!("❌ Balancer Quote Failed: {:?}", e);
            }
        }
    }

    #[tokio::test] 
    async fn test_balancer_polygon() {
        // Test on Polygon chain
        let params = crate::types::QuoteParams {
            token_in: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string(), // USDC on Polygon
            token_out: "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063".to_string(), // DAI on Polygon  
            amount_in: "1000000".to_string(), // 1 USDC (6 decimals)
            slippage: Some(0.005),
            chain: Some("polygon".to_string()),
        };

        let balancer = crate::dexes::balancer::BalancerDex::new().await.unwrap();
        
        match balancer.get_quote(&params).await {
            Ok(quote) => {
                println!("✅ Balancer Polygon Quote Success:");
                println!("   Amount Out: {}", quote.amount_out);
                println!("   Gas Used: {}", quote.gas_used);
            }
            Err(e) => {
                println!("❌ Balancer Polygon Quote Failed: {:?}", e);
            }
        }
    }
}
