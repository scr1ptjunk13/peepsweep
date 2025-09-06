use crate::risk_management::position_tracker::PositionTracker;
use crate::risk_management::types::{RiskError, TokenBalance};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Real-time P&L calculation engine
pub struct LivePnLEngine {
    position_tracker: Arc<PositionTracker>,
    price_feed: Arc<dyn PriceFeedInterface>,
    pnl_cache: Arc<RwLock<HashMap<Uuid, PnLSnapshot>>>,
    calculation_config: PnLCalculationConfig,
    event_sender: broadcast::Sender<PnLUpdateEvent>,
    calculation_stats: Arc<RwLock<PnLCalculationStats>>,
}

/// Price feed interface for real-time price data
#[async_trait::async_trait]
pub trait PriceFeedInterface: Send + Sync {
    async fn get_current_price(&self, token_address: &str, chain_id: u64) -> Result<Decimal, RiskError>;
    async fn get_historical_price(&self, token_address: &str, chain_id: u64, timestamp: DateTime<Utc>) -> Result<Decimal, RiskError>;
    async fn subscribe_to_price_updates(&self, token_address: &str, chain_id: u64) -> Result<broadcast::Receiver<PriceUpdate>, RiskError>;
}

/// P&L snapshot for a user at a specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSnapshot {
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub positions: Vec<PositionPnL>,
    pub total_unrealized_pnl_usd: Decimal,
    pub total_realized_pnl_usd: Decimal,
    pub total_pnl_usd: Decimal,
    pub total_portfolio_value_usd: Decimal,
    pub daily_change_usd: Decimal,
    pub daily_change_percent: Decimal,
    pub calculation_duration_ms: u64,
}

/// P&L data for individual position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionPnL {
    pub token_address: String,
    pub chain_id: u64,
    pub symbol: String,
    pub balance: Decimal,
    pub entry_price_usd: Decimal,
    pub current_price_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub total_pnl_usd: Decimal,
    pub position_value_usd: Decimal,
    pub price_change_24h_percent: Decimal,
    pub last_updated: DateTime<Utc>,
}

/// P&L update event for real-time streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLUpdateEvent {
    pub event_type: PnLEventType,
    pub user_id: Uuid,
    pub snapshot: PnLSnapshot,
    pub changed_positions: Vec<String>, // Token addresses that changed
    pub timestamp: DateTime<Utc>,
}

/// P&L event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PnLEventType {
    FullUpdate,
    PositionChange,
    PriceUpdate,
    NewPosition,
    ClosedPosition,
}

/// Price update from feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub token_address: String,
    pub chain_id: u64,
    pub price_usd: Decimal,
    pub timestamp: DateTime<Utc>,
    pub volume_24h: Option<Decimal>,
    pub price_change_24h: Option<Decimal>,
}

/// P&L calculation configuration
#[derive(Debug, Clone)]
pub struct PnLCalculationConfig {
    pub update_interval_ms: u64,
    pub price_staleness_threshold_ms: u64,
    pub min_position_value_usd: Decimal,
    pub enable_realized_pnl_tracking: bool,
    pub enable_historical_comparison: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for PnLCalculationConfig {
    fn default() -> Self {
        Self {
            update_interval_ms: 5000, // 5 seconds
            price_staleness_threshold_ms: 30000, // 30 seconds
            min_position_value_usd: Decimal::new(1, 2), // $0.01
            enable_realized_pnl_tracking: true,
            enable_historical_comparison: true,
            cache_ttl_seconds: 60, // 1 minute
        }
    }
}

/// P&L calculation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PnLCalculationStats {
    pub total_calculations: u64,
    pub successful_calculations: u64,
    pub failed_calculations: u64,
    pub average_calculation_time_ms: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub price_feed_errors: u64,
    pub position_tracker_errors: u64,
    pub last_calculation_time: Option<DateTime<Utc>>,
}

impl LivePnLEngine {
    /// Create new live P&L engine
    pub async fn new(
        position_tracker: Arc<PositionTracker>,
        price_feed: Arc<dyn PriceFeedInterface>,
        config: PnLCalculationConfig,
    ) -> Result<Self, RiskError> {
        let (event_sender, _) = broadcast::channel(10000);
        
        Ok(Self {
            position_tracker,
            price_feed,
            pnl_cache: Arc::new(RwLock::new(HashMap::new())),
            calculation_config: config,
            event_sender,
            calculation_stats: Arc::new(RwLock::new(PnLCalculationStats::default())),
        })
    }

    /// Calculate real-time P&L for a user
    pub async fn calculate_user_pnl(&self, user_id: Uuid) -> Result<PnLSnapshot, RiskError> {
        let start_time = std::time::Instant::now();
        
        // Update stats
        {
            let mut stats = self.calculation_stats.write().await;
            stats.total_calculations += 1;
            stats.last_calculation_time = Some(Utc::now());
        }

        // Check cache first
        if let Some(cached_snapshot) = self.get_cached_pnl(user_id).await? {
            let cache_age = Utc::now().signed_duration_since(cached_snapshot.timestamp);
            if cache_age.num_seconds() < self.calculation_config.cache_ttl_seconds as i64 {
                let mut stats = self.calculation_stats.write().await;
                stats.cache_hits += 1;
                return Ok(cached_snapshot);
            }
        }

        // Cache miss - calculate fresh P&L
        {
            let mut stats = self.calculation_stats.write().await;
            stats.cache_misses += 1;
        }

        // Get user positions
        let positions_opt = self.position_tracker.get_user_position(&user_id);

        let positions = positions_opt.unwrap_or_else(|| crate::risk_management::types::UserPositions {
            balances: HashMap::new(),
            pnl: Decimal::ZERO,
            last_updated: 0,
        });
        let mut position_pnls = Vec::new();
        let mut total_unrealized_pnl = Decimal::ZERO;
        let mut total_realized_pnl = Decimal::ZERO;
        let mut total_portfolio_value = Decimal::ZERO;

        // Calculate P&L for each position
        for (_, position) in &positions.balances {
            match self.calculate_position_pnl(&position).await {
                Ok(position_pnl) => {
                    total_unrealized_pnl += position_pnl.unrealized_pnl_usd;
                    total_realized_pnl += position_pnl.realized_pnl_usd;
                    total_portfolio_value += position_pnl.position_value_usd;
                    position_pnls.push(position_pnl);
                }
                Err(e) => {
                    warn!("Failed to calculate P&L for position {}: {}", position.token_address, e);
                    let mut stats = self.calculation_stats.write().await;
                    stats.price_feed_errors += 1;
                }
            }
        }

        // Calculate daily change (simplified - would need historical data in production)
        let daily_change_usd = total_unrealized_pnl * Decimal::new(1, 2); // Placeholder: 1% of unrealized P&L
        let daily_change_percent = if total_portfolio_value > Decimal::ZERO {
            (daily_change_usd / total_portfolio_value) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        };

        let calculation_duration = start_time.elapsed().as_millis() as u64;
        
        let snapshot = PnLSnapshot {
            user_id,
            timestamp: Utc::now(),
            positions: position_pnls,
            total_unrealized_pnl_usd: total_unrealized_pnl,
            total_realized_pnl_usd: total_realized_pnl,
            total_pnl_usd: total_unrealized_pnl + total_realized_pnl,
            total_portfolio_value_usd: total_portfolio_value,
            daily_change_usd,
            daily_change_percent,
            calculation_duration_ms: calculation_duration,
        };

        // Cache the result
        self.cache_pnl_snapshot(snapshot.clone()).await?;

        // Update stats
        {
            let mut stats = self.calculation_stats.write().await;
            stats.successful_calculations += 1;
            stats.average_calculation_time_ms = 
                (stats.average_calculation_time_ms * (stats.successful_calculations - 1) as f64 + calculation_duration as f64) 
                / stats.successful_calculations as f64;
        }

        // Send update event
        let update_event = PnLUpdateEvent {
            event_type: PnLEventType::FullUpdate,
            user_id,
            snapshot: snapshot.clone(),
            changed_positions: snapshot.positions.iter().map(|p| p.token_address.clone()).collect(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_sender.send(update_event) {
            warn!("Failed to send P&L update event: {}", e);
        }

        info!("Calculated P&L for user {} in {}ms: total_pnl=${}, portfolio_value=${}", 
              user_id, calculation_duration, snapshot.total_pnl_usd, snapshot.total_portfolio_value_usd);

        Ok(snapshot)
    }

    /// Calculate P&L for individual position
    pub async fn calculate_position_pnl(&self, position: &TokenBalance) -> Result<PositionPnL, RiskError> {
        // Get current price
        let current_price = self.price_feed.get_current_price(&position.token_address, 1).await?; // Default to Ethereum mainnet
        
        // Calculate entry price (simplified - would need trade history in production)
        let entry_price = self.estimate_entry_price(&position.token_address, 1).await?; // Default to Ethereum mainnet
        
        // Calculate position value
        let position_value_usd = position.balance * current_price;
        
        // Calculate unrealized P&L
        let unrealized_pnl_usd = position.balance * (current_price - entry_price);
        
        // Get realized P&L (would come from trade history in production)
        let realized_pnl_usd = Decimal::ZERO; // Placeholder
        
        // Calculate 24h price change
        let yesterday = Utc::now() - chrono::Duration::hours(24);
        let historical_price = self.price_feed.get_historical_price(&position.token_address, 1, yesterday).await
            .unwrap_or(current_price);
        
        let price_change_24h_percent = if historical_price > Decimal::ZERO {
            ((current_price - historical_price) / historical_price) * Decimal::new(100, 0)
        } else {
            Decimal::ZERO
        };

        let position_pnl = PositionPnL {
            token_address: position.token_address.clone(),
            chain_id: 1, // Default to Ethereum mainnet
            symbol: "UNKNOWN".to_string(), // Simplified - would need token registry in production
            balance: position.balance,
            entry_price_usd: entry_price,
            current_price_usd: current_price,
            unrealized_pnl_usd,
            realized_pnl_usd,
            total_pnl_usd: unrealized_pnl_usd + realized_pnl_usd,
            position_value_usd,
            price_change_24h_percent,
            last_updated: Utc::now(),
        };

        Ok(position_pnl)
    }

    /// Estimate entry price for a position (simplified implementation)
    async fn estimate_entry_price(&self, token_address: &str, chain_id: u64) -> Result<Decimal, RiskError> {
        // In production, this would calculate weighted average entry price from trade history
        // For now, we'll use common price estimates based on token type
        
        let current_price = self.price_feed.get_current_price(token_address, chain_id).await?;
        
        // Simple heuristic based on token address patterns
        let estimated_entry_price = if token_address.to_lowercase().contains("eth") {
            Decimal::new(3200, 0) // $3200 for ETH
        } else if token_address.to_lowercase().contains("btc") || token_address.to_lowercase().contains("wbtc") {
            Decimal::new(65000, 0) // $65000 for BTC
        } else if token_address.to_lowercase().contains("usdc") || 
                  token_address.to_lowercase().contains("usdt") || 
                  token_address.to_lowercase().contains("dai") {
            Decimal::new(1, 0) // $1 for stablecoins
        } else {
            // For other tokens, assume entry was at 95% of current price
            current_price * Decimal::new(95, 2)
        };

        Ok(estimated_entry_price)
    }

    /// Get cached P&L snapshot
    async fn get_cached_pnl(&self, user_id: Uuid) -> Result<Option<PnLSnapshot>, RiskError> {
        let cache = self.pnl_cache.read().await;
        Ok(cache.get(&user_id).cloned())
    }

    /// Cache P&L snapshot
    async fn cache_pnl_snapshot(&self, snapshot: PnLSnapshot) -> Result<(), RiskError> {
        let mut cache = self.pnl_cache.write().await;
        cache.insert(snapshot.user_id, snapshot);
        Ok(())
    }

    /// Subscribe to P&L updates for a user
    pub fn subscribe_to_pnl_updates(&self) -> broadcast::Receiver<PnLUpdateEvent> {
        self.event_sender.subscribe()
    }

    /// Start real-time P&L calculation loop
    pub async fn start_real_time_updates(&self, user_ids: Vec<Uuid>) -> Result<(), RiskError> {
        let update_interval = std::time::Duration::from_millis(self.calculation_config.update_interval_ms);
        
        info!("Starting real-time P&L updates for {} users with {}ms interval", 
              user_ids.len(), self.calculation_config.update_interval_ms);

        tokio::spawn({
            let engine = self.clone();
            async move {
                let mut interval = tokio::time::interval(update_interval);
                
                loop {
                    interval.tick().await;
                    
                    for user_id in &user_ids {
                        if let Err(e) = engine.calculate_user_pnl(*user_id).await {
                            error!("Failed to calculate P&L for user {}: {}", user_id, e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Get calculation statistics
    pub async fn get_calculation_stats(&self) -> PnLCalculationStats {
        self.calculation_stats.read().await.clone()
    }

    /// Clear P&L cache
    pub async fn clear_cache(&self) -> Result<(), RiskError> {
        let mut cache = self.pnl_cache.write().await;
        cache.clear();
        info!("P&L cache cleared");
        Ok(())
    }
}

// Clone implementation for LivePnLEngine (needed for tokio::spawn)
impl Clone for LivePnLEngine {
    fn clone(&self) -> Self {
        Self {
            position_tracker: Arc::clone(&self.position_tracker),
            price_feed: Arc::clone(&self.price_feed),
            pnl_cache: Arc::clone(&self.pnl_cache),
            calculation_config: self.calculation_config.clone(),
            event_sender: self.event_sender.clone(),
            calculation_stats: Arc::clone(&self.calculation_stats),
        }
    }
}
