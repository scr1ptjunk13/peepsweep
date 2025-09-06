use crate::risk_management::performance_tracker::{PortfolioPerformanceTracker, PerformanceMetrics};
use crate::risk_management::position_tracker::PositionTracker;
use crate::risk_management::redis_cache::RiskCache;
use crate::risk_management::types::UserId;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use uuid::Uuid;
use futures_util::{SinkExt, StreamExt};

/// WebSocket state for performance streaming
#[derive(Clone)]
pub struct PerformanceWebSocketState {
    pub performance_tracker: Arc<PortfolioPerformanceTracker>,
    pub broadcast_tx: broadcast::Sender<PerformanceUpdate>,
    pub connections: Arc<RwLock<HashMap<String, UserId>>>,
}

impl PerformanceWebSocketState {
    pub fn new(performance_tracker: Arc<PortfolioPerformanceTracker>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        let connections = Arc::new(RwLock::new(HashMap::new()));
        
        Self {
            performance_tracker,
            broadcast_tx,
            connections,
        }
    }

    pub async fn new_async(
        position_tracker: Arc<PositionTracker>,
        redis_cache: Arc<tokio::sync::RwLock<RiskCache>>,
    ) -> Result<Self, crate::risk_management::types::RiskError> {
        let performance_tracker = Arc::new(
            PortfolioPerformanceTracker::new(position_tracker, redis_cache).await?
        );
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Ok(Self {
            performance_tracker,
            broadcast_tx,
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start background task for periodic performance updates
    pub async fn start_performance_streaming(&self) {
        let tracker = self.performance_tracker.clone();
        let tx = self.broadcast_tx.clone();
        let connections = self.connections.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Update every 30 seconds
            
            loop {
                interval.tick().await;
                
                let active_users: Vec<UserId> = {
                    let conn_guard = connections.read().await;
                    conn_guard.values().cloned().collect()
                };

                for user_id in active_users {
                    match tracker.calculate_performance_metrics(user_id).await {
                        Ok(metrics) => {
                            let update = PerformanceUpdate {
                                user_id: user_id.to_string(),
                                metrics,
                                update_type: UpdateType::Periodic,
                                timestamp: chrono::Utc::now().timestamp() as u64,
                            };
                            
                            if let Err(e) = tx.send(update) {
                                eprintln!("Failed to broadcast performance update: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to calculate metrics for user {}: {}", user_id, e);
                        }
                    }
                }
            }
        });
    }
}

/// Performance update message for WebSocket streaming
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceUpdate {
    pub user_id: String,
    pub metrics: PerformanceMetrics,
    pub update_type: UpdateType,
    pub timestamp: u64,
}

/// Type of performance update
#[derive(Debug, Clone, Serialize)]
pub enum UpdateType {
    Periodic,
    TradeExecuted,
    PositionChanged,
    Manual,
}

/// WebSocket client message
#[derive(Debug, Deserialize)]
pub struct ClientMessage {
    pub message_type: String,
    pub user_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// WebSocket server response
#[derive(Debug, Serialize)]
pub struct ServerMessage {
    pub message_type: String,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

/// Handle WebSocket upgrade for performance streaming
pub async fn performance_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<PerformanceWebSocketState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_performance_websocket(socket, state))
}

/// Handle individual WebSocket connection
async fn handle_performance_websocket(socket: WebSocket, state: PerformanceWebSocketState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.broadcast_tx.subscribe();
    let connection_id = Uuid::new_v4().to_string();
    let mut user_id: Option<UserId> = None;

    // Spawn task to handle incoming messages from client
    let state_clone = state.clone();
    let connection_id_clone = connection_id.clone();
    tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            if let Ok(Message::Text(text)) = msg {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg.message_type.as_str() {
                        "subscribe" => {
                            if let Some(uid_str) = client_msg.user_id {
                                if let Ok(uid) = Uuid::parse_str(&uid_str) {
                                    let mut connections = state_clone.connections.write().await;
                                    connections.insert(connection_id_clone.clone(), uid);
                                    
                                    println!("User {} subscribed to performance updates", uid);
                                }
                            }
                        }
                        "unsubscribe" => {
                            let mut connections = state_clone.connections.write().await;
                            connections.remove(&connection_id_clone);
                            println!("Connection {} unsubscribed", connection_id_clone);
                        }
                        "get_current_metrics" => {
                            if let Some(uid_str) = client_msg.user_id {
                                if let Ok(uid) = Uuid::parse_str(&uid_str) {
                                    match state_clone.performance_tracker.calculate_performance_metrics(uid).await {
                                        Ok(metrics) => {
                                            let update = PerformanceUpdate {
                                                user_id: uid.to_string(),
                                                metrics,
                                                update_type: UpdateType::Manual,
                                                timestamp: chrono::Utc::now().timestamp() as u64,
                                            };
                                            
                                            if let Err(e) = state_clone.broadcast_tx.send(update) {
                                                eprintln!("Failed to send current metrics: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to get current metrics: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            println!("Unknown message type: {}", client_msg.message_type);
                        }
                    }
                }
            }
        }
    });

    // Handle outgoing messages to client
    loop {
        tokio::select! {
            update = rx.recv() => {
                match update {
                    Ok(performance_update) => {
                        // Only send updates for the subscribed user
                        if let Some(uid) = user_id {
                            if performance_update.user_id == uid.to_string() {
                                let server_msg = ServerMessage {
                                    message_type: "performance_update".to_string(),
                                    data: serde_json::to_value(&performance_update).unwrap_or_default(),
                                    timestamp: chrono::Utc::now().timestamp() as u64,
                                };
                                
                                if let Ok(msg_text) = serde_json::to_string(&server_msg) {
                                    if sender.send(Message::Text(msg_text)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        } else {
                            // Send all updates if no specific user is subscribed
                            let server_msg = ServerMessage {
                                message_type: "performance_update".to_string(),
                                data: serde_json::to_value(&performance_update).unwrap_or_default(),
                                timestamp: chrono::Utc::now().timestamp() as u64,
                            };
                            
                            if let Ok(msg_text) = serde_json::to_string(&server_msg) {
                                if sender.send(Message::Text(msg_text)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Handle lagged receiver by continuing
                        continue;
                    }
                }
            }
        }
    }

    // Clean up connection
    let mut connections = state.connections.write().await;
    connections.remove(&connection_id);
    println!("WebSocket connection {} closed", connection_id);
}

/// Trigger manual performance update for a user
pub async fn trigger_performance_update(
    state: &PerformanceWebSocketState,
    user_id: UserId,
    update_type: UpdateType,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let metrics = state.performance_tracker.calculate_performance_metrics(user_id).await?;
    
    let update = PerformanceUpdate {
        user_id: user_id.to_string(),
        metrics,
        update_type,
        timestamp: chrono::Utc::now().timestamp() as u64,
    };
    
    state.broadcast_tx.send(update)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_performance_websocket_state_creation() {
        // This test would require a mock PortfolioPerformanceTracker
        // For now, we'll just test the basic structure
        
        // Mock performance tracker would be created here
        // let tracker = Arc::new(mock_performance_tracker);
        // let state = PerformanceWebSocketState::new(tracker);
        
        // assert!(!state.broadcast_tx.is_closed());
        // assert_eq!(state.active_connections.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_client_message_deserialization() {
        let json = r#"{"message_type": "subscribe", "user_id": "123e4567-e89b-12d3-a456-426614174000"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        
        assert_eq!(msg.message_type, "subscribe");
        assert!(msg.user_id.is_some());
    }

    #[tokio::test]
    async fn test_server_message_serialization() {
        let msg = ServerMessage {
            message_type: "performance_update".to_string(),
            data: serde_json::json!({"test": "data"}),
            timestamp: 1234567890,
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("performance_update"));
        assert!(json.contains("1234567890"));
    }
}
