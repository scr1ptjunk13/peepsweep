use bralaladex_backend::risk_management::{
    types::{UserPositions, TokenBalance},
    websocket_server::{RiskWebSocketServer, WebSocketMessage, PositionUpdate, PositionData},
    risk_engine::RiskProcessingEngine,
    position_tracker::PositionTracker,
};
use std::collections::HashMap;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;
use std::sync::Arc;

#[tokio::test]
async fn test_real_position_updates_from_tracker() {
    let user_id = Uuid::new_v4();
    let positions = create_test_user_positions();
    
    // Create position tracker with real data
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    position_tracker.insert_user_position(user_id, positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    // Get real position updates (not mock data)
    let position_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    
    // Verify we get real data, not mock data
    assert!(!position_updates.is_empty(), "Should have position updates");
    assert_eq!(position_updates.len(), 2, "Should have 2 positions (ETH + USDC)");
    
    // Check ETH position
    let eth_position = position_updates.iter().find(|p| p.token == "ETH").unwrap();
    assert_eq!(eth_position.amount, 1.5, "ETH amount should match");
    assert_eq!(eth_position.current_price, 3500.0, "ETH current price should be calculated");
    assert_eq!(eth_position.pnl, 450.0, "ETH PnL should be calculated correctly");
    
    // Check USDC position
    let usdc_position = position_updates.iter().find(|p| p.token == "USDC").unwrap();
    assert_eq!(usdc_position.amount, 1000.0, "USDC amount should match");
    assert_eq!(usdc_position.current_price, 1.0, "USDC price should be $1");
    assert_eq!(usdc_position.pnl, 0.0, "USDC PnL should be 0 (stablecoin)");
}

#[tokio::test]
async fn test_position_updates_websocket_integration() {
    let user_id = Uuid::new_v4();
    let positions = create_test_user_positions();
    
    // Create WebSocket server with real position tracker
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    position_tracker.insert_user_position(user_id, positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    // Create message channel for testing
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    
    // Send real position update
    RiskWebSocketServer::send_real_position_update(&user_id.to_string(), &risk_engine, &sender).await;
    
    // Verify WebSocket message
    let message = receiver.recv().await.unwrap();
    match message {
        WebSocketMessage::Position(update) => {
            assert_eq!(update.user_id, user_id.to_string());
            assert_eq!(update.positions.len(), 2);
            assert_eq!(update.total_pnl, 450.0); // ETH: +450, USDC: 0
            
            // Verify individual positions
            let eth_pos = update.positions.iter().find(|p| p.token == "ETH").unwrap();
            assert_eq!(eth_pos.pnl, 450.0);
            assert_eq!(eth_pos.pnl_percentage, 9.375); // 450/4800 * 100
        }
        _ => panic!("Expected PositionUpdate message"),
    }
}

#[tokio::test]
async fn test_position_updates_with_losses() {
    let user_id = Uuid::new_v4();
    let positions = create_positions_with_losses();
    
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    position_tracker.insert_user_position(user_id, positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    let position_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    
    assert_eq!(position_updates.len(), 1, "Should have 1 position");
    
    let eth_position = &position_updates[0];
    assert_eq!(eth_position.token, "ETH");
    assert_eq!(eth_position.pnl, -400.0, "Should show loss correctly");
    assert!(eth_position.pnl_percentage < 0.0, "PnL percentage should be negative");
}

#[tokio::test]
async fn test_position_updates_multiple_tokens() {
    let user_id = Uuid::new_v4();
    let positions = create_mixed_token_positions();
    
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    position_tracker.insert_user_position(user_id, positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    let position_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    
    assert_eq!(position_updates.len(), 3, "Should have 3 positions (ETH, BTC, USDC)");
    
    // Verify each token is present
    let tokens: Vec<&str> = position_updates.iter().map(|p| p.token.as_str()).collect();
    assert!(tokens.contains(&"ETH"));
    assert!(tokens.contains(&"BTC"));
    assert!(tokens.contains(&"USDC"));
    
    // Verify total PnL calculation
    let total_pnl: f64 = position_updates.iter().map(|p| p.pnl).sum();
    assert_eq!(total_pnl, 300.0, "Total PnL should be sum of individual positions");
}

#[tokio::test]
async fn test_position_updates_empty_portfolio() {
    let user_id = Uuid::new_v4();
    
    // Empty position tracker
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker));
    
    let position_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    
    assert!(position_updates.is_empty(), "Empty portfolio should return no positions");
}

#[tokio::test]
async fn test_position_updates_real_time_changes() {
    let user_id = Uuid::new_v4();
    let initial_positions = create_test_user_positions();
    
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    position_tracker.insert_user_position(user_id, initial_positions.clone());
    
    let risk_engine = Arc::new(RiskProcessingEngine::new(Default::default(), position_tracker.clone()));
    
    // Get initial position updates
    let initial_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    let initial_pnl: f64 = initial_updates.iter().map(|p| p.pnl).sum();
    assert_eq!(initial_pnl, 450.0);
    
    // Update positions with new prices
    let updated_positions = create_updated_positions_higher_prices();
    position_tracker.insert_user_position(user_id, updated_positions);
    
    // Get updated position updates
    let updated_updates = RiskWebSocketServer::get_real_position_updates(&user_id.to_string(), &risk_engine).await;
    let updated_pnl: f64 = updated_updates.iter().map(|p| p.pnl).sum();
    assert_eq!(updated_pnl, 750.0, "PnL should update with price changes");
}

// Helper functions
fn create_test_user_positions() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 1.5 ETH at $3500, entry at $3200, PnL: +$450
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.5").unwrap(),
        value_usd: Decimal::from_str("5250.0").unwrap(), // 1.5 * 3500
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    // USDC position: 1000 USDC at $1.0, entry at $1.0, PnL: $0
    balances.insert("USDC".to_string(), TokenBalance {
        token_address: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(),
        balance: Decimal::from_str("1000.0").unwrap(),
        value_usd: Decimal::from_str("1000.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("450.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_positions_with_losses() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 2.0 ETH at $3000, entry at $3200, PnL: -$400
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("2.0").unwrap(),
        value_usd: Decimal::from_str("6000.0").unwrap(), // 2.0 * 3000
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("-400.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_mixed_token_positions() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH: 1.0 ETH at $3500, entry at $3200, PnL: +$300
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        balance: Decimal::from_str("1.0").unwrap(),
        value_usd: Decimal::from_str("3500.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    // BTC: 0.01 BTC at $65000, entry at $65000, PnL: $0
    balances.insert("BTC".to_string(), TokenBalance {
        token_address: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(),
        balance: Decimal::from_str("0.01").unwrap(),
        value_usd: Decimal::from_str("650.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    // USDC: 1000 USDC at $1.0, entry at $1.0, PnL: $0
    balances.insert("USDC".to_string(), TokenBalance {
        token_address: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(),
        balance: Decimal::from_str("1000.0").unwrap(),
        value_usd: Decimal::from_str("1000.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    });
    
    UserPositions {
        balances,
        pnl: Decimal::from_str("300.0").unwrap(), // ETH: +300, BTC: 0, USDC: 0
        last_updated: chrono::Utc::now().timestamp() as u64,
    }
}

fn create_updated_positions_higher_prices() -> UserPositions {
    let mut balances = HashMap::new();
    
    // ETH position: 1.5 ETH at $3700, entry at $3200, PnL: +$750
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
