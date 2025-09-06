use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use bralaladex_backend::risk_management::{
    websocket_server::{RiskWebSocketServer, WebSocketMessage, AlertMessage, RiskMetricsUpdate},
    risk_engine::{RiskProcessingEngine, RiskEngineConfig, RiskMetrics},
    position_tracker::PositionTracker,
    types::{UserPositions, TokenBalance},
};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::collections::HashMap;

#[tokio::test]
async fn test_automatic_threshold_breach_detection() {
    // Setup risk engine with position tracker
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create user with high-risk position
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from(100),
        value_usd: Decimal::from(320000), // 100 ETH * $3200
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    // Create WebSocket server with threshold monitoring
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test automatic threshold breach detection
    let breaches = RiskWebSocketServer::check_risk_thresholds(&user_id.to_string(), &risk_engine).await;
    
    // Should detect high exposure threshold breach
    assert!(!breaches.is_empty(), "Should detect threshold breaches for high-risk position");
    
    let exposure_breach = breaches.iter().find(|b| b.alert_type == "exposure");
    assert!(exposure_breach.is_some(), "Should detect exposure threshold breach");
    
    let breach = exposure_breach.unwrap();
    assert_eq!(breach.severity, "high");
    assert!(breach.current_value > breach.threshold);
    assert!(breach.message.contains("exposure"));
}

#[tokio::test]
async fn test_var_threshold_breach_detection() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create user with volatile position
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from(50),
        value_usd: Decimal::from(160000), // 50 ETH * $3200
        last_updated: 0,
    });
    balances.insert("BTC".to_string(), TokenBalance {
        token_address: "BTC".to_string(),
        balance: Decimal::from(2),
        value_usd: Decimal::from(130000), // 2 BTC * $65000
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test VaR threshold detection
    let breaches = RiskWebSocketServer::check_risk_thresholds(&user_id.to_string(), &risk_engine).await;
    
    let var_breach = breaches.iter().find(|b| b.alert_type == "var_95");
    if let Some(breach) = var_breach {
        assert!(breach.current_value > breach.threshold);
        assert!(breach.message.contains("VaR"));
    }
}

#[tokio::test]
async fn test_pnl_loss_threshold_breach() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create user with losing position (simulate by setting low balances)
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from_str("0.1").unwrap(),
        value_usd: Decimal::from(320), // 0.1 ETH * $3200
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(-5000), // Simulate loss
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test PnL loss threshold detection
    let breaches = RiskWebSocketServer::check_risk_thresholds(&user_id.to_string(), &risk_engine).await;
    
    let pnl_breach = breaches.iter().find(|b| b.alert_type == "pnl_loss");
    if let Some(breach) = pnl_breach {
        assert!(breach.current_value < 0.0); // Negative PnL
        assert!(breach.message.contains("loss"));
        assert_eq!(breach.severity, "high");
    }
}

#[tokio::test]
async fn test_no_threshold_breach_for_safe_position() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create user with safe position
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("USDC".to_string(), TokenBalance {
        token_address: "USDC".to_string(),
        balance: Decimal::from(1000),
        value_usd: Decimal::from(1000), // 1000 USDC * $1
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test no threshold breaches for safe position
    let breaches = RiskWebSocketServer::check_risk_thresholds(&user_id.to_string(), &risk_engine).await;
    
    // Should have no breaches for safe position
    assert!(breaches.is_empty(), "Safe position should not trigger threshold breaches");
}

#[tokio::test]
async fn test_automatic_alert_broadcasting() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create broadcast channel
    let (broadcast_tx, mut broadcast_rx) = broadcast::channel(100);
    
    // Create user with high-risk position
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from(100),
        value_usd: Decimal::from(320000), // 100 ETH * $3200
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test automatic alert broadcasting
    RiskWebSocketServer::send_threshold_alerts(&user_id.to_string(), &risk_engine, &broadcast_tx).await;
    
    // Should receive alert message
    let received_message = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        broadcast_rx.recv()
    ).await;
    
    assert!(received_message.is_ok(), "Should receive alert message");
    
    if let Ok(Ok(WebSocketMessage::Alert(alert))) = received_message {
        assert_eq!(alert.user_id, user_id.to_string());
        assert!(alert.current_value > alert.threshold);
        assert!(!alert.message.is_empty());
    } else {
        panic!("Expected alert message");
    }
}

#[tokio::test]
async fn test_multiple_threshold_types() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create user with complex position triggering multiple thresholds
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from(80),
        value_usd: Decimal::from(256000), // 80 ETH * $3200
        last_updated: 0,
    });
    balances.insert("BTC".to_string(), TokenBalance {
        token_address: "BTC".to_string(),
        balance: Decimal::from(3),
        value_usd: Decimal::from(195000), // 3 BTC * $65000
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Test multiple threshold types
    let breaches = RiskWebSocketServer::check_risk_thresholds(&user_id.to_string(), &risk_engine).await;
    
    // Should detect multiple breach types
    let breach_types: Vec<&str> = breaches.iter().map(|b| b.alert_type.as_str()).collect();
    
    // Should have at least exposure threshold breach
    assert!(breach_types.contains(&"exposure"), "Should detect exposure breach");
    
    // Verify each breach has proper structure
    for breach in &breaches {
        assert!(!breach.user_id.is_empty());
        assert!(!breach.alert_type.is_empty());
        assert!(!breach.message.is_empty());
        assert!(breach.current_value > 0.0);
        assert!(breach.threshold > 0.0);
        assert!(["low", "medium", "high"].contains(&breach.severity.as_str()));
    }
}

#[tokio::test]
async fn test_real_time_threshold_monitoring() {
    let config = RiskEngineConfig::default();
    let position_tracker = Arc::new(PositionTracker::new());
    let risk_engine = Arc::new(RiskProcessingEngine::new(config, position_tracker.clone()));
    
    // Create broadcast channel
    let (broadcast_tx, mut broadcast_rx) = broadcast::channel(100);
    
    // Start with safe position
    let user_id = Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert("USDC".to_string(), TokenBalance {
        token_address: "USDC".to_string(),
        balance: Decimal::from(1000),
        value_usd: Decimal::from(1000), // 1000 USDC * $1
        last_updated: 0,
    });
    let position = UserPositions {
        balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, position);
    
    let server = RiskWebSocketServer::new(risk_engine.clone());
    
    // Initial check - should be no alerts
    RiskWebSocketServer::send_threshold_alerts(&user_id.to_string(), &risk_engine, &broadcast_tx).await;
    
    // Should not receive any alerts for safe position
    let no_alert = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        broadcast_rx.recv()
    ).await;
    assert!(no_alert.is_err(), "Should not receive alerts for safe position");
    
    // Update to risky position
    let mut risky_balances = HashMap::new();
    risky_balances.insert("ETH".to_string(), TokenBalance {
        token_address: "ETH".to_string(),
        balance: Decimal::from(100),
        value_usd: Decimal::from(320000), // 100 ETH * $3200
        last_updated: 0,
    });
    let risky_position = UserPositions {
        balances: risky_balances,
        pnl: Decimal::from(0),
        last_updated: 0,
    };
    position_tracker.insert_user_position(user_id, risky_position);
    
    // Check again - should now trigger alerts
    RiskWebSocketServer::send_threshold_alerts(&user_id.to_string(), &risk_engine, &broadcast_tx).await;
    
    // Should receive alert for risky position
    let alert_received = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        broadcast_rx.recv()
    ).await;
    
    assert!(alert_received.is_ok(), "Should receive alert for risky position");
    
    if let Ok(Ok(WebSocketMessage::Alert(alert))) = alert_received {
        assert_eq!(alert.user_id, user_id.to_string());
        assert!(alert.current_value > alert.threshold);
    }
}
