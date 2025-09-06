use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use crate::types::*;
use crate::user_retention::arbitrage_alerts::detector::{ArbitrageOpportunity, ArbitrageDetector};
use crate::risk_management::RiskError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSubscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub preferences: AlertPreferences,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertPreferences {
    pub min_profit_threshold: Decimal, // Minimum profit percentage
    pub max_gas_cost_percentage: Decimal, // Max gas cost as % of profit
    pub min_liquidity_usd: Decimal,
    pub min_confidence_score: f64,
    pub enabled_chains: Vec<u64>,
    pub enabled_dexes: Vec<String>,
    pub monitored_tokens: Vec<String>,
    pub notification_channels: Vec<NotificationChannel>,
    pub alert_frequency: AlertFrequency,
    pub priority_filter: Vec<AlertPriority>,
    pub max_alerts_per_hour: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    WebSocket,
    Email,
    PushNotification,
    Webhook { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertFrequency {
    Immediate,
    Batched { interval_minutes: u32 },
    DailyDigest { hour: u8 }, // 0-23
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub user_id: Uuid,
    pub opportunity: ArbitrageOpportunity,
    pub priority: AlertPriority,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
    pub status: AlertStatus,
    pub delivery_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertStatus {
    Pending,
    Sent,
    Failed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertHistory {
    pub user_id: Uuid,
    pub alerts: Vec<Alert>,
    pub total_sent: u64,
    pub total_failed: u64,
    pub last_alert_at: Option<DateTime<Utc>>,
}

impl Default for AlertPreferences {
    fn default() -> Self {
        Self {
            min_profit_threshold: Decimal::from_str("0.02").unwrap(), // 2%
            max_gas_cost_percentage: Decimal::from_str("0.30").unwrap(), // 30%
            min_liquidity_usd: Decimal::from_str("10000").unwrap(), // $10k
            min_confidence_score: 0.7,
            enabled_chains: vec![1, 137, 42161, 10], // Ethereum, Polygon, Arbitrum, Optimism
            enabled_dexes: vec![
                "Uniswap".to_string(),
                "Curve".to_string(),
                "Balancer".to_string(),
                "Paraswap".to_string(),
            ],
            monitored_tokens: vec![
                "ETH".to_string(),
                "WETH".to_string(),
                "USDC".to_string(),
                "USDT".to_string(),
                "DAI".to_string(),
                "WBTC".to_string(),
            ],
            notification_channels: vec![NotificationChannel::WebSocket],
            alert_frequency: AlertFrequency::Immediate,
            priority_filter: vec![AlertPriority::High, AlertPriority::Medium],
            max_alerts_per_hour: 10,
        }
    }
}

pub struct AlertManager {
    subscriptions: Arc<RwLock<HashMap<Uuid, AlertSubscription>>>,
    pending_alerts: Arc<RwLock<Vec<Alert>>>,
    alert_history: Arc<RwLock<HashMap<Uuid, AlertHistory>>>,
    arbitrage_detector: Arc<ArbitrageDetector>,
    rate_limits: Arc<RwLock<HashMap<Uuid, Vec<DateTime<Utc>>>>>, // user_id -> alert timestamps
}

impl AlertManager {
    pub fn new(arbitrage_detector: Arc<ArbitrageDetector>) -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            pending_alerts: Arc::new(RwLock::new(Vec::new())),
            alert_history: Arc::new(RwLock::new(HashMap::new())),
            arbitrage_detector,
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_alert_processing(&self) -> Result<(), RiskError> {
        let manager = Arc::new(self.clone());
        
        // Start opportunity monitoring
        let manager_clone1 = Arc::clone(&manager);
        tokio::spawn(async move {
            manager_clone1.opportunity_monitoring_loop().await;
        });

        // Start alert delivery
        let manager_clone2 = Arc::clone(&manager);
        tokio::spawn(async move {
            manager_clone2.alert_delivery_loop().await;
        });

        // Start cleanup task
        let manager_clone3 = Arc::clone(&manager);
        tokio::spawn(async move {
            manager_clone3.cleanup_task().await;
        });
        
        Ok(())
    }

    pub async fn subscribe_user(&self, user_id: Uuid, preferences: AlertPreferences) -> Result<Uuid, RiskError> {
        let subscription = AlertSubscription {
            id: Uuid::new_v4(),
            user_id,
            preferences,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(user_id, subscription.clone());

        // Initialize alert history
        let mut history = self.alert_history.write().await;
        history.insert(user_id, AlertHistory {
            user_id,
            alerts: Vec::new(),
            total_sent: 0,
            total_failed: 0,
            last_alert_at: None,
        });

        tracing::info!("User {} subscribed to arbitrage alerts", user_id);
        Ok(subscription.id)
    }

    pub async fn unsubscribe_user(&self, user_id: Uuid) -> Result<(), RiskError> {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(mut subscription) = subscriptions.get_mut(&user_id) {
            subscription.is_active = false;
            subscription.updated_at = Utc::now();
            tracing::info!("User {} unsubscribed from arbitrage alerts", user_id);
            Ok(())
        } else {
            Err(RiskError::NotFound("User subscription not found".to_string()))
        }
    }

    pub async fn update_preferences(&self, user_id: Uuid, preferences: AlertPreferences) -> Result<(), RiskError> {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(subscription) = subscriptions.get_mut(&user_id) {
            subscription.preferences = preferences;
            subscription.updated_at = Utc::now();
            tracing::info!("Updated alert preferences for user {}", user_id);
            Ok(())
        } else {
            Err(RiskError::NotFound("User subscription not found".to_string()))
        }
    }

    pub async fn get_user_preferences(&self, user_id: Uuid) -> Option<AlertPreferences> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(&user_id).map(|sub| sub.preferences.clone())
    }

    pub async fn get_user_alert_history(&self, user_id: Uuid) -> Option<AlertHistory> {
        let history = self.alert_history.read().await;
        history.get(&user_id).cloned()
    }

    pub async fn get_pending_alerts(&self, user_id: Option<Uuid>) -> Vec<Alert> {
        let alerts = self.pending_alerts.read().await;
        match user_id {
            Some(uid) => alerts.iter().filter(|alert| alert.user_id == uid).cloned().collect(),
            None => alerts.clone(),
        }
    }

    async fn opportunity_monitoring_loop(&self) {
        let mut last_check = Utc::now();
        
        loop {
            // Get new opportunities since last check
            let opportunities = self.arbitrage_detector.get_opportunities().await;
            let new_opportunities: Vec<_> = opportunities
                .into_iter()
                .filter(|op| op.detected_at > last_check)
                .collect();

            if !new_opportunities.is_empty() {
                tracing::info!("Processing {} new arbitrage opportunities", new_opportunities.len());
                
                for opportunity in new_opportunities {
                    self.process_opportunity(opportunity).await;
                }
            }

            last_check = Utc::now();
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    // Public method for testing
    pub async fn process_opportunity_for_test(&self, opportunity: ArbitrageOpportunity) {
        self.process_opportunity(opportunity).await;
    }

    // Public method for testing - get pending alerts for a user
    pub async fn get_pending_alerts_for_user(&self, user_id: Uuid) -> Vec<Alert> {
        let pending_alerts = self.pending_alerts.read().await;
        pending_alerts.iter()
            .filter(|alert| alert.user_id == user_id)
            .cloned()
            .collect()
    }

    // Public method for testing - process pending alerts immediately
    pub async fn process_pending_alerts_for_test(&self) {
        let mut pending_alerts = self.pending_alerts.write().await;
        let alerts_to_process: Vec<Alert> = pending_alerts.drain(..).collect();
        drop(pending_alerts);

        for alert in alerts_to_process {
            self.deliver_alert(alert).await;
        }
    }

    async fn process_opportunity(&self, opportunity: ArbitrageOpportunity) {
        let subscriptions = self.subscriptions.read().await;
        
        println!("🔍 Processing opportunity with profit: {}%", opportunity.profit_percentage);
        println!("🔍 Found {} active subscriptions", subscriptions.len());
        
        for (user_id, subscription) in subscriptions.iter() {
            println!("🔍 Checking user {}: active={}", user_id, subscription.is_active);
            
            if !subscription.is_active {
                continue;
            }

            let should_alert = self.should_alert_user(&subscription.preferences, &opportunity).await;
            println!("🔍 Should alert user {}: {}", user_id, should_alert);
            
            if should_alert {
                let rate_limit_ok = self.check_rate_limit(*user_id).await;
                println!("🔍 Rate limit OK for user {}: {}", user_id, rate_limit_ok);
                
                if rate_limit_ok {
                    let priority = self.calculate_alert_priority(&opportunity);
                    println!("🔍 Alert priority: {:?}", priority);
                    
                    let priority_ok = subscription.preferences.priority_filter.contains(&priority);
                    println!("🔍 Priority filter OK for user {}: {}", user_id, priority_ok);
                    
                    if priority_ok {
                        let alert = Alert {
                            id: Uuid::new_v4(),
                            user_id: *user_id,
                            opportunity: opportunity.clone(),
                            priority,
                            created_at: Utc::now(),
                            sent_at: None,
                            status: AlertStatus::Pending,
                            delivery_attempts: 0,
                        };

                        let mut pending_alerts = self.pending_alerts.write().await;
                        pending_alerts.push(alert);
                        println!("✅ Alert created for user {}", user_id);
                    }
                }
            }
        }
    }

    async fn should_alert_user(&self, preferences: &AlertPreferences, opportunity: &ArbitrageOpportunity) -> bool {
        println!("🔍 Checking alert criteria:");
        println!("  - Opportunity profit: {}%", opportunity.profit_percentage);
        println!("  - User min threshold: {}%", preferences.min_profit_threshold);
        
        // Check profit threshold
        if opportunity.profit_percentage < preferences.min_profit_threshold {
            println!("  ❌ Profit threshold not met");
            return false;
        }
        println!("  ✅ Profit threshold met");

        // Check gas cost percentage
        let gas_percentage = opportunity.estimated_gas_cost / opportunity.estimated_profit_usd;
        println!("  - Gas percentage: {}%", gas_percentage * Decimal::from(100));
        println!("  - Max gas threshold: {}%", preferences.max_gas_cost_percentage * Decimal::from(100));
        
        if gas_percentage > preferences.max_gas_cost_percentage {
            println!("  ❌ Gas cost too high");
            return false;
        }
        println!("  ✅ Gas cost acceptable");

        // Check liquidity
        println!("  - Liquidity available: ${}", opportunity.liquidity_available);
        println!("  - Min liquidity required: ${}", preferences.min_liquidity_usd);
        if opportunity.liquidity_available < preferences.min_liquidity_usd {
            println!("  ❌ Insufficient liquidity");
            return false;
        }
        println!("  ✅ Liquidity sufficient");

        // Check confidence score
        println!("  - Confidence score: {}", opportunity.confidence_score);
        println!("  - Min confidence required: {}", preferences.min_confidence_score);
        if opportunity.confidence_score < preferences.min_confidence_score {
            println!("  ❌ Confidence score too low");
            return false;
        }
        println!("  ✅ Confidence score acceptable");

        // Check enabled chains
        println!("  - Chain ID: {}", opportunity.chain_id);
        println!("  - Enabled chains: {:?}", preferences.enabled_chains);
        if !preferences.enabled_chains.contains(&opportunity.chain_id) {
            println!("  ❌ Chain not enabled");
            return false;
        }
        println!("  ✅ Chain enabled");

        // Check enabled DEXes
        println!("  - Source DEX: {}", opportunity.source_dex);
        println!("  - Target DEX: {}", opportunity.target_dex);
        println!("  - Enabled DEXes: {:?}", preferences.enabled_dexes);
        if !preferences.enabled_dexes.contains(&opportunity.source_dex) ||
           !preferences.enabled_dexes.contains(&opportunity.target_dex) {
            println!("  ❌ DEX not enabled");
            return false;
        }
        println!("  ✅ DEXes enabled");

        // Check monitored tokens
        println!("  - Base token: {}", opportunity.token_pair.base_token);
        println!("  - Quote token: {}", opportunity.token_pair.quote_token);
        println!("  - Monitored tokens: {:?}", preferences.monitored_tokens);
        if !preferences.monitored_tokens.contains(&opportunity.token_pair.base_token) ||
           !preferences.monitored_tokens.contains(&opportunity.token_pair.quote_token) {
            println!("  ❌ Token not monitored");
            return false;
        }
        println!("  ✅ Tokens monitored");

        println!("  ✅ All criteria passed");
        true
    }

    async fn check_rate_limit(&self, user_id: Uuid) -> bool {
        let mut rate_limits = self.rate_limits.write().await;
        let now = Utc::now();
        let one_hour_ago = now - chrono::Duration::hours(1);

        let user_alerts = rate_limits.entry(user_id).or_insert_with(Vec::new);
        
        // Remove alerts older than 1 hour
        user_alerts.retain(|&timestamp| timestamp > one_hour_ago);

        // Get user's max alerts per hour
        let subscriptions = self.subscriptions.read().await;
        let max_alerts = subscriptions
            .get(&user_id)
            .map(|sub| sub.preferences.max_alerts_per_hour)
            .unwrap_or(10);
        drop(subscriptions);

        if user_alerts.len() < max_alerts as usize {
            user_alerts.push(now);
            true
        } else {
            false
        }
    }

    fn calculate_alert_priority(&self, opportunity: &ArbitrageOpportunity) -> AlertPriority {
        let profit_percentage = f64::try_from(opportunity.profit_percentage).unwrap_or(0.0);
        
        if profit_percentage >= 0.05 {
            AlertPriority::High
        } else if profit_percentage >= 0.02 {
            AlertPriority::Medium
        } else {
            AlertPriority::Low
        }
    }

    async fn alert_delivery_loop(&self) {
        loop {
            let mut pending_alerts = self.pending_alerts.write().await;
            let alerts_to_process: Vec<Alert> = pending_alerts
                .iter()
                .filter(|alert| alert.status == AlertStatus::Pending)
                .cloned()
                .collect();
            drop(pending_alerts);

            for alert in alerts_to_process {
                self.deliver_alert(alert).await;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }

    async fn deliver_alert(&self, mut alert: Alert) {
        let subscriptions = self.subscriptions.read().await;
        if let Some(subscription) = subscriptions.get(&alert.user_id) {
            let channels = &subscription.preferences.notification_channels;
            
            let mut delivery_success = false;
            
            for channel in channels {
                match self.send_notification(channel, &alert).await {
                    Ok(_) => {
                        delivery_success = true;
                        tracing::info!("Alert {} delivered to user {} via {:?}", 
                                     alert.id, alert.user_id, channel);
                    }
                    Err(e) => {
                        tracing::error!("Failed to deliver alert {} via {:?}: {:?}", 
                                      alert.id, channel, e);
                    }
                }
            }

            alert.delivery_attempts += 1;
            alert.sent_at = Some(Utc::now());
            alert.status = if delivery_success {
                AlertStatus::Sent
            } else if alert.delivery_attempts >= 3 {
                AlertStatus::Failed
            } else {
                AlertStatus::Pending
            };

            // Update alert in pending list
            let mut pending_alerts = self.pending_alerts.write().await;
            if let Some(pos) = pending_alerts.iter().position(|a| a.id == alert.id) {
                pending_alerts[pos] = alert.clone();
            }

            // Add to history
            let mut history = self.alert_history.write().await;
            if let Some(user_history) = history.get_mut(&alert.user_id) {
                user_history.alerts.push(alert.clone());
                user_history.last_alert_at = Some(Utc::now());
                
                if alert.status == AlertStatus::Sent {
                    user_history.total_sent += 1;
                } else if alert.status == AlertStatus::Failed {
                    user_history.total_failed += 1;
                }

                // Keep only last 100 alerts per user
                if user_history.alerts.len() > 100 {
                    user_history.alerts.drain(0..user_history.alerts.len() - 100);
                }
            }
        }
    }

    async fn send_notification(&self, channel: &NotificationChannel, alert: &Alert) -> Result<(), RiskError> {
        match channel {
            NotificationChannel::WebSocket => {
                // WebSocket notification will be handled by the notification service
                tracing::info!("WebSocket notification queued for alert {}", alert.id);
                Ok(())
            }
            NotificationChannel::Email => {
                // Email notification will be handled by the notification service
                tracing::info!("Email notification queued for alert {}", alert.id);
                Ok(())
            }
            NotificationChannel::PushNotification => {
                // Push notification will be handled by the notification service
                tracing::info!("Push notification queued for alert {}", alert.id);
                Ok(())
            }
            NotificationChannel::Webhook { url } => {
                // Webhook notification
                tracing::info!("Webhook notification to {} queued for alert {}", url, alert.id);
                Ok(())
            }
        }
    }

    async fn cleanup_loop(&self) {
        loop {
            // Clean up expired alerts
            let mut pending_alerts = self.pending_alerts.write().await;
            let now = Utc::now();
            
            for alert in pending_alerts.iter_mut() {
                if alert.opportunity.expires_at < now && alert.status == AlertStatus::Pending {
                    alert.status = AlertStatus::Expired;
                }
            }

            // Remove old expired alerts
            let one_hour_ago = now - chrono::Duration::hours(1);
            pending_alerts.retain(|alert| {
                !(alert.status == AlertStatus::Expired && alert.created_at < one_hour_ago)
            });

            drop(pending_alerts);

            // Clean up old rate limit data
            let mut rate_limits = self.rate_limits.write().await;
            let one_hour_ago = now - chrono::Duration::hours(1);
            
            for user_alerts in rate_limits.values_mut() {
                user_alerts.retain(|&timestamp| timestamp > one_hour_ago);
            }

            drop(rate_limits);

            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
        }
    }

    pub async fn cleanup_task(&self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
        
        loop {
            interval.tick().await;
            
            // Clean up expired alerts and rate limiting data
            let now = Utc::now();
            
            // Clean up expired rate limiting entries
            {
                let mut rate_limits = self.rate_limits.write().await;
                rate_limits.retain(|_, timestamps| {
                    timestamps.retain(|&timestamp| {
                        now.signed_duration_since(timestamp) < chrono::Duration::hours(1)
                    });
                    !timestamps.is_empty()
                });
            }
            
            tracing::debug!("Cleanup task completed at {}", now);
        }
    }
}

impl Clone for AlertManager {
    fn clone(&self) -> Self {
        Self {
            subscriptions: Arc::clone(&self.subscriptions),
            pending_alerts: Arc::clone(&self.pending_alerts),
            alert_history: Arc::clone(&self.alert_history),
            arbitrage_detector: Arc::clone(&self.arbitrage_detector),
            rate_limits: Arc::clone(&self.rate_limits),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregator::DEXAggregator;
    use crate::user_retention::ArbitrageConfig;

    #[tokio::test]
    async fn test_alert_manager_creation() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let detector = Arc::new(ArbitrageDetector::new(mock_aggregator, ArbitrageConfig::default()));
        let manager = AlertManager::new(detector);
        
        let pending = manager.get_pending_alerts(None).await;
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_user_subscription() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let detector = Arc::new(ArbitrageDetector::new(mock_aggregator, ArbitrageConfig::default()));
        let manager = AlertManager::new(detector);
        
        let user_id = Uuid::new_v4();
        let preferences = AlertPreferences::default();
        
        let subscription_id = manager.subscribe_user(user_id, preferences.clone()).await.unwrap();
        assert_ne!(subscription_id, Uuid::nil());
        
        let retrieved_prefs = manager.get_user_preferences(user_id).await;
        assert!(retrieved_prefs.is_some());
        assert_eq!(retrieved_prefs.unwrap().min_profit_threshold, preferences.min_profit_threshold);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let detector = Arc::new(ArbitrageDetector::new(mock_aggregator, ArbitrageConfig::default()));
        let manager = AlertManager::new(detector);
        
        let user_id = Uuid::new_v4();
        
        // First alert should pass
        assert!(manager.check_rate_limit(user_id).await);
        
        // Simulate hitting rate limit
        for _ in 0..10 {
            manager.check_rate_limit(user_id).await;
        }
        
        // Should be rate limited now
        assert!(!manager.check_rate_limit(user_id).await);
    }
}
