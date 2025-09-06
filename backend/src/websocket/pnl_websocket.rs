use axum::extract::ws::{WebSocket, Message};
use axum::extract::{WebSocketUpgrade, State};
use axum::response::Response;
use tokio::sync::{broadcast, RwLock};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;
use tracing::{debug, error, info, warn};
use std::collections::HashMap;
use crate::analytics::pnl_calculator::{PnLCalculator, PnLResult};
use crate::analytics::data_models::PositionPnL as DataPositionPnL;
use crate::risk_management::types::{RiskError, UserId};
use futures_util::{StreamExt, SinkExt};
use tokio::time::{interval, Duration};

/// P&L WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PnLMessage {
    Subscribe { user_id: String },
    Unsubscribe { user_id: String },
    PnLUpdate { user_id: String, data: PnLUpdateData },
    Error { message: String },
    Ping,
    Pong,
}

/// P&L update data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLUpdateData {
    pub timestamp: DateTime<Utc>,
    pub total_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub portfolio_value: Decimal,
    pub daily_change: Decimal,
    pub daily_change_percent: Decimal,
    pub position_count: u32,
    pub positions: Vec<PositionPnL>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionPnL {
    pub token: String,
    pub amount: Decimal,
    pub value_usd: Decimal,
    pub pnl: Decimal,
    pub pnl_percent: Decimal,
}

/// P&L WebSocket state
#[derive(Clone)]
pub struct PnLWebSocketState {
    pub pnl_calculator: Arc<PnLCalculator>,
    pub broadcaster: broadcast::Sender<PnLMessage>,
    pub subscribers: Arc<RwLock<HashMap<UserId, Vec<broadcast::Sender<PnLMessage>>>>>,
}

impl PnLWebSocketState {
    pub fn new(pnl_calculator: Arc<PnLCalculator>) -> Self {
        let (broadcaster, _) = broadcast::channel(1000);
        Self {
            pnl_calculator,
            broadcaster,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start background task for periodic P&L updates
    pub async fn start_periodic_updates(&self) {
        let state = self.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Update every 30 seconds
            
            loop {
                interval.tick().await;
                
                let subscribers = state.subscribers.read().await;
                for (user_id, _) in subscribers.iter() {
                    if let Err(e) = state.send_pnl_update(*user_id).await {
                        warn!("Failed to send P&L update for user {}: {}", user_id, e);
                    }
                }
            }
        });
    }

    /// Send P&L update for a specific user
    pub async fn send_pnl_update(&self, user_id: UserId) -> Result<(), RiskError> {
        match self.pnl_calculator.calculate_current_pnl(&user_id).await {
            Ok(pnl_result) => {
                let update_data = PnLUpdateData {
                    timestamp: Utc::now(),
                    total_pnl: pnl_result.total_pnl,
                    unrealized_pnl: pnl_result.unrealized_pnl,
                    realized_pnl: pnl_result.realized_pnl,
                    portfolio_value: pnl_result.portfolio_value,
                    daily_change: pnl_result.daily_change,
                    daily_change_percent: pnl_result.daily_change_percent,
                    position_count: pnl_result.positions.len() as u32,
                    positions: pnl_result.positions.into_iter().map(|pos| PositionPnL {
                        token: pos.token,
                        amount: pos.amount,
                        value_usd: pos.value_usd,
                        pnl: pos.pnl,
                        pnl_percent: pos.pnl_percent,
                    }).collect(),
                };

                let message = PnLMessage::PnLUpdate {
                    user_id: user_id.to_string(),
                    data: update_data,
                };

                // Broadcast to all subscribers
                if let Err(e) = self.broadcaster.send(message) {
                    warn!("Failed to broadcast P&L update: {}", e);
                }

                debug!("Sent P&L update for user {}", user_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to calculate P&L for user {}: {}", user_id, e);
                Err(e)
            }
        }
    }

    /// Subscribe user to P&L updates
    pub async fn subscribe_user(&self, user_id: UserId, sender: broadcast::Sender<PnLMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.entry(user_id).or_insert_with(Vec::new).push(sender);
        info!("User {} subscribed to P&L updates", user_id);
    }

    /// Unsubscribe user from P&L updates
    pub async fn unsubscribe_user(&self, user_id: UserId) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(&user_id);
        info!("User {} unsubscribed from P&L updates", user_id);
    }
}

/// Handle P&L WebSocket upgrade
pub async fn handle_pnl_websocket(
    ws: WebSocketUpgrade,
    State(state): State<PnLWebSocketState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_pnl_socket(socket, state))
}

/// Handle individual P&L WebSocket connection
async fn handle_pnl_socket(socket: WebSocket, state: PnLWebSocketState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel(100);
    
    // Task to send messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json_msg = match serde_json::to_string(&msg) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to serialize P&L message: {}", e);
                    continue;
                }
            };
            
            if sender.send(Message::Text(json_msg)).await.is_err() {
                break;
            }
        }
    });

    // Task to receive messages from client
    let receive_task = tokio::spawn(async move {
        let mut current_user_id: Option<UserId> = None;
        
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    match serde_json::from_str::<PnLMessage>(&text) {
                        Ok(PnLMessage::Subscribe { user_id }) => {
                            match Uuid::parse_str(&user_id) {
                                Ok(uuid) => {
                                    current_user_id = Some(uuid);
                                    state.subscribe_user(uuid, tx.clone()).await;
                                    
                                    // Send immediate P&L update
                                    if let Err(e) = state.send_pnl_update(uuid).await {
                                        warn!("Failed to send initial P&L update: {}", e);
                                    }
                                }
                                Err(_) => {
                                    let error_msg = PnLMessage::Error {
                                        message: "Invalid user ID format".to_string(),
                                    };
                                    let _ = tx.send(error_msg);
                                }
                            }
                        }
                        Ok(PnLMessage::Unsubscribe { user_id }) => {
                            if let Ok(uuid) = Uuid::parse_str(&user_id) {
                                state.unsubscribe_user(uuid).await;
                                current_user_id = None;
                            }
                        }
                        Ok(PnLMessage::Ping) => {
                            let pong_msg = PnLMessage::Pong;
                            let _ = tx.send(pong_msg);
                        }
                        Ok(_) => {
                            // Ignore other message types
                        }
                        Err(e) => {
                            warn!("Failed to parse P&L WebSocket message: {}", e);
                            let error_msg = PnLMessage::Error {
                                message: "Invalid message format".to_string(),
                            };
                            let _ = tx.send(error_msg);
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    if let Some(user_id) = current_user_id {
                        state.unsubscribe_user(user_id).await;
                    }
                    break;
                }
                Ok(_) => {
                    // Ignore binary and other message types
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
        
        // Cleanup on disconnect
        if let Some(user_id) = current_user_id {
            state.unsubscribe_user(user_id).await;
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = receive_task => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::pnl_calculator::{MockPriceOracle, MockPositionTracker, MockTradeHistory};

    #[tokio::test]
    async fn test_pnl_websocket_state() {
        let price_oracle = Arc::new(MockPriceOracle::new());
        let position_tracker = Arc::new(MockPositionTracker::new());
        let trade_history = Arc::new(MockTradeHistory::new());
        
        let pnl_calculator = Arc::new(PnLCalculator::new(
            price_oracle,
            position_tracker,
            trade_history,
        ));
        
        let state = PnLWebSocketState::new(pnl_calculator);
        let user_id = Uuid::new_v4();
        
        // Test subscription
        let (tx, _rx) = broadcast::channel(10);
        state.subscribe_user(user_id, tx).await;
        
        {
            let subscribers = state.subscribers.read().await;
            assert!(subscribers.contains_key(&user_id));
        }
        
        // Test unsubscription
        state.unsubscribe_user(user_id).await;
        
        {
            let subscribers = state.subscribers.read().await;
            assert!(!subscribers.contains_key(&user_id));
        }
    }

    #[tokio::test]
    async fn test_pnl_update_data_serialization() {
        let update_data = PnLUpdateData {
            timestamp: Utc::now(),
            total_pnl: Decimal::from(1000),
            unrealized_pnl: Decimal::from(800),
            realized_pnl: Decimal::from(200),
            portfolio_value: Decimal::from(10000),
            daily_change: Decimal::from(50),
            daily_change_percent: Decimal::from_f64_retain(0.5).unwrap(),
            position_count: 3,
            positions: vec![
                PositionPnL {
                    token: "ETH".to_string(),
                    amount: Decimal::from_f64_retain(2.5).unwrap(),
                    value_usd: Decimal::from(5000),
                    pnl: Decimal::from(500),
                    pnl_percent: Decimal::from(10),
                }
            ],
        };

        let message = PnLMessage::PnLUpdate {
            user_id: Uuid::new_v4().to_string(),
            data: update_data,
        };

        // Test serialization
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("PnLUpdate"));
        assert!(json.contains("total_pnl"));

        // Test deserialization
        let deserialized: PnLMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            PnLMessage::PnLUpdate { data, .. } => {
                assert_eq!(data.total_pnl, Decimal::from(1000));
                assert_eq!(data.position_count, 3);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
