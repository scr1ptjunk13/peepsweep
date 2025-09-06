use crate::risk_management::types::{RiskAlert, AlertSeverity, RiskError, UserId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use uuid::Uuid;

/// Configuration for the alert system
#[derive(Debug, Clone)]
pub struct AlertSystemConfig {
    pub max_pending_alerts: usize,
    pub alert_batch_size: usize,
    pub delivery_timeout_ms: u64,
    pub retry_attempts: u32,
    pub enable_dashboard_alerts: bool,
    pub enable_email_alerts: bool,
    pub enable_webhook_alerts: bool,
}

impl Default for AlertSystemConfig {
    fn default() -> Self {
        Self {
            max_pending_alerts: 10000,
            alert_batch_size: 100,
            delivery_timeout_ms: 5000,
            retry_attempts: 3,
            enable_dashboard_alerts: true,
            enable_email_alerts: false,
            enable_webhook_alerts: false,
        }
    }
}

/// Alert delivery status
#[derive(Debug, Clone, PartialEq)]
pub enum AlertDeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}

/// Alert delivery record
#[derive(Debug, Clone)]
pub struct AlertDeliveryRecord {
    pub alert_id: String,
    pub status: AlertDeliveryStatus,
    pub delivery_attempts: u32,
    pub last_attempt_timestamp: u64,
    pub delivery_channels: Vec<String>,
}

/// Alert subscription for users
#[derive(Debug, Clone)]
pub struct AlertSubscription {
    pub user_id: UserId,
    pub severity_filter: Vec<AlertSeverity>,
    pub channels: Vec<String>,
    pub enabled: bool,
}

/// Statistics for the alert system
#[derive(Debug, Clone, Default)]
pub struct AlertSystemStats {
    pub total_alerts_processed: u64,
    pub alerts_delivered: u64,
    pub alerts_failed: u64,
    pub average_delivery_time_ms: f64,
    pub active_subscriptions: usize,
    pub pending_alerts: usize,
}

/// High-performance alert system for real-time risk management
pub struct AlertSystem {
    config: AlertSystemConfig,
    pending_alerts: Arc<RwLock<Vec<RiskAlert>>>,
    delivery_records: Arc<RwLock<HashMap<String, AlertDeliveryRecord>>>,
    subscriptions: Arc<RwLock<HashMap<UserId, AlertSubscription>>>,
    stats: Arc<RwLock<AlertSystemStats>>,
    
    // Channels for real-time alert delivery
    alert_sender: mpsc::UnboundedSender<RiskAlert>,
    dashboard_broadcast: broadcast::Sender<RiskAlert>,
}

impl AlertSystem {
    /// Create a new alert system
    pub fn new(config: AlertSystemConfig) -> Self {
        let (alert_sender, alert_receiver) = mpsc::unbounded_channel();
        let (dashboard_broadcast, _) = broadcast::channel(1000);
        
        let system = Self {
            config,
            pending_alerts: Arc::new(RwLock::new(Vec::new())),
            delivery_records: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(AlertSystemStats::default())),
            alert_sender,
            dashboard_broadcast,
        };
        
        // Start background alert processing
        system.start_alert_processor(alert_receiver);
        
        system
    }
    
    /// Submit an alert for delivery
    pub async fn submit_alert(&self, alert: RiskAlert) -> Result<(), RiskError> {
        // Add to pending alerts
        let mut pending_alerts = self.pending_alerts.write().await;
        
        // Check capacity
        if pending_alerts.len() >= self.config.max_pending_alerts {
            return Err(RiskError::SystemError("Alert queue full".to_string()));
        }
        
        pending_alerts.push(alert.clone());
        
        // Send for immediate processing
        self.alert_sender.send(alert)
            .map_err(|_| RiskError::SystemError("Failed to queue alert".to_string()))?;
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_alerts_processed += 1;
        stats.pending_alerts = pending_alerts.len();
        
        Ok(())
    }
    
    /// Submit multiple alerts in batch
    pub async fn submit_alerts_batch(&self, alerts: Vec<RiskAlert>) -> Result<(), RiskError> {
        let mut pending_alerts = self.pending_alerts.write().await;
        
        // Check capacity
        if pending_alerts.len() + alerts.len() > self.config.max_pending_alerts {
            return Err(RiskError::SystemError("Alert queue would overflow".to_string()));
        }
        
        // Add all alerts to pending
        for alert in &alerts {
            pending_alerts.push(alert.clone());
            self.alert_sender.send(alert.clone())
                .map_err(|_| RiskError::SystemError("Failed to queue alert".to_string()))?;
        }
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_alerts_processed += alerts.len() as u64;
        stats.pending_alerts = pending_alerts.len();
        
        Ok(())
    }
    
    /// Subscribe a user to alerts
    pub async fn subscribe_user(&self, subscription: AlertSubscription) -> Result<(), RiskError> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(subscription.user_id, subscription);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.active_subscriptions = subscriptions.len();
        
        Ok(())
    }
    
    /// Unsubscribe a user from alerts
    pub async fn unsubscribe_user(&self, user_id: &UserId) -> Result<(), RiskError> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(user_id);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.active_subscriptions = subscriptions.len();
        
        Ok(())
    }
    
    /// Get user's alert subscription
    pub async fn get_user_subscription(&self, user_id: &UserId) -> Option<AlertSubscription> {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.get(user_id).cloned()
    }
    
    /// Get pending alerts for a user
    pub async fn get_user_alerts(&self, user_id: &UserId, limit: usize) -> Vec<RiskAlert> {
        let pending_alerts = self.pending_alerts.read().await;
        pending_alerts.iter()
            .filter(|alert| alert.user_id == *user_id)
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get all pending alerts
    pub async fn get_pending_alerts(&self, limit: usize) -> Vec<RiskAlert> {
        let pending_alerts = self.pending_alerts.read().await;
        pending_alerts.iter()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get delivery status for an alert
    pub async fn get_delivery_status(&self, alert_id: &str) -> Option<AlertDeliveryRecord> {
        let delivery_records = self.delivery_records.read().await;
        delivery_records.get(alert_id).cloned()
    }
    
    /// Get dashboard alert receiver
    pub fn get_dashboard_receiver(&self) -> broadcast::Receiver<RiskAlert> {
        self.dashboard_broadcast.subscribe()
    }
    
    /// Get alert system statistics
    pub async fn get_stats(&self) -> AlertSystemStats {
        self.stats.read().await.clone()
    }
    
    /// Clear old alerts and delivery records
    pub async fn cleanup_old_data(&self, max_age_hours: u64) -> Result<(), RiskError> {
        let cutoff_timestamp = chrono::Utc::now().timestamp_millis() as u64 - (max_age_hours * 3600 * 1000);
        
        // Clean up pending alerts
        let mut pending_alerts = self.pending_alerts.write().await;
        pending_alerts.retain(|alert| alert.timestamp > cutoff_timestamp);
        
        // Clean up delivery records
        let mut delivery_records = self.delivery_records.write().await;
        delivery_records.retain(|_, record| record.last_attempt_timestamp > cutoff_timestamp);
        
        // Update stats
        let mut stats = self.stats.write().await;
        stats.pending_alerts = pending_alerts.len();
        
        Ok(())
    }
    
    /// Start background alert processing
    fn start_alert_processor(&self, mut alert_receiver: mpsc::UnboundedReceiver<RiskAlert>) {
        let pending_alerts = self.pending_alerts.clone();
        let delivery_records = self.delivery_records.clone();
        let subscriptions = self.subscriptions.clone();
        let stats = self.stats.clone();
        let dashboard_broadcast = self.dashboard_broadcast.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            while let Some(alert) = alert_receiver.recv().await {
                let start_time = std::time::Instant::now();
                
                // Process the alert
                let delivery_result = Self::process_single_alert(
                    &alert,
                    &subscriptions,
                    &dashboard_broadcast,
                    &config,
                ).await;
                
                // Record delivery status
                let mut delivery_records = delivery_records.write().await;
                let delivery_record = AlertDeliveryRecord {
                    alert_id: alert.alert_id.clone(),
                    status: if delivery_result.is_ok() { 
                        AlertDeliveryStatus::Delivered 
                    } else { 
                        AlertDeliveryStatus::Failed 
                    },
                    delivery_attempts: 1,
                    last_attempt_timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    delivery_channels: vec!["dashboard".to_string()],
                };
                delivery_records.insert(alert.alert_id.clone(), delivery_record);
                
                // Update stats
                let processing_time = start_time.elapsed().as_millis() as f64;
                let mut stats = stats.write().await;
                if delivery_result.is_ok() {
                    stats.alerts_delivered += 1;
                } else {
                    stats.alerts_failed += 1;
                }
                
                // Update average delivery time
                let total_processed = stats.alerts_delivered + stats.alerts_failed;
                if total_processed > 0 {
                    stats.average_delivery_time_ms = 
                        (stats.average_delivery_time_ms * (total_processed - 1) as f64 + processing_time) / total_processed as f64;
                }
                
                // Remove from pending alerts
                let mut pending_alerts = pending_alerts.write().await;
                pending_alerts.retain(|a| a.alert_id != alert.alert_id);
                stats.pending_alerts = pending_alerts.len();
            }
        });
    }
    
    /// Process a single alert
    async fn process_single_alert(
        alert: &RiskAlert,
        subscriptions: &Arc<RwLock<HashMap<UserId, AlertSubscription>>>,
        dashboard_broadcast: &broadcast::Sender<RiskAlert>,
        config: &AlertSystemConfig,
    ) -> Result<(), RiskError> {
        // Check if user is subscribed
        let subscriptions = subscriptions.read().await;
        if let Some(subscription) = subscriptions.get(&alert.user_id) {
            if !subscription.enabled || !subscription.severity_filter.contains(&alert.severity) {
                return Ok(()); // Skip delivery for filtered alerts
            }
        }
        
        // Deliver to dashboard if enabled
        if config.enable_dashboard_alerts {
            let _ = dashboard_broadcast.send(alert.clone());
        }
        
        // Placeholder for email delivery
        if config.enable_email_alerts {
            // Would implement email delivery here
        }
        
        // Placeholder for webhook delivery
        if config.enable_webhook_alerts {
            // Would implement webhook delivery here
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::types::TradeId;
    
    fn create_test_config() -> AlertSystemConfig {
        AlertSystemConfig {
            max_pending_alerts: 100,
            alert_batch_size: 10,
            delivery_timeout_ms: 1000,
            retry_attempts: 2,
            enable_dashboard_alerts: true,
            enable_email_alerts: false,
            enable_webhook_alerts: false,
        }
    }
    
    fn create_test_alert(user_id: UserId, severity: AlertSeverity) -> RiskAlert {
        RiskAlert {
            user_id,
            alert_id: Uuid::new_v4().to_string(),
            rule_name: "test_rule".to_string(),
            severity,
            message: "Test alert message".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            trade_id: Some(Uuid::new_v4()),
        }
    }
    
    #[tokio::test]
    async fn test_alert_system_creation() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.total_alerts_processed, 0);
        assert_eq!(stats.alerts_delivered, 0);
        assert_eq!(stats.active_subscriptions, 0);
    }
    
    #[tokio::test]
    async fn test_single_alert_submission() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        let alert = create_test_alert(user_id, AlertSeverity::High);
        let alert_id = alert.alert_id.clone();
        
        // Submit alert
        let result = alert_system.submit_alert(alert).await;
        assert!(result.is_ok());
        
        // Check stats
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.total_alerts_processed, 1);
        
        // Check delivery status
        let delivery_status = alert_system.get_delivery_status(&alert_id).await;
        assert!(delivery_status.is_some());
    }
    
    #[tokio::test]
    async fn test_batch_alert_submission() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        let alerts = vec![
            create_test_alert(user_id, AlertSeverity::High),
            create_test_alert(user_id, AlertSeverity::Medium),
            create_test_alert(user_id, AlertSeverity::Low),
        ];
        
        // Submit batch
        let result = alert_system.submit_alerts_batch(alerts).await;
        assert!(result.is_ok());
        
        // Check stats
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.total_alerts_processed, 3);
    }
    
    #[tokio::test]
    async fn test_user_subscription_management() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        let subscription = AlertSubscription {
            user_id,
            severity_filter: vec![AlertSeverity::High, AlertSeverity::Critical],
            channels: vec!["dashboard".to_string(), "email".to_string()],
            enabled: true,
        };
        
        // Subscribe user
        let result = alert_system.subscribe_user(subscription.clone()).await;
        assert!(result.is_ok());
        
        // Check subscription
        let retrieved_subscription = alert_system.get_user_subscription(&user_id).await;
        assert!(retrieved_subscription.is_some());
        assert_eq!(retrieved_subscription.unwrap().user_id, user_id);
        
        // Check stats
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.active_subscriptions, 1);
        
        // Unsubscribe user
        let result = alert_system.unsubscribe_user(&user_id).await;
        assert!(result.is_ok());
        
        // Check subscription removed
        let retrieved_subscription = alert_system.get_user_subscription(&user_id).await;
        assert!(retrieved_subscription.is_none());
        
        // Check stats updated
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.active_subscriptions, 0);
    }
    
    #[tokio::test]
    async fn test_user_alert_filtering() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        
        // Submit alerts for different users
        let alert1 = create_test_alert(user_id, AlertSeverity::High);
        let alert2 = create_test_alert(Uuid::new_v4(), AlertSeverity::Medium);
        let alert3 = create_test_alert(user_id, AlertSeverity::Low);
        
        alert_system.submit_alert(alert1).await.unwrap();
        alert_system.submit_alert(alert2).await.unwrap();
        alert_system.submit_alert(alert3).await.unwrap();
        
        // Get alerts for specific user
        let user_alerts = alert_system.get_user_alerts(&user_id, 10).await;
        assert_eq!(user_alerts.len(), 2);
        assert!(user_alerts.iter().all(|alert| alert.user_id == user_id));
    }
    
    #[tokio::test]
    async fn test_dashboard_broadcast() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        // Get dashboard receiver
        let mut dashboard_receiver = alert_system.get_dashboard_receiver();
        
        let user_id = Uuid::new_v4();
        let alert = create_test_alert(user_id, AlertSeverity::Critical);
        
        // Submit alert
        alert_system.submit_alert(alert.clone()).await.unwrap();
        
        // Receive from dashboard
        tokio::time::timeout(
            tokio::time::Duration::from_millis(500),
            dashboard_receiver.recv()
        ).await.unwrap().unwrap();
    }
    
    #[tokio::test]
    async fn test_alert_queue_capacity() {
        let mut config = create_test_config();
        config.max_pending_alerts = 2; // Very small limit
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        
        // Submit alerts up to capacity
        let alert1 = create_test_alert(user_id, AlertSeverity::High);
        let alert2 = create_test_alert(user_id, AlertSeverity::Medium);
        
        assert!(alert_system.submit_alert(alert1).await.is_ok());
        assert!(alert_system.submit_alert(alert2).await.is_ok());
        
        // Third alert should fail
        let alert3 = create_test_alert(user_id, AlertSeverity::Low);
        let result = alert_system.submit_alert(alert3).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_cleanup_old_data() {
        let config = create_test_config();
        let alert_system = AlertSystem::new(config);
        
        let user_id = Uuid::new_v4();
        let alert = create_test_alert(user_id, AlertSeverity::High);
        
        // Submit alert
        alert_system.submit_alert(alert).await.unwrap();
        
        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Cleanup with 0 hour max age (should remove everything)
        let result = alert_system.cleanup_old_data(0).await;
        assert!(result.is_ok());
        
        // Check that pending alerts are cleaned up
        let stats = alert_system.get_stats().await;
        assert_eq!(stats.pending_alerts, 0);
    }
    
    #[tokio::test]
    async fn test_config_validation() {
        let config = AlertSystemConfig {
            max_pending_alerts: 1000,
            alert_batch_size: 50,
            delivery_timeout_ms: 3000,
            retry_attempts: 5,
            enable_dashboard_alerts: true,
            enable_email_alerts: true,
            enable_webhook_alerts: false,
        };
        
        let alert_system = AlertSystem::new(config.clone());
        assert_eq!(alert_system.config.max_pending_alerts, config.max_pending_alerts);
        assert_eq!(alert_system.config.alert_batch_size, config.alert_batch_size);
        assert_eq!(alert_system.config.enable_email_alerts, config.enable_email_alerts);
    }
}
