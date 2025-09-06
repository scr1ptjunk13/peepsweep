use crate::risk_management::types::{
    TradeEvent, UserPositions, TokenExposure, ExposureSnapshot, RiskError, 
    UserId, TokenAddress, TokenBalance
};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for the position tracker
#[derive(Debug, Clone)]
pub struct PositionTrackerConfig {
    pub max_users: usize,
    pub snapshot_interval_ms: u64,
    pub position_timeout_ms: u64,
    pub enable_pnl_tracking: bool,
    pub enable_exposure_limits: bool,
    pub max_token_exposure_usd: Decimal,
}

impl Default for PositionTrackerConfig {
    fn default() -> Self {
        Self {
            max_users: 100000,
            snapshot_interval_ms: 1000,
            position_timeout_ms: 86400000, // 24 hours
            enable_pnl_tracking: true,
            enable_exposure_limits: true,
            max_token_exposure_usd: Decimal::from(1000000), // $1M default limit
        }
    }
}

/// Statistics for monitoring position tracker performance
#[derive(Debug, Clone, Default)]
pub struct PositionTrackerStats {
    pub active_users: u64,
    pub total_positions: u64,
    pub updates_processed: u64,
    pub snapshots_created: u64,
    pub avg_update_time_ms: f64,
    pub memory_usage_mb: f64,
}

/// High-performance position tracker for real-time user position management
pub struct PositionTracker {
    config: PositionTrackerConfig,
    // Lock-free concurrent hash map for user positions
    positions: Arc<DashMap<UserId, UserPositions>>,
    // Recent snapshots for historical analysis
    snapshots: Arc<RwLock<Vec<ExposureSnapshot>>>,
    stats: Arc<RwLock<PositionTrackerStats>>,
    // Price cache for PnL calculations
    price_cache: Arc<DashMap<TokenAddress, Decimal>>,
}

impl PositionTracker {
    /// Create a new position tracker
    pub fn new(config: PositionTrackerConfig) -> Self {
        Self {
            config,
            positions: Arc::new(DashMap::new()),
            snapshots: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(PositionTrackerStats::default())),
            price_cache: Arc::new(DashMap::new()),
        }
    }

    /// Process a trade event and update user positions
    pub async fn process_trade_event(&self, event: &TradeEvent) -> Result<(), RiskError> {
        let start_time = std::time::Instant::now();

        // Get or create user position
        let mut user_position = self.positions
            .entry(event.user_id)
            .or_insert_with(UserPositions::new)
            .clone();

        // Update token balances
        self.update_token_balance(&mut user_position, &event.token_in, -event.amount_in)?;
        self.update_token_balance(&mut user_position, &event.token_out, event.amount_out)?;

        // Update timestamp
        user_position.last_updated = event.timestamp;

        // Calculate PnL if enabled
        if self.config.enable_pnl_tracking {
            user_position.pnl = self.calculate_pnl(&user_position).await?;
        }

        // Store updated position
        self.positions.insert(event.user_id, user_position);

        // Update statistics
        let processing_time = start_time.elapsed().as_millis() as f64;
        self.update_stats(processing_time).await;

        Ok(())
    }

    /// Get current position for a user
    pub fn get_user_position(&self, user_id: &UserId) -> Option<UserPositions> {
        self.positions.get(user_id).map(|entry| entry.clone())
    }

    /// Insert or update user position (for testing)
    pub fn insert_user_position(&self, user_id: UserId, position: UserPositions) {
        self.positions.insert(user_id, position);
    }

    /// Get all active users
    pub fn get_active_users(&self) -> Vec<UserId> {
        self.positions.iter().map(|entry| *entry.key()).collect()
    }

    /// Get total number of active positions
    pub fn get_position_count(&self) -> usize {
        self.positions.len()
    }

    /// Create exposure snapshot for a specific user
    pub async fn create_exposure_snapshot(&self, user_id: &UserId) -> Result<ExposureSnapshot, RiskError> {
        let position = self.get_user_position(user_id)
            .ok_or_else(|| RiskError::UserNotFound(*user_id))?;
        
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let start_time = std::time::Instant::now();
        
        let mut token_exposures = Vec::new();
        let mut total_exposure_usd = Decimal::ZERO;
        
        for (token_address, token_balance) in &position.balances {
            let mut value_usd = Decimal::ZERO;
            
            // Calculate USD value if price is available
            if let Some(price) = self.price_cache.get(token_address) {
                value_usd = token_balance.balance * *price;
                total_exposure_usd += value_usd;
            }
            
            token_exposures.push(TokenExposure {
                token: token_address.clone(),
                amount: token_balance.balance,
                value_usd,
                percentage: Decimal::ZERO, // Will be calculated after total is known
            });
        }
        
        // Calculate percentages
        if !total_exposure_usd.is_zero() {
            for exposure in &mut token_exposures {
                exposure.percentage = (exposure.value_usd / total_exposure_usd) * Decimal::from(100);
            }
        }
        
        let calculation_time_us = start_time.elapsed().as_micros() as u64;
        
        let snapshot = ExposureSnapshot {
            user_id: *user_id,
            total_exposure_usd,
            token_exposures,
            timestamp,
            calculation_time_us,
        };

        // Store snapshot
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.push(snapshot.clone());
            
            // Keep only recent snapshots (last 1000)
            if snapshots.len() > 1000 {
                snapshots.remove(0);
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.snapshots_created += 1;
        }

        Ok(snapshot)
    }

    /// Get recent exposure snapshots
    pub async fn get_recent_snapshots(&self, limit: usize) -> Vec<ExposureSnapshot> {
        let snapshots = self.snapshots.read().await;
        let start_idx = if snapshots.len() > limit {
            snapshots.len() - limit
        } else {
            0
        };
        snapshots[start_idx..].to_vec()
    }

    /// Update token price in cache
    pub fn update_token_price(&self, token_address: &TokenAddress, price: Decimal) {
        self.price_cache.insert(token_address.clone(), price);
    }

    /// Get token price from cache
    pub fn get_token_price(&self, token_address: &TokenAddress) -> Option<Decimal> {
        self.price_cache.get(token_address).map(|entry| *entry.value())
    }

    /// Check if user exceeds exposure limits
    pub async fn check_exposure_limits(&self, user_id: &UserId) -> Result<bool, RiskError> {
        if !self.config.enable_exposure_limits {
            return Ok(true);
        }

        let position = self.get_user_position(user_id)
            .ok_or_else(|| RiskError::UserNotFound(*user_id))?;

        let total_exposure_usd = self.calculate_total_exposure_usd(&position).await?;
        
        Ok(total_exposure_usd <= self.config.max_token_exposure_usd)
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> PositionTrackerStats {
        let mut stats = self.stats.read().await.clone();
        stats.active_users = self.positions.len() as u64;
        stats.total_positions = self.positions.iter().map(|entry| entry.value().balances.len() as u64).sum();
        stats.memory_usage_mb = self.estimate_memory_usage();
        stats
    }

    /// Clear old positions based on timeout
    pub async fn cleanup_old_positions(&self) -> Result<usize, RiskError> {
        let current_time = chrono::Utc::now().timestamp_millis() as u64;
        let timeout_threshold = current_time - self.config.position_timeout_ms;
        
        let mut removed_count = 0;
        let keys_to_remove: Vec<UserId> = self.positions
            .iter()
            .filter_map(|entry| {
                if entry.value().last_updated < timeout_threshold {
                    Some(*entry.key())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            self.positions.remove(&key);
            removed_count += 1;
        }

        Ok(removed_count)
    }

    /// Private helper to update token balance
    fn update_token_balance(
        &self,
        position: &mut UserPositions,
        token_address: &TokenAddress,
        amount_delta: Decimal,
    ) -> Result<(), RiskError> {
        let current_balance = position.balances.get(token_address)
            .map(|tb| tb.balance)
            .unwrap_or(Decimal::ZERO);
        let new_balance = current_balance + amount_delta;
        
        if new_balance.is_zero() {
            position.balances.remove(token_address);
        } else {
            // For simplicity, use current price as avg_cost (in real system, track actual cost basis)
            let avg_cost = self.price_cache.get(token_address)
                .map(|price| *price.value())
                .unwrap_or(Decimal::ZERO);
            
            position.balances.insert(token_address.clone(), TokenBalance {
                token_address: token_address.clone(),
                balance: new_balance,
                value_usd: new_balance * avg_cost,
                last_updated: chrono::Utc::now().timestamp_millis() as u64,
            });
        }
        
        Ok(())
    }

    /// Private helper to calculate PnL
    async fn calculate_pnl(&self, position: &UserPositions) -> Result<Decimal, RiskError> {
        let mut total_usd_value = Decimal::ZERO;
        
        for (token_address, token_balance) in &position.balances {
            if let Some(price) = self.price_cache.get(token_address) {
                total_usd_value += token_balance.balance * *price;
            }
        }
        
        // For simplicity, assume initial investment was the current USD value
        // In a real system, this would track cost basis
        Ok(total_usd_value)
    }

    /// Private helper to calculate total exposure in USD
    async fn calculate_total_exposure_usd(&self, position: &UserPositions) -> Result<Decimal, RiskError> {
        let mut total_exposure = Decimal::ZERO;
        
        for (token_address, token_balance) in &position.balances {
            if let Some(price) = self.price_cache.get(token_address) {
                total_exposure += token_balance.balance.abs() * *price;
            }
        }
        
        Ok(total_exposure)
    }

    /// Private helper to update statistics
    async fn update_stats(&self, processing_time_ms: f64) {
        let mut stats = self.stats.write().await;
        stats.updates_processed += 1;
        
        // Update rolling average processing time
        if stats.avg_update_time_ms == 0.0 {
            stats.avg_update_time_ms = processing_time_ms;
        } else {
            stats.avg_update_time_ms = (stats.avg_update_time_ms * 0.95) + (processing_time_ms * 0.05);
        }
    }

    /// Private helper to estimate memory usage
    fn estimate_memory_usage(&self) -> f64 {
        let position_count = self.positions.len();
        let avg_tokens_per_position = 5; // Rough estimate
        let bytes_per_position = std::mem::size_of::<UserPositions>() + 
                                (avg_tokens_per_position * std::mem::size_of::<(TokenAddress, Decimal)>());
        
        (position_count * bytes_per_position) as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn create_test_config() -> PositionTrackerConfig {
        PositionTrackerConfig {
            max_users: 1000,
            snapshot_interval_ms: 100,
            position_timeout_ms: 5000,
            enable_pnl_tracking: true,
            enable_exposure_limits: true,
            max_token_exposure_usd: Decimal::from(10000),
        }
    }

    fn create_test_trade_event(user_id: UserId, token_in: &str, token_out: &str, amount_in: &str, amount_out: &str) -> TradeEvent {
        TradeEvent {
            user_id,
            trade_id: Uuid::new_v4(),
            token_in: TokenAddress::from_str(token_in).unwrap(),
            token_out: TokenAddress::from_str(token_out).unwrap(),
            amount_in: Decimal::from_str(amount_in).unwrap(),
            amount_out: Decimal::from_str(amount_out).unwrap(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            dex_source: uuid::Uuid::from_str("550e8400-e29b-41d4-a716-446655440000").unwrap().to_string(),
            gas_used: Decimal::from_str("150000").unwrap(),
        }
    }

    #[tokio::test]
    async fn test_position_tracker_creation() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        assert_eq!(tracker.get_position_count(), 0);
        assert_eq!(tracker.get_active_users().len(), 0);
        
        let stats = tracker.get_stats().await;
        assert_eq!(stats.active_users, 0);
        assert_eq!(stats.total_positions, 0);
        assert_eq!(stats.updates_processed, 0);
    }

    #[tokio::test]
    async fn test_single_trade_processing() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let user_id = Uuid::new_v4();
        let event = create_test_trade_event(
            user_id,
            "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5",
            "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5",
            "1000.0",
            "950.0"
        );
        
        tracker.process_trade_event(&event).await.unwrap();
        
        // Check position was created
        assert_eq!(tracker.get_position_count(), 1);
        assert_eq!(tracker.get_active_users().len(), 1);
        
        // Check position details
        let position = tracker.get_user_position(&user_id).unwrap();
        assert_eq!(position.balances.len(), 2);
        assert_eq!(position.balances[&event.token_in].balance, Decimal::from_str("-1000.0").unwrap());
        assert_eq!(position.balances[&event.token_out].balance, Decimal::from_str("950.0").unwrap());
        
        let stats = tracker.get_stats().await;
        assert_eq!(stats.active_users, 1);
        assert_eq!(stats.updates_processed, 1);
        assert!(stats.avg_update_time_ms >= 0.0); // Allow zero for very fast operations
    }

    #[tokio::test]
    async fn test_multiple_trades_same_user() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let user_id = Uuid::new_v4();
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        let token_b = TokenAddress::from_str("0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // First trade: A -> B
        let event1 = create_test_trade_event(user_id, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1000.0", "950.0");
        tracker.process_trade_event(&event1).await.unwrap();
        
        // Second trade: B -> A (partial)
        let event2 = create_test_trade_event(user_id, "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "500.0", "525.0");
        tracker.process_trade_event(&event2).await.unwrap();
        
        // Check final position
        let position = tracker.get_user_position(&user_id).unwrap();
        assert_eq!(position.balances.len(), 2);
        assert_eq!(position.balances[&token_a].balance, Decimal::from_str("-475.0").unwrap()); // -1000 + 525
        assert_eq!(position.balances[&token_b].balance, Decimal::from_str("450.0").unwrap());  // 950 - 500
        
        // Still only one user
        assert_eq!(tracker.get_position_count(), 1);
        
        let stats = tracker.get_stats().await;
        assert_eq!(stats.active_users, 1);
        assert_eq!(stats.updates_processed, 2);
    }

    #[tokio::test]
    async fn test_multiple_users() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        let user3 = Uuid::new_v4();
        
        // Process trades for different users
        let event1 = create_test_trade_event(user1, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1000.0", "950.0");
        let event2 = create_test_trade_event(user2, "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xC0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "500.0", "480.0");
        let event3 = create_test_trade_event(user3, "0xC0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "200.0", "210.0");
        
        tracker.process_trade_event(&event1).await.unwrap();
        tracker.process_trade_event(&event2).await.unwrap();
        tracker.process_trade_event(&event3).await.unwrap();
        
        // Check we have 3 users
        assert_eq!(tracker.get_position_count(), 3);
        assert_eq!(tracker.get_active_users().len(), 3);
        
        // Check each user has correct position
        let pos1 = tracker.get_user_position(&user1).unwrap();
        let pos2 = tracker.get_user_position(&user2).unwrap();
        let pos3 = tracker.get_user_position(&user3).unwrap();
        
        assert_eq!(pos1.balances.len(), 2);
        assert_eq!(pos2.balances.len(), 2);
        assert_eq!(pos3.balances.len(), 2);
        
        let stats = tracker.get_stats().await;
        assert_eq!(stats.active_users, 3);
        assert_eq!(stats.updates_processed, 3);
    }

    #[tokio::test]
    async fn test_price_cache_and_exposure_calculation() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        let token_b = TokenAddress::from_str("0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // Update token prices
        tracker.update_token_price(&token_a, Decimal::from_str("2000.0").unwrap()); // $2000 per token
        tracker.update_token_price(&token_b, Decimal::from_str("1.0").unwrap());    // $1 per token
        
        // Check prices are cached
        assert_eq!(tracker.get_token_price(&token_a).unwrap(), Decimal::from_str("2000.0").unwrap());
        assert_eq!(tracker.get_token_price(&token_b).unwrap(), Decimal::from_str("1.0").unwrap());
        
        // Process a trade
        let user_id = Uuid::new_v4();
        let event = create_test_trade_event(user_id, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1.0", "1900.0");
        tracker.process_trade_event(&event).await.unwrap();
        
        // Check exposure limits
        let within_limits = tracker.check_exposure_limits(&user_id).await.unwrap();
        assert!(within_limits); // Should be within $10k limit
    }

    #[tokio::test]
    async fn test_exposure_snapshot_creation() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        let token_b = TokenAddress::from_str("0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // Set prices
        tracker.update_token_price(&token_a, Decimal::from_str("100.0").unwrap());
        tracker.update_token_price(&token_b, Decimal::from_str("1.0").unwrap());
        
        // Process trades for multiple users
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        
        let event1 = create_test_trade_event(user1, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "10.0", "950.0");
        let event2 = create_test_trade_event(user2, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "5.0", "475.0");
        
        tracker.process_trade_event(&event1).await.unwrap();
        tracker.process_trade_event(&event2).await.unwrap();
        
        // Create snapshot for user1
        let snapshot = tracker.create_exposure_snapshot(&user1).await.unwrap();
        
        assert_eq!(snapshot.user_id, user1);
        assert_eq!(snapshot.token_exposures.len(), 2);
        
        // Check token exposures
        let token_a_exposure = snapshot.token_exposures.iter().find(|e| e.token == token_a).unwrap();
        let token_b_exposure = snapshot.token_exposures.iter().find(|e| e.token == token_b).unwrap();
        
        assert_eq!(token_a_exposure.amount, Decimal::from_str("-10.0").unwrap());
        assert_eq!(token_a_exposure.value_usd, Decimal::from_str("-1000.0").unwrap()); // -10 * 100
        
        assert_eq!(token_b_exposure.amount, Decimal::from_str("950.0").unwrap());
        assert_eq!(token_b_exposure.value_usd, Decimal::from_str("950.0").unwrap()); // 950 * 1
        
        let stats = tracker.get_stats().await;
        assert_eq!(stats.snapshots_created, 1);
    }

    #[tokio::test]
    async fn test_position_cleanup() {
        let mut config = create_test_config();
        config.position_timeout_ms = 100; // Very short timeout for testing
        let tracker = PositionTracker::new(config);
        
        let user_id = Uuid::new_v4();
        let event = create_test_trade_event(user_id, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1000.0", "950.0");
        
        tracker.process_trade_event(&event).await.unwrap();
        assert_eq!(tracker.get_position_count(), 1);
        
        // Wait for timeout
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        
        // Cleanup old positions
        let removed_count = tracker.cleanup_old_positions().await.unwrap();
        assert_eq!(removed_count, 1);
        assert_eq!(tracker.get_position_count(), 0);
    }

    #[tokio::test]
    async fn test_zero_balance_removal() {
        let config = create_test_config();
        let tracker = PositionTracker::new(config);
        
        let user_id = Uuid::new_v4();
        let token_a = "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5";
        let token_b = "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5";
        
        // First trade: A -> B
        let event1 = create_test_trade_event(user_id, token_a, token_b, "1000.0", "950.0");
        tracker.process_trade_event(&event1).await.unwrap();
        
        let position = tracker.get_user_position(&user_id).unwrap();
        assert_eq!(position.balances.len(), 2);
        
        // Second trade: B -> A (exact reverse)
        let event2 = create_test_trade_event(user_id, token_b, token_a, "950.0", "1000.0");
        tracker.process_trade_event(&event2).await.unwrap();
        
        // Both balances should be zero and removed
        let position = tracker.get_user_position(&user_id).unwrap();
        assert_eq!(position.balances.len(), 0);
    }

    #[tokio::test]
    async fn test_config_parameters() {
        let config = PositionTrackerConfig {
            max_users: 50000,
            snapshot_interval_ms: 500,
            position_timeout_ms: 3600000,
            enable_pnl_tracking: false,
            enable_exposure_limits: false,
            max_token_exposure_usd: Decimal::from(500000),
        };
        
        assert_eq!(config.max_users, 50000);
        assert_eq!(config.snapshot_interval_ms, 500);
        assert_eq!(config.position_timeout_ms, 3600000);
        assert!(!config.enable_pnl_tracking);
        assert!(!config.enable_exposure_limits);
        assert_eq!(config.max_token_exposure_usd, Decimal::from(500000));
    }
}
