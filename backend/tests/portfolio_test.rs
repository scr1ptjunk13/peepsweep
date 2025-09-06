use bralaladex_backend::crosschain::portfolio_manager::{PortfolioManager, Portfolio, PortfolioSummary};

#[tokio::test]
async fn test_portfolio_manager_creation() {
    let manager = PortfolioManager::new();
    
    // Basic creation test - verify it doesn't panic
    assert!(true);
}

#[tokio::test]
async fn test_get_portfolio() {
    let manager = PortfolioManager::new();
    let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
    
    let portfolio = manager.get_portfolio(user_address).await.unwrap();
    
    assert_eq!(portfolio.user_address, user_address);
    assert!(!portfolio.balances.is_empty());
    assert!(portfolio.total_value_usd > 0.0);
    assert!(portfolio.last_updated > 0);
}

#[tokio::test]
async fn test_portfolio_caching() {
    let manager = PortfolioManager::new();
    let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
    
    // First call should fetch from "blockchain"
    let portfolio1 = manager.get_portfolio(user_address).await.unwrap();
    
    // Second call should return cached result
    let portfolio2 = manager.get_portfolio(user_address).await.unwrap();
    
    assert_eq!(portfolio1.last_updated, portfolio2.last_updated);
    assert_eq!(portfolio1.total_value_usd, portfolio2.total_value_usd);
}

#[tokio::test]
async fn test_get_portfolio_summary() {
    let manager = PortfolioManager::new();
    let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
    
    let summary = manager.get_portfolio_summary(user_address).await.unwrap();
    
    assert_eq!(summary.user_address, user_address);
    assert!(summary.total_value_usd > 0.0);
    assert!(!summary.chain_distribution.is_empty());
    assert!(!summary.top_tokens.is_empty());
}

#[tokio::test]
async fn test_token_price_cache() {
    let manager = PortfolioManager::new();
    
    // Update prices
    let prices = vec![
        (1, "0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5".to_string(), 1.0),
        (1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), 3300.0),
    ];
    
    manager.update_token_prices(prices).await.unwrap();
    
    // Verify prices are cached
    let usdc_price = manager.get_token_price(1, "0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5").await;
    assert_eq!(usdc_price, Some(1.0));
    
    let weth_price = manager.get_token_price(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").await;
    assert_eq!(weth_price, Some(3300.0));
    
    // Non-existent token should return None
    let unknown_price = manager.get_token_price(1, "0xUnknown").await;
    assert_eq!(unknown_price, None);
}
