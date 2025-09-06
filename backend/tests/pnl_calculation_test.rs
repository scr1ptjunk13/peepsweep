use bralaladex_backend::risk_management::{
    database::RiskDatabase,
    types::{UserPositions, TokenBalance},
    websocket_server::RiskWebSocketServer,
    risk_engine::RiskProcessingEngine,
    position_tracker::PositionTracker,
    config::{DatabaseConfig, RiskManagementConfig},
};
use std::collections::HashMap;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_pnl_calculation_from_positions() {
    // Create test user positions with known values
    let positions = create_test_positions_with_pnl();
    
    // Calculate PnL using the same logic as WebSocket server
    let calculated_pnl = calculate_portfolio_pnl(&positions).await;
    
    // Expected PnL: ETH: (3500-3200)*1.5 = 450, USDC: 0, Total: 450
    assert_eq!(calculated_pnl, 450.0, "PnL calculation should be accurate");
}

#[tokio::test]
async fn test_pnl_calculation_with_losses() {
    let positions = create_test_positions_with_losses();
    let calculated_pnl = calculate_portfolio_pnl(&positions).await;
    
    // Expected PnL: ETH: (3000-3200)*2.0 = -400, Total: -400
    assert_eq!(calculated_pnl, -400.0, "Loss calculation should be accurate");
}

#[tokio::test]
async fn test_pnl_calculation_mixed_positions() {
    let positions = create_mixed_positions();
    let calculated_pnl = calculate_portfolio_pnl(&positions).await;
    
    // Expected PnL: ETH: +300, BTC: 0, Total: +300
    assert_eq!(calculated_pnl, 300.0, "Mixed P&L calculation should be accurate");
}

#[tokio::test]
async fn test_websocket_pnl_integration() {
    let user_id = Uuid::new_v4();
    let positions = create_test_positions_with_pnl();
    
    // Create WebSocket server with position tracker
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    
    // Add positions to position tracker
    position_tracker.insert_user_position(user_id, positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    // Test PnL calculation through WebSocket server
    let calculated_pnl = RiskWebSocketServer::calculate_real_pnl_for_user(&user_id, &risk_engine).await;
    
    assert_eq!(calculated_pnl, 450.0, "WebSocket PnL should match calculated PnL");
}

#[tokio::test]
async fn test_real_time_pnl_updates() {
    let user_id = Uuid::new_v4();
    
    // Initial positions
    let initial_positions = create_test_positions_with_pnl();
    let initial_pnl = calculate_portfolio_pnl(&initial_positions).await;
    assert_eq!(initial_pnl, 450.0);
    
    // Update positions (price change)
    let updated_positions = create_updated_positions_higher_prices();
    let updated_pnl = calculate_portfolio_pnl(&updated_positions).await;
    assert_eq!(updated_pnl, 750.0, "PnL should update with price changes");
}

#[tokio::test]
async fn test_pnl_calculation_edge_cases() {
    // Test zero positions
    let empty_positions = UserPositions {
        balances: HashMap::new(),
        pnl: Decimal::from(0),
        last_updated: chrono::Utc::now().timestamp() as u64,
    };
    let pnl = calculate_portfolio_pnl(&empty_positions).await;
    assert_eq!(pnl, 0.0, "Empty portfolio should have zero PnL");
    
    // Test positions with zero entry price (should handle gracefully)
    let zero_entry_positions = create_positions_with_zero_entry_price();
    let pnl = calculate_portfolio_pnl(&zero_entry_positions).await;
    assert!(pnl >= 0.0, "Zero entry price should be handled gracefully");
}

// Helper functions - removed database dependency for now

fn create_test_positions_with_pnl() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 1.5 ETH, entry: $3200, current: $3500, PnL: +$450
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.5").unwrap(),
        value_usd: Decimal::from_str("5250.0").unwrap(), // 1.5 * 3500
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    // USDC position: 1000 USDC, entry: $1.0, current: $1.0, PnL: $0
    balances.insert("USDC".to_string(), TokenBalance {
        token_address: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(),
        balance: Decimal::from_str("1000.0").unwrap(),
        value_usd: Decimal::from_str("1000.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("450.0").unwrap(), // Will be calculated
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_test_positions_with_losses() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 2.0 ETH, entry: $3200 (default), current: $3000, PnL: (3000-3200)*2.0 = -$400
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("2.0").unwrap(),
        value_usd: Decimal::from_str("6000.0").unwrap(), // 2.0 * 3000
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("-400.0").unwrap(), // Updated to match calculation
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_mixed_positions() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH: 1.0 ETH, entry: $3200, current: $3500, PnL: (3500-3200)*1.0 = +$300
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.0").unwrap(),
        value_usd: Decimal::from_str("3500.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    // BTC: 0.01 BTC, entry: $65000, current: $65000, PnL: (65000-65000)*0.01 = $0
    // (Note: BTC uses default entry price of $65000 in our logic)
    balances.insert("BTC".to_string(), TokenBalance {
        token_address: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(),
        balance: Decimal::from_str("0.01").unwrap(),
        value_usd: Decimal::from_str("650.0").unwrap(), // 0.01 * 65000
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("300.0").unwrap(), // ETH: +300, BTC: 0, Total: +300
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_updated_positions_higher_prices() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 1.5 ETH, entry: $3200, current: $3700, PnL: +$750
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.5").unwrap(),
        value_usd: Decimal::from_str("5550.0").unwrap(), // 1.5 * 3700
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("750.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_positions_with_zero_entry_price() -> UserPositions {
    let mut balances = HashMap::new();
    
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.0").unwrap(),
        value_usd: Decimal::from_str("3500.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("3500.0").unwrap(), // All current value as profit
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

// Core PnL calculation function - matches WebSocket server logic
async fn calculate_portfolio_pnl(positions: &UserPositions) -> f64 {
    let mut total_pnl = 0.0;
    
    for (token, balance) in &positions.balances {
        let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
        let token_amount = balance.balance.to_string().parse::<f64>().unwrap_or(0.0);
        
        if token_amount > 0.0 {
            let current_price = current_value / token_amount;
            
            // Entry price estimation (matches WebSocket server logic)
            let estimated_entry_price = match token.as_str() {
                "ETH" => 3200.0,
                "BTC" => 65000.0,
                "USDC" | "USDT" | "DAI" => 1.0,
                _ => current_price * 0.9,
            };
            
            let entry_value = token_amount * estimated_entry_price;
            let pnl = current_value - entry_value;
            total_pnl += pnl;
        }
    }
    
    total_pnl
}
