use crate::analytics::live_pnl_engine::*;
use crate::analytics::pnl_persistence::*;
use crate::risk_management::RiskError;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::Response,
};
use chrono::{DateTime, Utc};
use futures::{sink::SinkExt, stream::StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Query for aggregated P&L data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPnLQuery {
    pub user_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub interval: String,
}

/// Query for P&L history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLHistoryQuery {
    pub user_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub limit: Option<usize>,
}

/// WebSocket server for real-time P&L streaming
pub struct PnLWebSocketServer {
    pnl_engine: Arc<LivePnLEngine>,
    persistence_manager: Arc<dyn PersistenceInterface>,
    active_connections: Arc<RwLock<HashMap<Uuid, ClientConnection>>>,
    broadcast_sender: broadcast::Sender<PnLBroadcastMessage>,
    server_config: WebSocketConfig,
    connection_stats: Arc<RwLock<ConnectionStats>>,
}

/// Persistence interface for WebSocket server
#[async_trait::async_trait]
pub trait PersistenceInterface: Send + Sync {
    async fn get_latest_pnl_snapshot(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError>;
    async fn get_pnl_history(&self, query: &PnLHistoryQuery) -> Result<Vec<PnLSnapshot>, RiskError>;
    async fn get_aggregated_pnl_data(&self, query: &AggregatedPnLQuery) -> Result<Vec<AggregatedPnLData>, RiskError>;
    async fn get_position_history(&self, user_id: Uuid, token_address: &str, chain_id: u64, start_time: chrono::DateTime<chrono::Utc>, end_time: chrono::DateTime<chrono::Utc>) -> Result<Vec<crate::analytics::data_models::PositionPnL>, RiskError>;
    async fn get_persistence_stats(&self) -> Result<PersistenceStats, RiskError>;
}

/// Client WebSocket connection
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub connection_id: Uuid,
    pub user_id: Uuid,
    pub connected_at: DateTime<Utc>,
    pub last_ping: DateTime<Utc>,
    pub subscriptions: Vec<PnLSubscription>,
    pub message_count: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// P&L subscription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSubscription {
    pub subscription_id: Uuid,
    pub user_id: Uuid,
    pub subscription_type: SubscriptionType,
    pub update_interval_ms: Option<u64>,
    pub token_filter: Option<Vec<String>>,
    pub chain_filter: Option<Vec<u64>>,
    pub min_change_threshold: Option<Decimal>, // Only send updates if change exceeds threshold
}

/// Subscription types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionType {
    RealTimePnL,
    PositionUpdates,
    PortfolioSummary,
    PriceAlerts,
    HistoricalData,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    // Client to server messages
    Subscribe {
        subscription: PnLSubscription,
    },
    Unsubscribe {
        subscription_id: Uuid,
    },
    GetSnapshot {
        user_id: Uuid,
    },
    GetHistory {
        query: PnLHistoryQuery,
    },
    Ping {
        timestamp: DateTime<Utc>,
    },
    
    // Server to client messages
    PnLUpdate {
        subscription_id: Uuid,
        snapshot: PnLSnapshot,
        change_summary: PnLChangeSummary,
    },
    PositionUpdate {
        subscription_id: Uuid,
        position: PositionPnL,
        change_type: PositionChangeType,
    },
    PortfolioSummary {
        subscription_id: Uuid,
        summary: PortfolioSummary,
    },
    HistoricalData {
        query_id: Uuid,
        snapshots: Vec<PnLSnapshot>,
    },
    Error {
        error_code: String,
        message: String,
    },
    Pong {
        timestamp: DateTime<Utc>,
    },
    SubscriptionConfirmed {
        subscription_id: Uuid,
    },
}

/// P&L change summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLChangeSummary {
    pub total_pnl_change_usd: Decimal,
    pub total_pnl_change_percent: Decimal,
    pub portfolio_value_change_usd: Decimal,
    pub portfolio_value_change_percent: Decimal,
    pub positions_changed: u32,
    pub new_positions: u32,
    pub closed_positions: u32,
    pub largest_gain_token: Option<String>,
    pub largest_loss_token: Option<String>,
}

/// Position change types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PositionChangeType {
    NewPosition,
    PositionClosed,
    BalanceChanged,
    PriceChanged,
    PnLChanged,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub total_portfolio_value_usd: Decimal,
    pub total_pnl_usd: Decimal,
    pub total_pnl_percent: Decimal,
    pub position_count: u32,
    pub top_performers: Vec<TopPerformer>,
    pub worst_performers: Vec<TopPerformer>,
    pub diversification_score: Decimal,
}

/// Top/worst performing positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopPerformer {
    pub token_address: String,
    pub symbol: String,
    pub pnl_usd: Decimal,
    pub pnl_percent: Decimal,
    pub position_value_usd: Decimal,
}

/// Broadcast message for internal distribution
#[derive(Debug, Clone)]
pub struct PnLBroadcastMessage {
    pub message_type: BroadcastMessageType,
    pub user_id: Uuid,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Broadcast message types
#[derive(Debug, Clone)]
pub enum BroadcastMessageType {
    PnLUpdate,
    PositionChange,
    PortfolioUpdate,
    PriceAlert,
}

/// WebSocket server configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    pub max_connections: usize,
    pub ping_interval_seconds: u64,
    pub connection_timeout_seconds: u64,
    pub max_message_size: usize,
    pub rate_limit_messages_per_minute: u32,
    pub enable_compression: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections: 10000,
            ping_interval_seconds: 30,
            connection_timeout_seconds: 300, // 5 minutes
            max_message_size: 1024 * 1024, // 1MB
            rate_limit_messages_per_minute: 100,
            enable_compression: true,
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_messages_sent: u64,
    pub total_messages_received: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub connection_errors: u64,
    pub message_errors: u64,
    pub average_connection_duration_seconds: f64,
}

impl PnLWebSocketServer {
    /// Broadcast P&L update to all connected clients
    pub async fn broadcast_pnl_update(&self, snapshot: &PnLSnapshot) -> Result<(), RiskError> {
        let broadcast_msg = PnLBroadcastMessage {
            message_type: BroadcastMessageType::PnLUpdate,
            user_id: snapshot.user_id,
            data: serde_json::to_value(snapshot).map_err(|e| RiskError::DatabaseError(format!("Serialization error: {}", e)))?,
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = self.broadcast_sender.send(broadcast_msg) {
            return Err(RiskError::DatabaseError(format!("Failed to broadcast P&L update: {}", e)));
        }
        
        Ok(())
    }

    /// Get connection count
    pub async fn get_connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }

    /// Stop the WebSocket server
    pub async fn stop(&self) -> Result<(), RiskError> {
        // Simplified stop implementation for compilation
        Ok(())
    }


    /// Create new P&L WebSocket server
    pub async fn new(
        pnl_engine: Arc<LivePnLEngine>,
        persistence_manager: Arc<dyn PersistenceInterface>,
        config: WebSocketConfig,
    ) -> Result<Self, RiskError> {
        let (broadcast_sender, _) = broadcast::channel(10000);
        
        Ok(Self {
            pnl_engine,
            persistence_manager,
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            broadcast_sender,
            server_config: config,
            connection_stats: Arc::new(RwLock::new(ConnectionStats::default())),
        })
    }

    /// Handle WebSocket upgrade
    pub async fn handle_websocket_upgrade(
        ws: WebSocketUpgrade,
        Path(user_id): Path<Uuid>,
        State(server): State<Arc<PnLWebSocketServer>>,
    ) -> Response {
        ws.on_upgrade(move |socket| {
            let server = Arc::clone(&server);
            async move {
                server.handle_websocket_connection(socket, user_id).await
            }
        })
    }

    /// Handle individual WebSocket connection
    pub async fn handle_websocket_connection(&self, socket: WebSocket, user_id: Uuid) {
        let connection_id = Uuid::new_v4();
        let connection = ClientConnection {
            connection_id,
            user_id,
            connected_at: Utc::now(),
            last_ping: Utc::now(),
            subscriptions: Vec::new(),
            message_count: 0,
            bytes_sent: 0,
            bytes_received: 0,
        };

        // Add to active connections
        {
            let mut connections = self.active_connections.write().await;
            connections.insert(connection_id, connection.clone());
            
            let mut stats = self.connection_stats.write().await;
            stats.total_connections += 1;
            stats.active_connections += 1;
        }

        info!("New WebSocket connection established: {} for user {}", connection_id, user_id);

        // Split socket into sender and receiver
        let (sender, mut receiver) = socket.split();
        let sender = Arc::new(tokio::sync::Mutex::new(sender));

        // Subscribe to broadcast messages
        let mut broadcast_receiver = self.broadcast_sender.subscribe();

        // Handle incoming messages
        let persistence_manager = Arc::clone(&self.persistence_manager);
        let sender_clone = Arc::clone(&sender);
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Handle client message (simplified for now)
                        debug!("Received text message from {}: {}", connection_id, text);
                    }
                    Ok(Message::Binary(data)) => {
                        if let Ok(text) = String::from_utf8(data) {
                            debug!("Received binary message from {}: {}", connection_id, text);
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        let mut sender_guard = sender_clone.lock().await;
                        if sender_guard.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket connection closed by client: {}", connection_id);
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Handle outgoing messages
        let ping_interval = self.server_config.ping_interval_seconds;
        let sender_clone = Arc::clone(&sender);
        let outgoing_task = tokio::spawn(async move {
            while let Ok(broadcast_msg) = broadcast_receiver.recv().await {
                // Simplified message handling
                if let Ok(message_text) = serde_json::to_string(&broadcast_msg.data) {
                    let mut sender_guard = sender_clone.lock().await;
                    if sender_guard.send(Message::Text(message_text)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Start periodic ping task
        let sender_clone = Arc::clone(&sender);
        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(ping_interval));
            
            loop {
                interval.tick().await;
                
                let ping_message = WebSocketMessage::Ping {
                    timestamp: Utc::now(),
                };
                
                if let Ok(message_text) = serde_json::to_string(&ping_message) {
                    let mut sender_guard = sender_clone.lock().await;
                    if sender_guard.send(Message::Text(message_text)).await.is_err() {
                        break;
                    }
                }
            }
        });

        // Wait for any task to complete (indicating connection should close)
        tokio::select! {
            _ = incoming_task => {},
            _ = outgoing_task => {},
            _ = ping_task => {},
        }

        // Clean up connection
        self.cleanup_connection(connection_id).await;
    }

    /// Handle client message
    async fn handle_client_message(&self, connection_id: Uuid, message: &str) -> Result<(), RiskError> {
        let ws_message: WebSocketMessage = serde_json::from_str(message)
            .map_err(|e| RiskError::ValidationError(format!("Invalid WebSocket message: {}", e)))?;

        match ws_message {
            WebSocketMessage::Subscribe { subscription } => {
                self.handle_subscription(connection_id, subscription).await?;
            }
            WebSocketMessage::Unsubscribe { subscription_id } => {
                self.handle_unsubscription(connection_id, subscription_id).await?;
            }
            WebSocketMessage::GetSnapshot { user_id } => {
                self.handle_snapshot_request(connection_id, user_id).await?;
            }
            WebSocketMessage::GetHistory { query } => {
                self.handle_history_request(connection_id, query).await?;
            }
            WebSocketMessage::Ping { timestamp: _ } => {
                self.handle_ping(connection_id).await?;
            }
            _ => {
                return Err(RiskError::ValidationError("Invalid client message type".to_string()));
            }
        }

        Ok(())
    }

    /// Handle subscription request
    async fn handle_subscription(&self, connection_id: Uuid, subscription: PnLSubscription) -> Result<(), RiskError> {
        // Add subscription to connection
        {
            let mut connections = self.active_connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.subscriptions.push(subscription.clone());
            }
        }

        // Send confirmation
        let confirmation = WebSocketMessage::SubscriptionConfirmed {
            subscription_id: subscription.subscription_id,
        };

        self.send_message_to_connection(connection_id, confirmation).await?;

        info!("Subscription created: {} for connection {}", subscription.subscription_id, connection_id);

        Ok(())
    }

    /// Handle unsubscription request
    async fn handle_unsubscription(&self, connection_id: Uuid, subscription_id: Uuid) -> Result<(), RiskError> {
        let mut connections = self.active_connections.write().await;
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.subscriptions.retain(|s| s.subscription_id != subscription_id);
        }

        info!("Subscription removed: {} from connection {}", subscription_id, connection_id);

        Ok(())
    }

    /// Handle snapshot request
    async fn handle_snapshot_request(&self, connection_id: Uuid, user_id: Uuid) -> Result<(), RiskError> {
        if let Some(snapshot) = self.persistence_manager.get_latest_pnl_snapshot(user_id).await? {
            let response = WebSocketMessage::PnLUpdate {
                subscription_id: Uuid::new_v4(), // One-time request
                snapshot,
                change_summary: PnLChangeSummary {
                    total_pnl_change_usd: Decimal::ZERO,
                    total_pnl_change_percent: Decimal::ZERO,
                    portfolio_value_change_usd: Decimal::ZERO,
                    portfolio_value_change_percent: Decimal::ZERO,
                    positions_changed: 0,
                    new_positions: 0,
                    closed_positions: 0,
                    largest_gain_token: None,
                    largest_loss_token: None,
                },
            };

            self.send_message_to_connection(connection_id, response).await?;
        }

        Ok(())
    }

    /// Handle history request
    async fn handle_history_request(&self, connection_id: Uuid, query: PnLHistoryQuery) -> Result<(), RiskError> {
        let snapshots = self.persistence_manager.get_pnl_history(&query).await?;
        
        let response = WebSocketMessage::HistoricalData {
            query_id: Uuid::new_v4(),
            snapshots,
        };

        self.send_message_to_connection(connection_id, response).await?;

        Ok(())
    }

    /// Handle ping message
    async fn handle_ping(&self, connection_id: Uuid) -> Result<(), RiskError> {
        // Update last ping time
        {
            let mut connections = self.active_connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.last_ping = Utc::now();
            }
        }

        // Send pong response
        let pong = WebSocketMessage::Pong {
            timestamp: Utc::now(),
        };

        self.send_message_to_connection(connection_id, pong).await?;

        Ok(())
    }

    /// Send message to specific connection
    async fn send_message_to_connection(&self, connection_id: Uuid, message: WebSocketMessage) -> Result<(), RiskError> {
        let broadcast_msg = PnLBroadcastMessage {
            message_type: BroadcastMessageType::PnLUpdate,
            user_id: Uuid::new_v4(), // Will be filtered by connection
            data: serde_json::to_value(message)?,
            timestamp: Utc::now(),
        };

        self.broadcast_sender.send(broadcast_msg)
            .map_err(|e| RiskError::ValidationError(format!("Failed to send message: {}", e)))?;

        Ok(())
    }

    /// Check if connection should receive broadcast message
    async fn should_send_to_connection(&self, connection_id: Uuid, broadcast_msg: &PnLBroadcastMessage) -> bool {
        let connections = self.active_connections.read().await;
        if let Some(connection) = connections.get(&connection_id) {
            // Check if any subscription matches this message
            for subscription in &connection.subscriptions {
                if self.subscription_matches_message(subscription, broadcast_msg) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if subscription matches broadcast message
    fn subscription_matches_message(&self, subscription: &PnLSubscription, broadcast_msg: &PnLBroadcastMessage) -> bool {
        // Simple matching logic - would be more sophisticated in production
        subscription.user_id == broadcast_msg.user_id
    }

    /// Convert broadcast message to WebSocket message
    async fn convert_broadcast_to_websocket_message(&self, broadcast_msg: &PnLBroadcastMessage) -> WebSocketMessage {
        // Convert based on message type - simplified implementation
        serde_json::from_value(broadcast_msg.data.clone()).unwrap_or(WebSocketMessage::Error {
            error_code: "CONVERSION_ERROR".to_string(),
            message: "Failed to convert broadcast message".to_string(),
        })
    }

    /// Clean up connection
    async fn cleanup_connection(&self, connection_id: Uuid) {
        let mut connections = self.active_connections.write().await;
        if let Some(connection) = connections.remove(&connection_id) {
            let mut stats = self.connection_stats.write().await;
            stats.active_connections = stats.active_connections.saturating_sub(1);
            
            let connection_duration = Utc::now().signed_duration_since(connection.connected_at).num_seconds() as f64;
            stats.average_connection_duration_seconds = 
                (stats.average_connection_duration_seconds * (stats.total_connections - 1) as f64 + connection_duration) 
                / stats.total_connections as f64;

            info!("WebSocket connection cleaned up: {} (duration: {:.1}s)", connection_id, connection_duration);
        }
    }

    /// Start P&L update broadcasting
    pub async fn start_pnl_broadcasting(&self) -> Result<(), RiskError> {
        let mut pnl_receiver = self.pnl_engine.subscribe_to_pnl_updates();
        let broadcast_sender = self.broadcast_sender.clone();

        tokio::spawn(async move {
            while let Ok(pnl_update) = pnl_receiver.recv().await {
                let broadcast_msg = PnLBroadcastMessage {
                    message_type: BroadcastMessageType::PnLUpdate,
                    user_id: pnl_update.user_id,
                    data: serde_json::to_value(pnl_update).unwrap_or_default(),
                    timestamp: Utc::now(),
                };

                if let Err(e) = broadcast_sender.send(broadcast_msg) {
                    error!("Failed to broadcast P&L update: {}", e);
                }
            }
        });

        info!("P&L broadcasting started");

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        self.connection_stats.read().await.clone()
    }

    /// Get active connection count
    pub async fn get_active_connection_count(&self) -> usize {
        self.active_connections.read().await.len()
    }
}
