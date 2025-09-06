use tokio_tungstenite::{
    accept_async, tungstenite::protocol::Message, WebSocketStream,
};
use tokio::net::{TcpListener, TcpStream};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::{info, warn, error, debug};

use super::risk_engine::RiskProcessingEngine;
use super::database::RiskDatabase;

#[derive(Debug, Clone)]
pub struct RiskMetricsWithPnL {
    pub total_exposure: f64,
    pub var_95: f64,
    pub pnl: f64,
}

// MockPosition struct removed - now using real position data from position tracker

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    #[serde(rename = "risk_metrics_update")]
    RiskMetrics(RiskMetricsUpdate),
    #[serde(rename = "position_update")]
    Position(PositionUpdate),
    #[serde(rename = "alert")]
    Alert(AlertMessage),
    #[serde(rename = "portfolio_update")]
    Portfolio(PortfolioUpdate),
    #[serde(rename = "subscription_ack")]
    SubscriptionAck(SubscriptionAck),
    #[serde(rename = "error")]
    Error(ErrorMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetricsUpdate {
    pub user_id: String,
    pub total_exposure: f64,
    pub var_95: f64,
    pub pnl: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub user_id: String,
    pub positions: Vec<PositionData>,
    pub total_pnl: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionData {
    pub token: String,
    pub amount: f64,
    pub current_price: f64,
    pub entry_price: f64,
    pub pnl: f64,
    pub pnl_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMessage {
    pub user_id: String,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub threshold: f64,
    pub current_value: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioUpdate {
    pub user_id: String,
    pub composition: Vec<AssetAllocation>,
    pub diversification_ratio: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetAllocation {
    pub token: String,
    pub percentage: f64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionAck {
    pub channel: String,
    pub user_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct ClientMessage {
    #[serde(rename = "type")]
    message_type: String,
    channel: Option<String>,
    user_id: Option<String>,
    alert_type: Option<String>,
    threshold: Option<f64>,
    current_value: Option<f64>,
}

#[derive(Debug, Clone)]
struct ClientConnection {
    id: Uuid,
    user_id: String,
    subscriptions: Vec<String>,
    sender: tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
}

pub struct RiskWebSocketServer {
    risk_engine: Arc<RiskProcessingEngine>,
    clients: Arc<RwLock<HashMap<Uuid, ClientConnection>>>,
    broadcast_tx: broadcast::Sender<WebSocketMessage>,
}

impl RiskWebSocketServer {
    pub fn new(risk_engine: RiskProcessingEngine) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            risk_engine: Arc::new(risk_engine),
            clients: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
        }
    }

    /// Calculate risk metrics with real PnL from database positions
    pub async fn calculate_risk_metrics_with_pnl(&self, user_id: &str, database: &RiskDatabase) -> Result<RiskMetricsWithPnL> {
        let user_uuid = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        
        // Get positions from database
        let positions = database.get_user_positions_by_uuid(user_uuid).await
            .map_err(|e| anyhow::anyhow!("Failed to get positions: {}", e))?;
        
        let pnl = if let Some(positions) = positions {
            self.calculate_portfolio_pnl(&positions).await
        } else {
            0.0
        };
        
        // Get risk metrics from engine
        let metrics = self.risk_engine.calculate_user_risk_metrics(&user_uuid).await
            .map_err(|e| anyhow::anyhow!("Failed to calculate risk metrics: {}", e))?;
        
        Ok(RiskMetricsWithPnL {
            total_exposure: metrics.total_exposure_usd.to_string().parse().unwrap_or(0.0),
            var_95: metrics.var_95.to_string().parse().unwrap_or(0.0),
            pnl,
        })
    }

    /// Calculate portfolio PnL from positions
    async fn calculate_portfolio_pnl(&self, positions: &super::types::UserPositions) -> f64 {
        let mut total_pnl = 0.0;
        
        for (token, balance) in &positions.balances {
            // Get current price and calculate PnL
            let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
            let token_amount = balance.balance.to_string().parse::<f64>().unwrap_or(0.0);
            
            if token_amount > 0.0 {
                let current_price = current_value / token_amount;
                
                // For now, use a simple entry price estimation
                // In production, this should come from trade history
                let estimated_entry_price = match token.as_str() {
                    "ETH" => 3200.0,
                    "BTC" => 65000.0,
                    "USDC" | "USDT" | "DAI" => 1.0,
                    _ => current_price * 0.9, // Assume 10% profit for unknown tokens
                };
                
                let entry_value = token_amount * estimated_entry_price;
                let pnl = current_value - entry_value;
                total_pnl += pnl;
                
                debug!("Token: {}, Amount: {}, Current Price: ${:.2}, Entry Price: ${:.2}, PnL: ${:.2}", 
                       token, token_amount, current_price, estimated_entry_price, pnl);
            }
        }
        
        info!("📊 Total Portfolio PnL: ${:.2}", total_pnl);
        total_pnl
    }

    pub async fn start(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("🚀 Risk WebSocket server listening on: {}", addr);

        // Start background task for periodic risk metrics updates
        self.start_risk_metrics_broadcaster().await;

        while let Ok((stream, addr)) = listener.accept().await {
            debug!("📡 New connection from: {}", addr);
            let server_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = server_clone.handle_connection(stream).await {
                    error!("❌ Connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(&self, stream: TcpStream) -> Result<()> {
        let ws_stream = accept_async(stream).await?;
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        let client_id = Uuid::new_v4();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        // Handle outgoing messages
        let outgoing_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let json_msg = serde_json::to_string(&msg).unwrap_or_else(|e| {
                    format!(r#"{{"type":"error","data":{{"message":"Serialization error: {}"}}}}"#, e)
                });
                
                if ws_sender.send(Message::Text(json_msg)).await.is_err() {
                    break;
                }
            }
        });

        // Handle incoming messages
        let clients = self.clients.clone();
        let risk_engine = self.risk_engine.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                            Self::handle_client_message(
                                client_id,
                                client_msg,
                                &clients,
                                &risk_engine,
                                &tx,
                                &broadcast_tx,
                            ).await;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }
            
            // Clean up client on disconnect
            clients.write().await.remove(&client_id);
            debug!("🔌 Client {} disconnected", client_id);
        });

        // Wait for either task to complete
        tokio::select! {
            _ = outgoing_task => {},
            _ = incoming_task => {},
        }

        Ok(())
    }

    async fn handle_client_message(
        client_id: Uuid,
        msg: ClientMessage,
        clients: &Arc<RwLock<HashMap<Uuid, ClientConnection>>>,
        risk_engine: &Arc<RiskProcessingEngine>,
        sender: &tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
        broadcast_tx: &broadcast::Sender<WebSocketMessage>,
    ) {
        match msg.message_type.as_str() {
            "subscribe" => {
                if let (Some(channel), Some(user_id)) = (msg.channel, msg.user_id) {
                    let mut clients_guard = clients.write().await;
                    
                    let connection = ClientConnection {
                        id: client_id,
                        user_id: user_id.clone(),
                        subscriptions: vec![channel.clone()],
                        sender: sender.clone(),
                    };
                    
                    clients_guard.insert(client_id, connection);
                    
                    // Send subscription acknowledgment
                    let ack = WebSocketMessage::SubscriptionAck(SubscriptionAck {
                        channel: channel.clone(),
                        user_id: user_id.clone(),
                        status: "subscribed".to_string(),
                    });
                    
                    let _ = sender.send(ack);
                    
                    // Send initial data based on channel
                    match channel.as_str() {
                        "risk_metrics" => {
                            Self::send_risk_metrics_update(&user_id, risk_engine, sender).await;
                        }
                        "positions" => {
                            Self::send_position_update(&user_id, risk_engine, sender).await;
                        }
                        "portfolio" => {
                            Self::send_portfolio_update(&user_id, risk_engine, sender).await;
                        }
                        _ => {}
                    }
                    
                    info!("✅ Client {} subscribed to {} for user {}", client_id, channel, user_id);
                }
            }
            "trigger_alert" => {
                // Manual triggers are deprecated - using automatic threshold monitoring
                warn!("Manual alert trigger deprecated. Using automatic threshold monitoring instead.");
            }
            _ => {
                let error = WebSocketMessage::Error(ErrorMessage {
                    code: "UNKNOWN_MESSAGE_TYPE".to_string(),
                    message: format!("Unknown message type: {}", msg.message_type),
                });
                let _ = sender.send(error);
            }
        }
    }

    async fn send_risk_metrics_update(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
        sender: &tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    ) {
        // Get real risk metrics from risk engine
        let user_uuid = uuid::Uuid::parse_str(user_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        if let Ok(metrics) = risk_engine.calculate_user_risk_metrics(&user_uuid).await {
            // Calculate real PnL from positions
            let pnl = Self::calculate_real_pnl_for_user(&user_uuid, risk_engine).await;
            
            let update = WebSocketMessage::RiskMetrics(RiskMetricsUpdate {
                user_id: user_id.to_string(),
                total_exposure: metrics.total_exposure_usd.to_string().parse().unwrap_or(0.0),
                var_95: metrics.var_95.to_string().parse().unwrap_or(0.0),
                pnl,
                timestamp: Utc::now(),
            });
            
            let _ = sender.send(update);
        }
    }

    /// Calculate real PnL for a user from their positions
    pub async fn calculate_real_pnl_for_user(user_uuid: &uuid::Uuid, risk_engine: &Arc<RiskProcessingEngine>) -> f64 {
        // Try to get positions from the position tracker
        if let Some(position) = risk_engine.get_position_tracker().get_user_position(user_uuid) {
            Self::calculate_pnl_from_position(&position).await
        } else {
            // No position found - return 0 PnL
            0.0
        }
    }

    /// Get real position updates from position tracker
    pub async fn get_real_position_updates(user_id: &str, risk_engine: &Arc<RiskProcessingEngine>) -> Vec<PositionData> {
        let user_uuid = match uuid::Uuid::parse_str(user_id) {
            Ok(uuid) => uuid,
            Err(_) => return Vec::new(),
        };

        if let Some(position) = risk_engine.get_position_tracker().get_user_position(&user_uuid) {
            Self::convert_positions_to_position_data(&position).await
        } else {
            Vec::new()
        }
    }

    /// Send real position update via WebSocket
    pub async fn send_real_position_update(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
        sender: &tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    ) {
        let position_data = Self::get_real_position_updates(user_id, risk_engine).await;
        let total_pnl: f64 = position_data.iter().map(|p| p.pnl).sum();

        let update = WebSocketMessage::Position(PositionUpdate {
            user_id: user_id.to_string(),
            positions: position_data,
            total_pnl,
            timestamp: Utc::now(),
        });

        let _ = sender.send(update);
    }

    /// Convert UserPositions to PositionData for WebSocket messages
    async fn convert_positions_to_position_data(positions: &super::types::UserPositions) -> Vec<PositionData> {
        let mut position_data = Vec::new();

        for (token, balance) in &positions.balances {
            let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
            let token_amount = balance.balance.to_string().parse::<f64>().unwrap_or(0.0);

            if token_amount > 0.0 {
                let current_price = current_value / token_amount;

                // Entry price estimation (same logic as PnL calculation)
                let estimated_entry_price = match token.as_str() {
                    "ETH" => 3200.0,
                    "BTC" => 65000.0,
                    "USDC" | "USDT" | "DAI" => 1.0,
                    _ => current_price * 0.9,
                };

                let entry_value = token_amount * estimated_entry_price;
                let pnl = current_value - entry_value;
                let pnl_percentage = if entry_value > 0.0 {
                    (pnl / entry_value) * 100.0
                } else {
                    0.0
                };

                position_data.push(PositionData {
                    token: token.clone(),
                    amount: token_amount,
                    current_price,
                    entry_price: estimated_entry_price,
                    pnl,
                    pnl_percentage,
                });

                debug!("💼 Position: {} - Amount: {}, PnL: ${:.2} ({:.2}%)", 
                       token, token_amount, pnl, pnl_percentage);
            }
        }

        info!("📊 Real position updates generated: {} positions", position_data.len());
        position_data
    }

    /// Calculate PnL from a position
    pub async fn calculate_pnl_from_position(position: &super::types::UserPositions) -> f64 {
        let mut total_pnl = 0.0;
        
        for (token, balance) in &position.balances {
            let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
            let token_amount = balance.balance.to_string().parse::<f64>().unwrap_or(0.0);
            
            if token_amount > 0.0 {
                let current_price = current_value / token_amount;
                
                // Entry price estimation (in production, get from trade history)
                let estimated_entry_price = match token.as_str() {
                    "ETH" => 3200.0,
                    "BTC" => 65000.0,
                    "USDC" | "USDT" | "DAI" => 1.0,
                    _ => current_price * 0.9,
                };
                
                let entry_value = token_amount * estimated_entry_price;
                let pnl = current_value - entry_value;
                total_pnl += pnl;
                
                debug!("💰 Token: {}, PnL: ${:.2}", token, pnl);
            }
        }
        
        info!("📊 Real-time PnL calculated: ${:.2}", total_pnl);
        total_pnl
    }

    // Check risk thresholds and return breach alerts
    pub async fn check_risk_thresholds(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
    ) -> Vec<AlertMessage> {
        let mut alerts = Vec::new();
        
        // Parse user ID
        let user_uuid = match uuid::Uuid::parse_str(user_id) {
            Ok(uuid) => uuid,
            Err(_) => {
                warn!("Invalid user ID format: {}", user_id);
                return alerts;
            }
        };
        
        // Get user position and calculate metrics
        if let Some(position) = risk_engine.get_position_tracker().get_user_position(&user_uuid) {
            // Calculate current metrics
            let total_exposure = Self::calculate_total_exposure(&position).await;
            let var_95 = Self::calculate_var_95(&position).await;
            let pnl = Self::calculate_pnl_from_position(&position).await;
            
            // Define thresholds
            let exposure_threshold = 100000.0; // $100k exposure limit
            let var_threshold = 10000.0; // $10k VaR limit
            let pnl_loss_threshold = -5000.0; // $5k loss limit
            
            // Check exposure threshold
            if total_exposure > exposure_threshold {
                let severity = if total_exposure > exposure_threshold * 1.5 {
                    "high"
                } else if total_exposure > exposure_threshold * 1.2 {
                    "medium"
                } else {
                    "low"
                };
                
                alerts.push(AlertMessage {
                    user_id: user_id.to_string(),
                    alert_type: "exposure".to_string(),
                    severity: severity.to_string(),
                    message: format!(
                        "Total exposure threshold breached: ${:.2} > ${:.2}",
                        total_exposure, exposure_threshold
                    ),
                    threshold: exposure_threshold,
                    current_value: total_exposure,
                    timestamp: Utc::now(),
                });
            }
            
            // Check VaR threshold
            if var_95 > var_threshold {
                let severity = if var_95 > var_threshold * 1.5 {
                    "high"
                } else if var_95 > var_threshold * 1.2 {
                    "medium"
                } else {
                    "low"
                };
                
                alerts.push(AlertMessage {
                    user_id: user_id.to_string(),
                    alert_type: "var_95".to_string(),
                    severity: severity.to_string(),
                    message: format!(
                        "VaR 95% threshold breached: ${:.2} > ${:.2}",
                        var_95, var_threshold
                    ),
                    threshold: var_threshold,
                    current_value: var_95,
                    timestamp: Utc::now(),
                });
            }
            
            // Check PnL loss threshold
            if pnl < pnl_loss_threshold {
                alerts.push(AlertMessage {
                    user_id: user_id.to_string(),
                    alert_type: "pnl_loss".to_string(),
                    severity: "high".to_string(),
                    message: format!(
                        "PnL loss threshold breached: ${:.2} < ${:.2}",
                        pnl, pnl_loss_threshold
                    ),
                    threshold: pnl_loss_threshold,
                    current_value: pnl,
                    timestamp: Utc::now(),
                });
            }
        }
        
        alerts
    }
    
    // Calculate total exposure from position
    async fn calculate_total_exposure(position: &super::types::UserPositions) -> f64 {
        let mut total_exposure = 0.0;
        
        for (token, balance) in &position.balances {
            let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
            total_exposure += current_value;
        }
        
        total_exposure
    }
    
    // Calculate VaR 95% from position
    async fn calculate_var_95(position: &super::types::UserPositions) -> f64 {
        let mut var_95 = 0.0;
        
        for (token, balance) in &position.balances {
            let current_value = balance.value_usd.to_string().parse::<f64>().unwrap_or(0.0);
            
            if current_value > 0.0 {
                // Estimate volatility-based VaR (simplified)
                let volatility = match token.as_str() {
                    "ETH" => 0.05, // 5% daily volatility
                    "BTC" | "WBTC" => 0.04, // 4% daily volatility
                    "USDC" | "USDT" | "DAI" => 0.001, // 0.1% stablecoin volatility
                    _ => 0.03, // 3% default volatility
                };
                
                let position_var = current_value * volatility * 1.645; // 95% confidence
                var_95 += position_var;
            }
        }
        
        var_95
    }
    
    // Send threshold alerts automatically
    pub async fn send_threshold_alerts(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
        broadcast_tx: &broadcast::Sender<WebSocketMessage>,
    ) {
        let alerts = Self::check_risk_thresholds(user_id, risk_engine).await;
        
        for alert in alerts {
            debug!("🚨 Threshold breach detected: {} for user {}", alert.alert_type, user_id);
            let _ = broadcast_tx.send(WebSocketMessage::Alert(alert));
        }
    }

    async fn send_position_update(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
        sender: &tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    ) {
        // Use real position data from position tracker
        Self::send_real_position_update(user_id, risk_engine, sender).await;
    }

    async fn send_portfolio_update(
        user_id: &str,
        risk_engine: &Arc<RiskProcessingEngine>,
        sender: &tokio::sync::mpsc::UnboundedSender<WebSocketMessage>,
    ) {
        // Use real position data for portfolio calculation
        let position_data = Self::get_real_position_updates(user_id, risk_engine).await;
        
        if position_data.is_empty() {
            return; // No positions to report
        }
        
        let total_value: f64 = position_data.iter()
            .map(|pos| pos.amount * pos.current_price)
            .sum();
        
        let composition: Vec<AssetAllocation> = position_data.iter().map(|pos| {
            let value = pos.amount * pos.current_price;
            let percentage = if total_value > 0.0 { (value / total_value) * 100.0 } else { 0.0 };
            
            AssetAllocation {
                token: pos.token.clone(),
                percentage,
                value,
            }
        }).collect();
        
        // Simple diversification ratio calculation
        let diversification_ratio = if composition.len() > 1 {
            1.0 / composition.iter().map(|a| (a.percentage / 100.0).powi(2)).sum::<f64>()
        } else {
            1.0
        };
        
        let update = WebSocketMessage::Portfolio(PortfolioUpdate {
            user_id: user_id.to_string(),
            composition,
            diversification_ratio,
            timestamp: Utc::now(),
        });
        
        let _ = sender.send(update);
    }

    async fn start_risk_metrics_broadcaster(&self) {
        let clients = self.clients.clone();
        let risk_engine = self.risk_engine.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let clients_guard = clients.read().await;
                for client in clients_guard.values() {
                    if client.subscriptions.contains(&"risk_metrics".to_string()) {
                        Self::send_risk_metrics_update(&client.user_id, &risk_engine, &client.sender).await;
                    }
                    if client.subscriptions.contains(&"positions".to_string()) {
                        Self::send_position_update(&client.user_id, &risk_engine, &client.sender).await;
                    }
                    if client.subscriptions.contains(&"portfolio".to_string()) {
                        Self::send_portfolio_update(&client.user_id, &risk_engine, &client.sender).await;
                    }
                    
                    // Automatic threshold monitoring - check for all subscribed users
                    Self::send_threshold_alerts(&client.user_id, &risk_engine, &broadcast_tx).await;
                }
            }
        });
    }
}

impl Clone for RiskWebSocketServer {
    fn clone(&self) -> Self {
        Self {
            risk_engine: self.risk_engine.clone(),
            clients: self.clients.clone(),
            broadcast_tx: self.broadcast_tx.clone(),
        }
    }
}
