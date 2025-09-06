use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{broadcast, RwLock},
    time::interval,
};
use tracing::{error, info, warn};

use super::portfolio_manager::{PortfolioManager, Portfolio, PortfolioSummary, ChainBalanceResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioUpdate {
    pub user_address: String,
    pub update_type: UpdateType,
    pub timestamp: u64,
    pub data: UpdateData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UpdateType {
    FullPortfolio,
    ChainBalance,
    Summary,
    PriceUpdate,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UpdateData {
    Portfolio(Portfolio),
    ChainBalance(ChainBalanceResponse),
    Summary(PortfolioSummary),
    PriceUpdate(PriceUpdateData),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdateData {
    pub token_symbol: String,
    pub old_price: f64,
    pub new_price: f64,
    pub change_percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    pub address: String,
    pub chains: Option<String>, // Comma-separated chain IDs
    pub update_interval: Option<u64>, // Seconds
}

pub struct PortfolioWebSocketManager {
    portfolio_manager: Arc<PortfolioManager>,
    subscribers: Arc<RwLock<HashMap<String, broadcast::Sender<PortfolioUpdate>>>>,
    price_cache: Arc<RwLock<HashMap<String, f64>>>,
}

impl PortfolioWebSocketManager {
    pub fn new(portfolio_manager: Arc<PortfolioManager>) -> Self {
        Self {
            portfolio_manager,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_websocket(
        &self,
        ws: WebSocketUpgrade,
        query: Query<WebSocketQuery>,
    ) -> Response {
        let user_address = query.address.clone();
        let chains: Vec<u64> = query
            .chains
            .as_ref()
            .map(|c| {
                c.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect()
            })
            .unwrap_or_else(|| vec![1, 10, 137, 42161, 8453]); // Default chains
        
        let update_interval = Duration::from_secs(query.update_interval.unwrap_or(30));

        let manager = self.clone();
        
        ws.on_upgrade(move |socket| {
            manager.handle_socket(socket, user_address, chains, update_interval)
        })
    }

    async fn handle_socket(
        self,
        socket: WebSocket,
        user_address: String,
        chains: Vec<u64>,
        update_interval: Duration,
    ) {
        let (mut sender, mut receiver) = socket.split();
        
        // Create broadcast channel for this user
        let (tx, mut rx) = broadcast::channel(100);
        
        // Store subscriber
        {
            let mut subscribers = self.subscribers.write().await;
            subscribers.insert(user_address.clone(), tx.clone());
        }

        info!("WebSocket connected for user: {}", user_address);

        // Send initial portfolio data
        if let Err(e) = self.send_initial_data(&user_address, &chains, &tx).await {
            warn!("Failed to send initial data: {}", e);
        }

        // Spawn background task for periodic updates
        let manager_clone = self.clone();
        let user_address_clone = user_address.clone();
        let chains_clone = chains.clone();
        let tx_clone = tx.clone();
        
        tokio::spawn(async move {
            manager_clone.periodic_updates(user_address_clone, chains_clone, tx_clone, update_interval).await;
        });

        // Handle incoming messages and outgoing broadcasts
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg = receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_client_message(&text, &user_address, &tx).await {
                                warn!("Failed to handle client message: {}", e);
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed for user: {}", user_address);
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error for user {}: {}", user_address, e);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                
                // Handle outgoing broadcasts
                update = rx.recv() => {
                    match update {
                        Ok(portfolio_update) => {
                            let json = match serde_json::to_string(&portfolio_update) {
                                Ok(j) => j,
                                Err(e) => {
                                    error!("Failed to serialize update: {}", e);
                                    continue;
                                }
                            };
                            
                            if sender.send(Message::Text(json)).await.is_err() {
                                info!("Failed to send update, client disconnected: {}", user_address);
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            warn!("WebSocket lagging for user: {}", user_address);
                        }
                    }
                }
            }
        }

        // Cleanup
        {
            let mut subscribers = self.subscribers.write().await;
            subscribers.remove(&user_address);
        }
        info!("WebSocket disconnected for user: {}", user_address);
    }

    async fn send_initial_data(
        &self,
        user_address: &str,
        chains: &[u64],
        tx: &broadcast::Sender<PortfolioUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Send full portfolio
        match self.portfolio_manager.get_portfolio(user_address).await {
            Ok(portfolio) => {
                let update = PortfolioUpdate {
                    user_address: user_address.to_string(),
                    update_type: UpdateType::FullPortfolio,
                    timestamp: now,
                    data: UpdateData::Portfolio(portfolio),
                };
                let _ = tx.send(update);
            }
            Err(e) => {
                warn!("Failed to get initial portfolio: {}", e);
                let update = PortfolioUpdate {
                    user_address: user_address.to_string(),
                    update_type: UpdateType::Error,
                    timestamp: now,
                    data: UpdateData::Error(format!("Failed to get portfolio: {}", e)),
                };
                let _ = tx.send(update);
            }
        }

        // Send chain-specific balances
        for &chain_id in chains {
            match self.portfolio_manager.get_chain_balance_detailed(user_address, chain_id).await {
                Ok(chain_balance) => {
                    let update = PortfolioUpdate {
                        user_address: user_address.to_string(),
                        update_type: UpdateType::ChainBalance,
                        timestamp: now,
                        data: UpdateData::ChainBalance(chain_balance),
                    };
                    let _ = tx.send(update);
                }
                Err(e) => {
                    warn!("Failed to get chain balance for chain {}: {}", chain_id, e);
                }
            }
        }

        // Send portfolio summary
        match self.portfolio_manager.get_portfolio_summary(user_address).await {
            Ok(summary) => {
                let update = PortfolioUpdate {
                    user_address: user_address.to_string(),
                    update_type: UpdateType::Summary,
                    timestamp: now,
                    data: UpdateData::Summary(summary),
                };
                let _ = tx.send(update);
            }
            Err(e) => {
                warn!("Failed to get portfolio summary: {}", e);
            }
        }

        Ok(())
    }

    async fn periodic_updates(
        &self,
        user_address: String,
        chains: Vec<u64>,
        tx: broadcast::Sender<PortfolioUpdate>,
        update_interval: Duration,
    ) {
        let mut interval = interval(update_interval);
        
        loop {
            interval.tick().await;
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Check for price updates
            if let Err(e) = self.check_price_updates(&user_address, &tx, now).await {
                warn!("Failed to check price updates: {}", e);
            }

            // Update portfolio summary
            match self.portfolio_manager.get_portfolio_summary(&user_address).await {
                Ok(summary) => {
                    let update = PortfolioUpdate {
                        user_address: user_address.clone(),
                        update_type: UpdateType::Summary,
                        timestamp: now,
                        data: UpdateData::Summary(summary),
                    };
                    let _ = tx.send(update);
                }
                Err(e) => {
                    warn!("Failed to get updated portfolio summary: {}", e);
                }
            }

            // Update chain balances (less frequently)
            if now % 120 == 0 { // Every 2 minutes
                for &chain_id in &chains {
                    match self.portfolio_manager.get_chain_balance_detailed(&user_address, chain_id).await {
                        Ok(chain_balance) => {
                            let update = PortfolioUpdate {
                                user_address: user_address.clone(),
                                update_type: UpdateType::ChainBalance,
                                timestamp: now,
                                data: UpdateData::ChainBalance(chain_balance),
                            };
                            let _ = tx.send(update);
                        }
                        Err(e) => {
                            warn!("Failed to get updated chain balance for chain {}: {}", chain_id, e);
                        }
                    }
                }
            }
        }
    }

    async fn check_price_updates(
        &self,
        user_address: &str,
        tx: &broadcast::Sender<PortfolioUpdate>,
        timestamp: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get current portfolio to check token prices
        let portfolio = self.portfolio_manager.get_portfolio(user_address).await?;
        let mut price_cache = self.price_cache.write().await;
        
        for balance in &portfolio.balances {
            let cache_key = format!("{}_{}", balance.chain_id, balance.token_symbol);
            let current_price = balance.balance_usd / balance.balance.parse::<f64>().unwrap_or(1.0);
            
            if let Some(&old_price) = price_cache.get(&cache_key) {
                let change_percentage = ((current_price - old_price) / old_price) * 100.0;
                
                // Send update if price change is significant (>1%)
                if change_percentage.abs() > 1.0 {
                    let update = PortfolioUpdate {
                        user_address: user_address.to_string(),
                        update_type: UpdateType::PriceUpdate,
                        timestamp,
                        data: UpdateData::PriceUpdate(PriceUpdateData {
                            token_symbol: balance.token_symbol.clone(),
                            old_price,
                            new_price: current_price,
                            change_percentage,
                        }),
                    };
                    let _ = tx.send(update);
                }
            }
            
            price_cache.insert(cache_key, current_price);
        }
        
        Ok(())
    }

    async fn handle_client_message(
        &self,
        message: &str,
        user_address: &str,
        tx: &broadcast::Sender<PortfolioUpdate>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[derive(Deserialize)]
        struct ClientMessage {
            action: String,
            data: Option<serde_json::Value>,
        }

        let client_msg: ClientMessage = serde_json::from_str(message)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        match client_msg.action.as_str() {
            "refresh_portfolio" => {
                match self.portfolio_manager.get_portfolio(user_address).await {
                    Ok(portfolio) => {
                        let update = PortfolioUpdate {
                            user_address: user_address.to_string(),
                            update_type: UpdateType::FullPortfolio,
                            timestamp: now,
                            data: UpdateData::Portfolio(portfolio),
                        };
                        let _ = tx.send(update);
                    }
                    Err(e) => {
                        let update = PortfolioUpdate {
                            user_address: user_address.to_string(),
                            update_type: UpdateType::Error,
                            timestamp: now,
                            data: UpdateData::Error(format!("Failed to refresh portfolio: {}", e)),
                        };
                        let _ = tx.send(update);
                    }
                }
            }
            "refresh_chain" => {
                if let Some(data) = client_msg.data {
                    if let Ok(chain_id) = data["chain_id"].as_u64().ok_or("Invalid chain_id") {
                        match self.portfolio_manager.get_chain_balance_detailed(user_address, chain_id).await {
                            Ok(chain_balance) => {
                                let update = PortfolioUpdate {
                                    user_address: user_address.to_string(),
                                    update_type: UpdateType::ChainBalance,
                                    timestamp: now,
                                    data: UpdateData::ChainBalance(chain_balance),
                                };
                                let _ = tx.send(update);
                            }
                            Err(e) => {
                                let update = PortfolioUpdate {
                                    user_address: user_address.to_string(),
                                    update_type: UpdateType::Error,
                                    timestamp: now,
                                    data: UpdateData::Error(format!("Failed to refresh chain {}: {}", chain_id, e)),
                                };
                                let _ = tx.send(update);
                            }
                        }
                    }
                }
            }
            _ => {
                warn!("Unknown client action: {}", client_msg.action);
            }
        }

        Ok(())
    }

    pub async fn broadcast_to_user(
        &self,
        user_address: &str,
        update: PortfolioUpdate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let subscribers = self.subscribers.read().await;
        if let Some(tx) = subscribers.get(user_address) {
            tx.send(update)?;
        }
        Ok(())
    }

    pub async fn get_active_connections(&self) -> usize {
        self.subscribers.read().await.len()
    }
}

impl Clone for PortfolioWebSocketManager {
    fn clone(&self) -> Self {
        Self {
            portfolio_manager: Arc::clone(&self.portfolio_manager),
            subscribers: Arc::clone(&self.subscribers),
            price_cache: Arc::clone(&self.price_cache),
        }
    }
}
