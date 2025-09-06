use crate::risk_management::types::{
    TradeEvent, UserPositions, RiskMetrics, RiskAlert, AlertSeverity, RiskError, 
    UserId, TokenAddress
};
use crate::risk_management::position_tracker::PositionTracker;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Configuration for the risk processing engine
#[derive(Debug, Clone)]
pub struct RiskEngineConfig {
    pub max_position_size_usd: Decimal,
    pub max_concentration_pct: Decimal,
    pub var_confidence_level: Decimal,
    pub max_drawdown_pct: Decimal,
    pub min_sharpe_ratio: Decimal,
    pub enable_real_time_monitoring: bool,
    pub risk_calculation_interval_ms: u64,
}

impl Default for RiskEngineConfig {
    fn default() -> Self {
        Self {
            max_position_size_usd: Decimal::from(1000000), // $1M max position
            max_concentration_pct: Decimal::from(25), // 25% max concentration
            var_confidence_level: Decimal::from(95), // 95% VaR
            max_drawdown_pct: Decimal::from(20), // 20% max drawdown
            min_sharpe_ratio: Decimal::from(1), // Minimum Sharpe ratio of 1.0
            enable_real_time_monitoring: true,
            risk_calculation_interval_ms: 1000, // 1 second
        }
    }
}

/// Statistics for monitoring risk engine performance
#[derive(Debug, Clone, Default)]
pub struct RiskEngineStats {
    pub risk_calculations_performed: u64,
    pub alerts_generated: u64,
    pub avg_calculation_time_ms: f64,
    pub users_monitored: u64,
    pub high_risk_users: u64,
}

/// High-performance risk processing engine for real-time risk analysis
pub struct RiskProcessingEngine {
    config: RiskEngineConfig,
    position_tracker: Arc<PositionTracker>,
    risk_cache: Arc<RwLock<HashMap<UserId, RiskMetrics>>>,
    alert_history: Arc<RwLock<Vec<RiskAlert>>>,
    stats: Arc<RwLock<RiskEngineStats>>,
    price_history: Arc<RwLock<HashMap<TokenAddress, Vec<(u64, Decimal)>>>>, // (timestamp, price)
}

impl RiskProcessingEngine {
    /// Create a new risk processing engine
    pub fn new(config: RiskEngineConfig, position_tracker: Arc<PositionTracker>) -> Self {
        Self {
            config,
            position_tracker,
            risk_cache: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(RiskEngineStats::default())),
            price_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get access to the position tracker
    pub fn get_position_tracker(&self) -> &Arc<PositionTracker> {
        &self.position_tracker
    }

    /// Process a trade event and update risk metrics
    pub async fn process_trade_event(&self, event: &TradeEvent) -> Result<Vec<RiskAlert>, RiskError> {
        let start_time = std::time::Instant::now();
        let mut alerts = Vec::new();

        // Calculate updated risk metrics for the user
        let risk_metrics = self.calculate_user_risk_metrics(&event.user_id).await?;
        
        // Store in cache
        {
            let mut cache = self.risk_cache.write().await;
            cache.insert(event.user_id, risk_metrics.clone());
        }

        // Check for risk threshold violations
        alerts.extend(self.check_risk_thresholds(&event.user_id, &risk_metrics).await?);

        // Update statistics
        let processing_time = start_time.elapsed().as_millis() as f64;
        self.update_stats(processing_time, alerts.len()).await;

        // Store alerts
        if !alerts.is_empty() {
            let mut alert_history = self.alert_history.write().await;
            alert_history.extend(alerts.clone());
            
            // Keep only recent alerts (last 10000)
            if alert_history.len() > 10000 {
                let len = alert_history.len();
                alert_history.drain(0..len - 10000);
            }
        }

        Ok(alerts)
    }

    /// Calculate comprehensive risk metrics for a user
    pub async fn calculate_user_risk_metrics(&self, user_id: &UserId) -> Result<RiskMetrics, RiskError> {
        let position = self.position_tracker.get_user_position(user_id)
            .ok_or_else(|| RiskError::UserNotFound(*user_id))?;

        let mut total_exposure_usd = Decimal::ZERO;
        let mut token_exposures = HashMap::new();
        
        // Calculate exposures and total portfolio value
        for (token_address, token_balance) in &position.balances {
            if let Some(price) = self.position_tracker.get_token_price(token_address) {
                let exposure_usd = token_balance.balance.abs() * price;
                total_exposure_usd += exposure_usd;
                token_exposures.insert(token_address.clone(), exposure_usd);
            }
        }

        // Calculate concentration risk (largest position as % of total)
        let concentration_risk = if !total_exposure_usd.is_zero() {
            token_exposures.values()
                .max()
                .copied()
                .unwrap_or(Decimal::ZERO) / total_exposure_usd * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        // Calculate VaR (simplified historical simulation)
        let var_95 = self.calculate_value_at_risk(&position, &self.config.var_confidence_level).await?;

        // Calculate max drawdown (simplified)
        let max_drawdown = self.calculate_max_drawdown(&position).await?;

        // Calculate Sharpe ratio (simplified)
        let sharpe_ratio = self.calculate_sharpe_ratio(&position).await?;

        // Calculate win rate and average trade size (simplified)
        let (win_rate, avg_trade_size) = self.calculate_trading_metrics(user_id).await?;

        Ok(RiskMetrics {
            total_exposure_usd,
            concentration_risk,
            var_95,
            max_drawdown,
            sharpe_ratio,
            win_rate,
            avg_trade_size,
        })
    }

    /// Get cached risk metrics for a user
    pub async fn get_user_risk_metrics(&self, user_id: &UserId) -> Option<RiskMetrics> {
        let cache = self.risk_cache.read().await;
        cache.get(user_id).cloned()
    }

    /// Get all users with high risk
    pub async fn get_high_risk_users(&self) -> Vec<(UserId, RiskMetrics)> {
        let cache = self.risk_cache.read().await;
        cache.iter()
            .filter(|(_, metrics)| self.is_high_risk(metrics))
            .map(|(user_id, metrics)| (*user_id, metrics.clone()))
            .collect()
    }

    /// Update price history for VaR calculations
    pub async fn update_price_history(&self, token_address: &TokenAddress, price: Decimal) {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        let mut price_history = self.price_history.write().await;
        
        let history = price_history.entry(token_address.clone()).or_insert_with(Vec::new);
        history.push((timestamp, price));
        
        // Keep only last 100 price points for each token
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// Get recent risk alerts
    pub async fn get_recent_alerts(&self, limit: usize) -> Vec<RiskAlert> {
        let alerts = self.alert_history.read().await;
        let start_idx = if alerts.len() > limit {
            alerts.len() - limit
        } else {
            0
        };
        alerts[start_idx..].to_vec()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> RiskEngineStats {
        let mut stats = self.stats.read().await.clone();
        stats.users_monitored = self.risk_cache.read().await.len() as u64;
        stats.high_risk_users = self.get_high_risk_users().await.len() as u64;
        stats
    }

    /// Clear old risk data
    pub async fn cleanup_old_data(&self) -> Result<usize, RiskError> {
        let current_time = chrono::Utc::now().timestamp_millis() as u64;
        let cleanup_threshold = current_time - (24 * 60 * 60 * 1000); // 24 hours ago
        
        let mut removed_count = 0;
        
        // Clean up price history
        {
            let mut price_history = self.price_history.write().await;
            for (_, history) in price_history.iter_mut() {
                let original_len = history.len();
                history.retain(|(timestamp, _)| *timestamp > cleanup_threshold);
                removed_count += original_len - history.len();
            }
        }
        
        Ok(removed_count)
    }

    /// Private helper to check risk thresholds and generate alerts
    async fn check_risk_thresholds(&self, user_id: &UserId, metrics: &RiskMetrics) -> Result<Vec<RiskAlert>, RiskError> {
        let mut alerts = Vec::new();
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;

        // Check position size limit
        if metrics.total_exposure_usd > self.config.max_position_size_usd {
            alerts.push(RiskAlert {
                alert_id: Uuid::new_v4().to_string(),
                user_id: *user_id,
                rule_name: "position_size_limit".to_string(),
                severity: AlertSeverity::High,
                message: format!(
                    "Position size ${:.2} exceeds limit of ${:.2}",
                    metrics.total_exposure_usd,
                    self.config.max_position_size_usd
                ),
                timestamp,
                trade_id: None,
            });
        }

        // Check concentration risk
        if metrics.concentration_risk > self.config.max_concentration_pct {
            alerts.push(RiskAlert {
                alert_id: Uuid::new_v4().to_string(),
                user_id: *user_id,
                rule_name: "concentration_risk_limit".to_string(),
                severity: AlertSeverity::Medium,
                message: format!(
                    "Concentration risk {:.1}% exceeds limit of {:.1}%",
                    metrics.concentration_risk,
                    self.config.max_concentration_pct
                ),
                timestamp,
                trade_id: None,
            });
        }

        Ok(alerts)
    }

    /// Private helper to calculate Value at Risk
    async fn calculate_value_at_risk(&self, _position: &UserPositions, _confidence_level: &Decimal) -> Result<Decimal, RiskError> {
        // Simplified VaR calculation - in real implementation would use historical simulation
        Ok(Decimal::from(5000)) // Placeholder $5000 VaR
    }

    /// Private helper to calculate maximum drawdown
    async fn calculate_max_drawdown(&self, _position: &UserPositions) -> Result<Decimal, RiskError> {
        // Simplified drawdown calculation
        Ok(Decimal::from(10)) // Placeholder 10% drawdown
    }

    /// Private helper to calculate Sharpe ratio
    async fn calculate_sharpe_ratio(&self, _position: &UserPositions) -> Result<Decimal, RiskError> {
        // Simplified Sharpe ratio calculation
        Ok(Decimal::from_str("1.5").unwrap()) // Placeholder Sharpe ratio of 1.5
    }

    /// Private helper to calculate trading metrics
    async fn calculate_trading_metrics(&self, _user_id: &UserId) -> Result<(Decimal, Decimal), RiskError> {
        // Simplified trading metrics
        Ok((Decimal::from(65), Decimal::from(2500))) // 65% win rate, $2500 avg trade
    }

    /// Private helper to determine if metrics indicate high risk
    fn is_high_risk(&self, metrics: &RiskMetrics) -> bool {
        metrics.total_exposure_usd > self.config.max_position_size_usd ||
        metrics.concentration_risk > self.config.max_concentration_pct ||
        metrics.max_drawdown > self.config.max_drawdown_pct ||
        metrics.sharpe_ratio < self.config.min_sharpe_ratio
    }

    /// Private helper to update statistics
    async fn update_stats(&self, processing_time_ms: f64, alerts_count: usize) {
        let mut stats = self.stats.write().await;
        stats.risk_calculations_performed += 1;
        stats.alerts_generated += alerts_count as u64;
        
        // Update rolling average processing time
        if stats.avg_calculation_time_ms == 0.0 {
            stats.avg_calculation_time_ms = processing_time_ms;
        } else {
            stats.avg_calculation_time_ms = (stats.avg_calculation_time_ms * 0.9) + (processing_time_ms * 0.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::position_tracker::{PositionTracker, PositionTrackerConfig};
    use std::str::FromStr;

    fn create_test_config() -> RiskEngineConfig {
        RiskEngineConfig {
            max_position_size_usd: Decimal::from(10000),
            max_concentration_pct: Decimal::from(30),
            var_confidence_level: Decimal::from(95),
            max_drawdown_pct: Decimal::from(15),
            min_sharpe_ratio: Decimal::from(1),
            enable_real_time_monitoring: true,
            risk_calculation_interval_ms: 100,
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
    async fn test_risk_engine_creation() {
        let config = create_test_config();
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let engine = RiskProcessingEngine::new(config, position_tracker);
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.risk_calculations_performed, 0);
        assert_eq!(stats.alerts_generated, 0);
        assert_eq!(stats.users_monitored, 0);
        assert_eq!(stats.high_risk_users, 0);
    }

    #[tokio::test]
    async fn test_risk_calculation_with_position() {
        let config = create_test_config();
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let engine = RiskProcessingEngine::new(config, position_tracker.clone());
        
        let user_id = Uuid::new_v4();
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // Set up prices
        position_tracker.update_token_price(&token_a, Decimal::from_str("2000.0").unwrap());
        
        // Process a trade to create position
        let event = create_test_trade_event(user_id, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1.0", "1900.0");
        position_tracker.process_trade_event(&event).await.unwrap();
        
        // Calculate risk metrics
        let metrics = engine.calculate_user_risk_metrics(&user_id).await.unwrap();
        
        assert!(metrics.total_exposure_usd > Decimal::ZERO);
        assert!(metrics.concentration_risk >= Decimal::ZERO);
        assert_eq!(metrics.var_95, Decimal::from(5000)); // Placeholder value
        assert_eq!(metrics.max_drawdown, Decimal::from(10)); // Placeholder value
        assert_eq!(metrics.sharpe_ratio, Decimal::from_str("1.5").unwrap()); // Placeholder value
    }

    #[tokio::test]
    async fn test_risk_alert_generation() {
        let mut config = create_test_config();
        config.max_position_size_usd = Decimal::from(1000); // Low limit to trigger alert
        
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let engine = RiskProcessingEngine::new(config, position_tracker.clone());
        
        let user_id = Uuid::new_v4();
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // Set up high price to trigger position size alert
        position_tracker.update_token_price(&token_a, Decimal::from_str("2000.0").unwrap());
        
        // Process a large trade
        let event = create_test_trade_event(user_id, "0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "0xB0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5", "1.0", "1900.0");
        position_tracker.process_trade_event(&event).await.unwrap();
        
        // Process trade through risk engine
        let alerts = engine.process_trade_event(&event).await.unwrap();
        
        // Should generate position size alert
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|alert| alert.severity == AlertSeverity::High));
        
        let stats = engine.get_stats().await;
        assert_eq!(stats.risk_calculations_performed, 1);
        assert!(stats.alerts_generated > 0);
    }

    #[tokio::test]
    async fn test_price_history_tracking() {
        let config = create_test_config();
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let engine = RiskProcessingEngine::new(config, position_tracker);
        
        let token_a = TokenAddress::from_str("0xA0b86a33E6441e6e80D0c2c3C5C0C5e5E5E5E5E5").unwrap();
        
        // Update price history
        engine.update_price_history(&token_a, Decimal::from_str("1000.0").unwrap()).await;
        engine.update_price_history(&token_a, Decimal::from_str("1050.0").unwrap()).await;
        engine.update_price_history(&token_a, Decimal::from_str("980.0").unwrap()).await;
        
        // Check price history is stored
        let price_history = engine.price_history.read().await;
        let history = price_history.get(&token_a).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].1, Decimal::from_str("1000.0").unwrap());
        assert_eq!(history[1].1, Decimal::from_str("1050.0").unwrap());
        assert_eq!(history[2].1, Decimal::from_str("980.0").unwrap());
    }

    #[tokio::test]
    async fn test_config_parameters() {
        let config = RiskEngineConfig {
            max_position_size_usd: Decimal::from(500000),
            max_concentration_pct: Decimal::from(40),
            var_confidence_level: Decimal::from(99),
            max_drawdown_pct: Decimal::from(25),
            min_sharpe_ratio: Decimal::from(2),
            enable_real_time_monitoring: false,
            risk_calculation_interval_ms: 5000,
        };
        
        assert_eq!(config.max_position_size_usd, Decimal::from(500000));
        assert_eq!(config.max_concentration_pct, Decimal::from(40));
        assert_eq!(config.var_confidence_level, Decimal::from(99));
        assert_eq!(config.max_drawdown_pct, Decimal::from(25));
        assert_eq!(config.min_sharpe_ratio, Decimal::from(2));
        assert!(!config.enable_real_time_monitoring);
        assert_eq!(config.risk_calculation_interval_ms, 5000);
    }
}
