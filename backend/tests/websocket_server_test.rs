use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc as StdArc;

use bralaladex_backend::risk_management::{
    websocket_server::{RiskWebSocketServer, WebSocketMessage, RiskMetricsUpdate, PositionUpdate, AlertMessage},
    risk_engine::RiskProcessingEngine,
    position_tracker::PositionTracker,
    config::{RiskManagementConfig, DatabaseConfig, RedisCacheConfig},
};
use std::sync::Arc;

#[tokio::test]
async fn test_websocket_server_startup() {
    let server = create_test_websocket_server().await;
    
    // Test server can start and bind to port (with timeout)
    let result = timeout(
        Duration::from_secs(2),
        server.start("127.0.0.1:9001")
    ).await;
    
    // Server should start but we'll timeout since it runs indefinitely
    assert!(result.is_err(), "Server should timeout as expected for infinite loop");
}

#[tokio::test]
async fn test_client_connection() {
    let server = create_test_websocket_server().await;
    
    // Start server in background
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9002").await.unwrap();
    });
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test client can connect
    let connect_result = timeout(
        Duration::from_secs(5),
        connect_async("ws://127.0.0.1:9002")
    ).await;
    
    assert!(connect_result.is_ok(), "Client should connect to WebSocket server");
    let (ws_stream, _) = connect_result.unwrap().unwrap();
    drop(ws_stream);
}

#[tokio::test]
async fn test_risk_metrics_streaming() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9003").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9003").await.unwrap();
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to risk metrics
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "risk_metrics",
        "user_id": "test-user-123"
    });
    
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    
    // Should receive risk metrics update
    let msg = timeout(Duration::from_secs(2), read.next()).await;
    assert!(msg.is_ok(), "Should receive risk metrics message");
    
    let message = msg.unwrap().unwrap().unwrap();
    if let Message::Text(text) = message {
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "risk_metrics_update");
        assert!(parsed["data"]["total_exposure"].is_number());
        assert!(parsed["data"]["var_95"].is_number());
        assert!(parsed["data"]["pnl"].is_number());
    } else {
        panic!("Expected text message");
    }
}

#[tokio::test]
async fn test_position_updates_streaming() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9004").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9004").await.unwrap();
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to position updates
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "positions",
        "user_id": "test-user-456"
    });
    
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    
    // Should receive position update
    let msg = timeout(Duration::from_secs(2), read.next()).await;
    assert!(msg.is_ok(), "Should receive position update message");
    
    let message = msg.unwrap().unwrap().unwrap();
    if let Message::Text(text) = message {
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "position_update");
        assert!(parsed["data"]["positions"].is_array());
        assert!(parsed["data"]["total_pnl"].is_number());
    }
}

#[tokio::test]
async fn test_risk_threshold_alerts() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9005").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9005").await.unwrap();
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to alerts
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "alerts",
        "user_id": "test-user-789"
    });
    
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    
    // Trigger a risk threshold breach (simulate high VaR)
    let trigger_msg = json!({
        "type": "trigger_alert",
        "alert_type": "var_breach",
        "threshold": 1000.0,
        "current_value": 1500.0
    });
    
    write.send(Message::Text(trigger_msg.to_string())).await.unwrap();
    
    // Should receive alert message
    let msg = timeout(Duration::from_secs(2), read.next()).await;
    assert!(msg.is_ok(), "Should receive alert message");
    
    let message = msg.unwrap().unwrap().unwrap();
    if let Message::Text(text) = message {
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "alert");
        assert_eq!(parsed["data"]["alert_type"], "var_breach");
        assert_eq!(parsed["data"]["severity"], "high");
    }
}

#[tokio::test]
async fn test_portfolio_composition_updates() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9006").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect client
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9006").await.unwrap();
    let (mut write, mut read) = ws_stream.split();
    
    // Subscribe to portfolio updates
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "portfolio",
        "user_id": "test-user-portfolio"
    });
    
    write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    
    // Should receive portfolio composition update
    let msg = timeout(Duration::from_secs(2), read.next()).await;
    assert!(msg.is_ok(), "Should receive portfolio update message");
    
    let message = msg.unwrap().unwrap().unwrap();
    if let Message::Text(text) = message {
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], "portfolio_update");
        assert!(parsed["data"]["composition"].is_array());
        assert!(parsed["data"]["diversification_ratio"].is_number());
    }
}

#[tokio::test]
async fn test_multiple_client_connections() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9007").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect multiple clients
    let mut clients = Vec::new();
    for i in 0..5 {
        let (ws_stream, _) = connect_async("ws://127.0.0.1:9007").await.unwrap();
        let (mut write, read) = ws_stream.split();
        
        // Subscribe each client
        let subscribe_msg = json!({
            "type": "subscribe",
            "channel": "risk_metrics",
            "user_id": format!("test-user-{}", i)
        });
        
        write.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
        clients.push((write, read));
    }
    
    // All clients should receive updates
    for (_, mut read) in clients {
        let msg = timeout(Duration::from_secs(2), read.next()).await;
        assert!(msg.is_ok(), "Each client should receive updates");
    }
}

#[tokio::test]
async fn test_websocket_message_serialization() {
    // Test RiskMetricsUpdate serialization
    let risk_update = RiskMetricsUpdate {
        user_id: "test-user".to_string(),
        total_exposure: 50000.0,
        var_95: 2500.0,
        pnl: 1250.0,
        timestamp: chrono::Utc::now(),
    };
    
    let msg = WebSocketMessage::RiskMetrics(risk_update);
    let serialized = serde_json::to_string(&msg).unwrap();
    let deserialized: WebSocketMessage = serde_json::from_str(&serialized).unwrap();
    
    match deserialized {
        WebSocketMessage::RiskMetrics(update) => {
            assert_eq!(update.user_id, "test-user");
            assert_eq!(update.total_exposure, 50000.0);
        }
        _ => panic!("Wrong message type"),
    }
}

#[tokio::test]
async fn test_connection_cleanup() {
    let server = create_test_websocket_server().await;
    
    // Start server
    let _handle = tokio::spawn(async move {
        server.start("127.0.0.1:9008").await.unwrap();
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Connect and disconnect client
    {
        let (ws_stream, _) = connect_async("ws://127.0.0.1:9008").await.unwrap();
        // Connection drops here
    }
    
    // Server should handle disconnection gracefully
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // New connection should still work
    let (ws_stream, _) = connect_async("ws://127.0.0.1:9008").await.unwrap();
    drop(ws_stream);
}

// Helper function to create test WebSocket server
async fn create_test_websocket_server() -> RiskWebSocketServer {
    let config = RiskManagementConfig::default();
    let position_tracker = Arc::new(PositionTracker::new(Default::default()));
    let risk_engine = RiskProcessingEngine::new(Default::default(), position_tracker);
    
    RiskWebSocketServer::new(risk_engine)
}
