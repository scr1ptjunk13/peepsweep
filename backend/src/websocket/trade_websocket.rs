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
use crate::analytics::trade_history::{TradeHistoryManager, TradeRecord, TradeStatus};
use crate::risk_management::types::{RiskError, UserId};
use futures_util::{StreamExt, SinkExt};

/// Trade WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TradeMessage {
    Subscribe { user_id: String },
    Unsubscribe { user_id: String },
    TradeUpdate { user_id: String, trade: TradeRecord },
    TradeCreated { user_id: String, trade: TradeRecord },
    TradeExecuted { user_id: String, trade: TradeRecord },
    TradeFailed { user_id: String, trade: TradeRecord },
    Error { message: String },
    Ping,
    Pong,
}

/// Trade WebSocket state
#[derive(Clone)]
pub struct TradeWebSocketState {
    pub trade_manager: Arc<TradeHistoryManager>,
    pub broadcaster: broadcast::Sender<TradeMessage>,
    pub subscribers: Arc<RwLock<HashMap<UserId, Vec<broadcast::Sender<TradeMessage>>>>>,
}

impl TradeWebSocketState {
    pub fn new(trade_manager: Arc<TradeHistoryManager>) -> Self {
        let (broadcaster, _) = broadcast::channel(1000);
        Self {
            trade_manager,
            broadcaster,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Broadcast trade update to subscribers
    pub async fn broadcast_trade_update(&self, user_id: UserId, trade: TradeRecord) -> Result<(), RiskError> {
        let message = match trade.status {
            TradeStatus::Pending => TradeMessage::TradeCreated {
                user_id: user_id.to_string(),
                trade,
            },
            TradeStatus::Executed => TradeMessage::TradeExecuted {
                user_id: user_id.to_string(),
                trade,
            },
            TradeStatus::Failed => TradeMessage::TradeFailed {
                user_id: user_id.to_string(),
                trade,
            },
            _ => TradeMessage::TradeUpdate {
                user_id: user_id.to_string(),
                trade,
            },
        };

        if let Err(e) = self.broadcaster.send(message) {
            warn!("Failed to broadcast trade update: {}", e);
        }

        debug!("Broadcasted trade update for user {}", user_id);
        Ok(())
    }

    /// Subscribe user to trade updates
    pub async fn subscribe_user(&self, user_id: UserId, sender: broadcast::Sender<TradeMessage>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.entry(user_id).or_insert_with(Vec::new).push(sender);
        info!("User {} subscribed to trade updates", user_id);
    }

    /// Unsubscribe user from trade updates
    pub async fn unsubscribe_user(&self, user_id: UserId) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.remove(&user_id);
        info!("User {} unsubscribed from trade updates", user_id);
    }

    /// Get recent trades for a user (for initial connection)
    pub async fn get_recent_trades(&self, user_id: UserId, limit: u32) -> Result<Vec<TradeRecord>, RiskError> {
        use crate::analytics::trade_history::{TradeQuery, TradeSortBy};
        
        let query = TradeQuery {
            filter: None,
            sort_by: Some(TradeSortBy::Timestamp),
            sort_desc: Some(true),
            page: Some(0),
            page_size: Some(limit),
        };

        self.trade_manager.query_trades(&user_id, &query).await
    }
}

/// Handle Trade WebSocket upgrade
pub async fn handle_trade_websocket(
    ws: WebSocketUpgrade,
    State(state): State<TradeWebSocketState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_trade_socket(socket, state))
}

/// Handle individual Trade WebSocket connection
async fn handle_trade_socket(socket: WebSocket, state: TradeWebSocketState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel(100);
    
    // Task to send messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json_msg = match serde_json::to_string(&msg) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to serialize trade message: {}", e);
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
                    match serde_json::from_str::<TradeMessage>(&text) {
                        Ok(TradeMessage::Subscribe { user_id }) => {
                            match Uuid::parse_str(&user_id) {
                                Ok(uuid) => {
                                    current_user_id = Some(uuid);
                                    state.subscribe_user(uuid, tx.clone()).await;
                                    
                                    // Send recent trades on subscription
                                    match state.get_recent_trades(uuid, 10).await {
                                        Ok(trades) => {
                                            for trade in trades {
                                                let update_msg = TradeMessage::TradeUpdate {
                                                    user_id: uuid.to_string(),
                                                    trade,
                                                };
                                                let _ = tx.send(update_msg);
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to get recent trades for user {}: {}", uuid, e);
                                        }
                                    }
                                }
                                Err(_) => {
                                    let error_msg = TradeMessage::Error {
                                        message: "Invalid user ID format".to_string(),
                                    };
                                    let _ = tx.send(error_msg);
                                }
                            }
                        }
                        Ok(TradeMessage::Unsubscribe { user_id }) => {
                            if let Ok(uuid) = Uuid::parse_str(&user_id) {
                                state.unsubscribe_user(uuid).await;
                                current_user_id = None;
                            }
                        }
                        Ok(TradeMessage::Ping) => {
                            let pong_msg = TradeMessage::Pong;
                            let _ = tx.send(pong_msg);
                        }
                        Ok(_) => {
                            // Ignore other message types
                        }
                        Err(e) => {
                            warn!("Failed to parse trade WebSocket message: {}", e);
                            let error_msg = TradeMessage::Error {
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
    use crate::analytics::trade_history::{MockTradeDataStore, MockTradeSearchIndex, MockTradeDataValidator};

    #[tokio::test]
    async fn test_trade_websocket_state() {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        
        let trade_manager = Arc::new(TradeHistoryManager::new(data_store, search_index, validator));
        let state = TradeWebSocketState::new(trade_manager);
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
    async fn test_trade_message_serialization() {
        use crate::analytics::trade_history::{TradeType, TradeStatus};
        use rust_decimal::Decimal;
        use std::collections::HashMap;

        let trade = TradeRecord {
            trade_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            trade_type: TradeType::Swap,
            status: TradeStatus::Executed,
            timestamp: Utc::now(),
            execution_timestamp: Some(Utc::now()),
            input_token: "USDC".to_string(),
            output_token: "ETH".to_string(),
            input_amount: Decimal::from(1000),
            output_amount: Some(Decimal::from_f64_retain(0.5).unwrap()),
            expected_output: Decimal::from_f64_retain(0.5).unwrap(),
            dex_used: "Uniswap".to_string(),
            route_path: vec!["USDC".to_string(), "ETH".to_string()],
            slippage_tolerance: Decimal::from_f64_retain(0.5).unwrap(),
            actual_slippage: Some(Decimal::from_f64_retain(0.2).unwrap()),
            gas_used: Some(150000),
            gas_price: Some(Decimal::from(20)),
            gas_cost_usd: Some(Decimal::from(15)),
            protocol_fees: Decimal::from(3),
            network_fees: Decimal::from(15),
            price_impact: Some(Decimal::from_f64_retain(0.1).unwrap()),
            execution_time_ms: Some(2500),
            pnl_usd: Some(Decimal::from(50)),
            transaction_hash: Some("0x123".to_string()),
            block_number: Some(18000000),
            nonce: Some(42),
            metadata: HashMap::new(),
            error_message: None,
        };

        let message = TradeMessage::TradeExecuted {
            user_id: Uuid::new_v4().to_string(),
            trade,
        };

        // Test serialization
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("TradeExecuted"));
        assert!(json.contains("input_token"));

        // Test deserialization
        let deserialized: TradeMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            TradeMessage::TradeExecuted { trade, .. } => {
                assert_eq!(trade.input_token, "USDC");
                assert_eq!(trade.status, TradeStatus::Executed);
            }
            _ => panic!("Wrong message type"),
        }
    }
}
