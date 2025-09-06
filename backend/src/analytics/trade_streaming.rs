use crate::analytics::trade_history::{TradeHistoryManager, TradeRecord, TradeStatus, TradeType};
use crate::risk_management::types::{RiskError, UserId};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
};
use chrono::{DateTime, Utc};
use futures_util::{sink::SinkExt, stream::StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Trade streaming server state
#[derive(Clone)]
pub struct TradeStreamingState {
    pub trade_manager: Arc<TradeHistoryManager>,
    pub broadcaster: broadcast::Sender<TradeStreamMessage>,
    pub active_connections: Arc<RwLock<HashMap<String, UserConnection>>>,
}

impl TradeStreamingState {
    pub fn new(trade_manager: Arc<TradeHistoryManager>) -> Self {
        let (broadcaster, _) = broadcast::channel(1000);
        Self {
            trade_manager,
            broadcaster,
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Broadcast trade update to all subscribed users
    pub async fn broadcast_trade_update(&self, trade: TradeRecord) {
        let message = TradeStreamMessage::TradeUpdate {
            trade,
            timestamp: Utc::now(),
        };

        if let Err(e) = self.broadcaster.send(message) {
            warn!("Failed to broadcast trade update: {}", e);
        }
    }

    /// Broadcast trade status change
    pub async fn broadcast_status_change(&self, trade_id: Uuid, user_id: UserId, old_status: TradeStatus, new_status: TradeStatus) {
        let message = TradeStreamMessage::StatusChange {
            trade_id,
            user_id,
            old_status,
            new_status,
            timestamp: Utc::now(),
        };

        if let Err(e) = self.broadcaster.send(message) {
            warn!("Failed to broadcast status change: {}", e);
        }
    }

    /// Get active connection count
    pub async fn get_connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }

    /// Get connections for specific user
    pub async fn get_user_connections(&self, user_id: &UserId) -> Vec<String> {
        self.active_connections
            .read()
            .await
            .iter()
            .filter(|(_, conn)| &conn.user_id == user_id)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

/// User connection information
#[derive(Debug, Clone)]
pub struct UserConnection {
    pub connection_id: String,
    pub user_id: UserId,
    pub connected_at: DateTime<Utc>,
    pub subscriptions: Vec<TradeSubscription>,
    pub last_ping: DateTime<Utc>,
}

/// Trade subscription filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSubscription {
    pub subscription_id: String,
    pub trade_types: Option<Vec<TradeType>>,
    pub dexes: Option<Vec<String>>,
    pub token_pairs: Option<Vec<String>>,
    pub min_amount: Option<Decimal>,
    pub status_updates: bool,
    pub real_time_updates: bool,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TradeStreamMessage {
    /// New trade executed
    TradeUpdate {
        trade: TradeRecord,
        timestamp: DateTime<Utc>,
    },
    /// Trade status changed
    StatusChange {
        trade_id: Uuid,
        user_id: UserId,
        old_status: TradeStatus,
        new_status: TradeStatus,
        timestamp: DateTime<Utc>,
    },
    /// Bulk trade updates
    BulkUpdate {
        trades: Vec<TradeRecord>,
        timestamp: DateTime<Utc>,
    },
    /// Trade analytics update
    AnalyticsUpdate {
        user_id: UserId,
        total_trades: u64,
        success_rate: Decimal,
        total_volume: Decimal,
        total_pnl: Decimal,
        timestamp: DateTime<Utc>,
    },
    /// Connection status
    ConnectionStatus {
        status: String,
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// Ping/Pong for keepalive
    Ping {
        timestamp: DateTime<Utc>,
    },
    Pong {
        timestamp: DateTime<Utc>,
    },
}

/// Client message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Subscribe to trade updates
    Subscribe {
        subscription: TradeSubscription,
    },
    /// Unsubscribe from updates
    Unsubscribe {
        subscription_id: String,
    },
    /// Request historical trades
    RequestHistory {
        limit: Option<u64>,
        start_date: Option<DateTime<Utc>>,
    },
    /// Ping for keepalive
    Ping {
        timestamp: DateTime<Utc>,
    },
}

/// WebSocket handler for trade streaming
pub async fn handle_trade_websocket(
    ws: WebSocketUpgrade,
    Path(user_id): Path<Uuid>,
    State(state): State<TradeStreamingState>,
) -> Response {
    debug!("WebSocket connection request for user: {}", user_id);
    
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, Arc::new(state)))
}

/// Handle individual WebSocket connection
async fn handle_socket(socket: WebSocket, user_id: UserId, state: Arc<TradeStreamingState>) {
    let connection_id = Uuid::new_v4().to_string();
    info!("New WebSocket connection: {} for user: {}", connection_id, user_id);

    // Register connection
    let connection = UserConnection {
        connection_id: connection_id.clone(),
        user_id,
        connected_at: Utc::now(),
        subscriptions: Vec::new(),
        last_ping: Utc::now(),
    };

    state.active_connections.write().await.insert(connection_id.clone(), connection);

    // Split socket into sender and receiver
    let (mut sender, mut receiver) = socket.split();

    // Create broadcast receiver for this connection
    let mut broadcast_rx = state.broadcaster.subscribe();

    // Send connection confirmation
    let welcome_msg = TradeStreamMessage::ConnectionStatus {
        status: "connected".to_string(),
        message: format!("Connected to trade streaming for user {}", user_id),
        timestamp: Utc::now(),
    };

    if let Ok(msg_json) = serde_json::to_string(&welcome_msg) {
        if sender.send(Message::Text(msg_json)).await.is_err() {
            error!("Failed to send welcome message to {}", connection_id);
            return;
        }
    }

    // Split sender for different handlers
    let (sender_tx, mut sender_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    
    // Sender task
    let sender_task = tokio::spawn({
        let connection_id = connection_id.clone();
        async move {
            while let Some(message) = sender_rx.recv().await {
                if sender.send(message).await.is_err() {
                    error!("Failed to send message to {}", connection_id);
                    break;
                }
            }
        }
    });

    // Handle incoming messages from client
    let client_handler = tokio::spawn({
        let state = state.clone();
        let connection_id = connection_id.clone();
        let sender_tx = sender_tx.clone();
        async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = handle_client_message(&text, &user_id, &connection_id, &state).await {
                            error!("Error handling client message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Client {} closed connection", connection_id);
                        break;
                    }
                    Ok(Message::Ping(data)) => {
                        let _ = sender_tx.send(Message::Pong(data));
                    }
                    Err(e) => {
                        error!("WebSocket error for {}: {}", connection_id, e);
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    // Handle broadcast messages to client
    let broadcast_handler = tokio::spawn({
        let connection_id = connection_id.clone();
        let sender_tx = sender_tx.clone();
        let state_clone = state.clone();
        async move {
            while let Ok(message) = broadcast_rx.recv().await {
                // Check if message is relevant to this user
                if should_send_to_user(&message, &user_id, &state_clone).await {
                    if let Ok(msg_json) = serde_json::to_string(&message) {
                        let _ = sender_tx.send(Message::Text(msg_json));
                    }
                }
            }
        }
    });

    // Periodic ping to keep connection alive
    let ping_handler = tokio::spawn({
        let connection_id = connection_id.clone();
        let sender_tx = sender_tx.clone();
        async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let ping_msg = TradeStreamMessage::Ping {
                    timestamp: Utc::now(),
                };
                if let Ok(msg_json) = serde_json::to_string(&ping_msg) {
                    let _ = sender_tx.send(Message::Text(msg_json));
                }
            }
        }
    });

    // Wait for any handler to complete
    tokio::select! {
        _ = client_handler => {},
        _ = broadcast_handler => {},
        _ = ping_handler => {},
        _ = sender_task => {},
    }

    // Clean up connection
    state.active_connections.write().await.remove(&connection_id);
    info!("WebSocket connection {} closed for user {}", connection_id, user_id);
}

/// Handle client messages
async fn handle_client_message(
    text: &str,
    user_id: &UserId,
    connection_id: &str,
    state: &Arc<TradeStreamingState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client_msg: ClientMessage = serde_json::from_str(text)?;

    match client_msg {
        ClientMessage::Subscribe { subscription } => {
            debug!("User {} subscribing to: {:?}", user_id, subscription);
            
            // Add subscription to connection
            if let Some(connection) = state.active_connections.write().await.get_mut(connection_id) {
                connection.subscriptions.push(subscription.clone());
            }

            // Send confirmation
            let response = TradeStreamMessage::ConnectionStatus {
                status: "subscribed".to_string(),
                message: format!("Subscribed to {}", subscription.subscription_id),
                timestamp: Utc::now(),
            };

            state.broadcaster.send(response)?;
        }
        ClientMessage::Unsubscribe { subscription_id } => {
            debug!("User {} unsubscribing from: {}", user_id, subscription_id);
            
            // Remove subscription from connection
            if let Some(connection) = state.active_connections.write().await.get_mut(connection_id) {
                connection.subscriptions.retain(|s| s.subscription_id != subscription_id);
            }
        }
        ClientMessage::RequestHistory { limit, start_date } => {
            debug!("User {} requesting trade history", user_id);
            
            // Get recent trades for user
            let query = crate::analytics::trade_history::TradeQuery {
                filter: None,
                sort_by: Some(crate::analytics::trade_history::TradeSortBy::Timestamp),
                sort_desc: Some(true),
                page: Some(0),
                page_size: Some(limit.unwrap_or(50) as u32),
            };
            let trades = state.trade_manager.query_trades(user_id, &query).await?;
            
            let response = TradeStreamMessage::BulkUpdate {
                trades,
                timestamp: Utc::now(),
            };

            state.broadcaster.send(response)?;
        }
        ClientMessage::Ping { timestamp: _ } => {
            // Update last ping time
            if let Some(connection) = state.active_connections.write().await.get_mut(connection_id) {
                connection.last_ping = Utc::now();
            }

            let response = TradeStreamMessage::Pong {
                timestamp: Utc::now(),
            };

            state.broadcaster.send(response)?;
        }
    }

    Ok(())
}

/// Check if message should be sent to specific user
async fn should_send_to_user(
    message: &TradeStreamMessage,
    user_id: &UserId,
    state: &Arc<TradeStreamingState>,
) -> bool {
    match message {
        TradeStreamMessage::TradeUpdate { trade, .. } => {
            trade.user_id == *user_id
        }
        TradeStreamMessage::StatusChange { user_id: msg_user_id, .. } => {
            msg_user_id == user_id
        }
        TradeStreamMessage::AnalyticsUpdate { user_id: msg_user_id, .. } => {
            msg_user_id == user_id
        }
        TradeStreamMessage::BulkUpdate { trades, .. } => {
            trades.iter().any(|t| t.user_id == *user_id)
        }
        _ => true, // Send system messages to all users
    }
}

/// Trade streaming manager for coordinating updates
pub struct TradeStreamingManager {
    state: TradeStreamingState,
}

impl TradeStreamingManager {
    pub fn new(trade_manager: Arc<TradeHistoryManager>) -> Self {
        Self {
            state: TradeStreamingState::new(trade_manager),
        }
    }

    pub fn get_state(&self) -> TradeStreamingState {
        self.state.clone()
    }

    /// Notify of new trade execution
    pub async fn notify_trade_executed(&self, trade: TradeRecord) {
        self.state.broadcast_trade_update(trade).await;
    }

    /// Notify of trade status change
    pub async fn notify_status_change(&self, trade_id: Uuid, user_id: UserId, old_status: TradeStatus, new_status: TradeStatus) {
        self.state.broadcast_status_change(trade_id, user_id, old_status, new_status).await;
    }

    /// Send analytics update to user
    pub async fn send_analytics_update(&self, user_id: UserId) {
        if let Ok(analytics) = self.state.trade_manager.calculate_analytics(&user_id).await {
            let message = TradeStreamMessage::AnalyticsUpdate {
                user_id,
                total_trades: analytics.total_trades,
                success_rate: analytics.success_rate,
                total_volume: analytics.total_volume_usd,
                total_pnl: analytics.total_pnl_usd,
                timestamp: Utc::now(),
            };

            if let Err(e) = self.state.broadcaster.send(message) {
                warn!("Failed to send analytics update: {}", e);
            }
        }
    }

    /// Get streaming statistics
    pub async fn get_stats(&self) -> StreamingStats {
        let connections = self.state.get_connection_count().await;
        let total_subscriptions: usize = self.state.active_connections
            .read()
            .await
            .values()
            .map(|conn| conn.subscriptions.len())
            .sum();

        StreamingStats {
            active_connections: connections,
            total_subscriptions,
            uptime_seconds: 0, // Would track actual uptime in production
        }
    }
}

/// Streaming statistics
#[derive(Debug, Serialize)]
pub struct StreamingStats {
    pub active_connections: usize,
    pub total_subscriptions: usize,
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::trade_history::{MockTradeDataStore, MockTradeSearchIndex, MockTradeDataValidator};

    #[tokio::test]
    async fn test_streaming_state_creation() {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        let trade_manager = Arc::new(TradeHistoryManager::new(data_store, search_index, validator));
        
        let state = TradeStreamingState::new(trade_manager);
        assert_eq!(state.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_message_serialization() {
        let trade = create_test_trade();
        let message = TradeStreamMessage::TradeUpdate {
            trade,
            timestamp: Utc::now(),
        };

        let serialized = serde_json::to_string(&message).unwrap();
        assert!(serialized.contains("TradeUpdate"));
    }

    fn create_test_trade() -> TradeRecord {
        TradeRecord {
            trade_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            trade_type: TradeType::Swap,
            status: TradeStatus::Executed,
            timestamp: Utc::now(),
            execution_timestamp: Some(Utc::now()),
            input_token: "ETH".to_string(),
            output_token: "USDC".to_string(),
            input_amount: Decimal::from(1),
            output_amount: Some(Decimal::from(3000)),
            expected_output: Decimal::from(3000),
            dex_used: "Uniswap".to_string(),
            route_path: vec!["ETH".to_string(), "USDC".to_string()],
            slippage_tolerance: Decimal::from_f64_retain(0.005).unwrap(),
            actual_slippage: Some(Decimal::from_f64_retain(0.002).unwrap()),
            gas_used: Some(150000),
            gas_price: Some(Decimal::from(20)),
            gas_cost_usd: Some(Decimal::from(50)),
            protocol_fees: Decimal::from(5),
            network_fees: Decimal::from(5),
            price_impact: None,
            execution_time_ms: Some(15000),
            pnl_usd: Some(Decimal::from(100)),
            transaction_hash: Some("0x123".to_string()),
            block_number: Some(18000000),
            nonce: None,
            error_message: None,
            metadata: std::collections::HashMap::new(),
        }
    }
}
