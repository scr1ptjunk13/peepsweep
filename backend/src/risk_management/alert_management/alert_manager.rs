use crate::risk_management::alert_management::{
    AlertCategory, AlertNotification, AlertSeverity, AlertStatus, EscalationEngine, EscalationInfo,
    NotificationChannel, NotificationConfig, NotificationManager, RiskAlert, ThresholdConfig,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::{interval, Duration};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertManagerConfig {
    pub max_active_alerts: usize,
    pub cleanup_interval_seconds: u64,
    pub escalation_check_interval_seconds: u64,
    pub notification_retry_attempts: u8,
    pub alert_retention_hours: u64,
}

impl Default for AlertManagerConfig {
    fn default() -> Self {
        Self {
            max_active_alerts: 10000,
            cleanup_interval_seconds: 300, // 5 minutes
            escalation_check_interval_seconds: 60, // 1 minute
            notification_retry_attempts: 3,
            alert_retention_hours: 168, // 7 days
        }
    }
}

pub struct AlertManager {
    config: AlertManagerConfig,
    alerts: Arc<RwLock<HashMap<Uuid, RiskAlert>>>,
    notifications: Arc<RwLock<HashMap<Uuid, AlertNotification>>>,
    threshold_config: Arc<RwLock<ThresholdConfig>>,
    escalation_engine: Arc<Mutex<EscalationEngine>>,
    notification_manager: Arc<NotificationManager>,
    alert_sender: broadcast::Sender<RiskAlert>,
    notification_sender: broadcast::Sender<AlertNotification>,
}

impl AlertManager {
    pub fn new(
        config: AlertManagerConfig,
        threshold_config: ThresholdConfig,
        notification_manager: NotificationManager,
    ) -> Self {
        let (alert_sender, _) = broadcast::channel(1000);
        let (notification_sender, _) = broadcast::channel(1000);

        Self {
            config,
            alerts: Arc::new(RwLock::new(HashMap::new())),
            notifications: Arc::new(RwLock::new(HashMap::new())),
            threshold_config: Arc::new(RwLock::new(threshold_config)),
            escalation_engine: Arc::new(Mutex::new(EscalationEngine::new())),
            notification_manager: Arc::new(notification_manager),
            alert_sender,
            notification_sender,
        }
    }

    pub async fn start_background_tasks(&self) {
        let manager = Arc::new(self.clone());
        
        // Start escalation check task
        let escalation_manager = manager.clone();
        tokio::spawn(async move {
            escalation_manager.escalation_check_loop().await;
        });

        // Start cleanup task
        let cleanup_manager = manager.clone();
        tokio::spawn(async move {
            cleanup_manager.cleanup_loop().await;
        });

        // Start notification retry task
        let retry_manager = manager.clone();
        tokio::spawn(async move {
            retry_manager.notification_retry_loop().await;
        });
    }

    pub async fn check_and_create_alert(
        &self,
        category: AlertCategory,
        current_value: f64,
        user_id: Option<Uuid>,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<Option<RiskAlert>> {
        let threshold_config = self.threshold_config.read().await;
        
        // Check all severity levels for this category
        let severities = [
            AlertSeverity::Critical,
            AlertSeverity::High,
            AlertSeverity::Medium,
            AlertSeverity::Low,
        ];

        for severity in &severities {
            if threshold_config.should_trigger_alert(&category, severity, current_value, user_id) {
                if let Some(threshold) = threshold_config.get_threshold(&category, severity, user_id) {
                    let title = self.generate_alert_title(&category, severity, current_value, threshold.threshold_value);
                    let description = self.generate_alert_description(&category, severity, current_value, threshold.threshold_value);
                    
                    let mut alert = RiskAlert::new(
                        category.clone(),
                        severity.clone(),
                        title,
                        description,
                        threshold.threshold_value,
                        current_value,
                    );
                    
                    alert.user_id = user_id;
                    if let Some(meta) = metadata {
                        alert.metadata = meta;
                    }

                    // Store the alert
                    let mut alerts = self.alerts.write().await;
                    alerts.insert(alert.id, alert.clone());
                    
                    // Start escalation tracking
                    let mut escalation_engine = self.escalation_engine.lock().await;
                    escalation_engine.start_escalation(&alert)?;
                    
                    // Send initial notifications
                    self.send_alert_notifications(&alert).await?;
                    
                    // Broadcast alert
                    let _ = self.alert_sender.send(alert.clone());
                    
                    return Ok(Some(alert));
                }
            }
        }
        
        Ok(None)
    }

    pub async fn acknowledge_alert(&self, alert_id: Uuid, acknowledged_by: Uuid) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.get_mut(&alert_id) {
            alert.acknowledge(acknowledged_by);
            
            // Stop escalation
            let mut escalation_engine = self.escalation_engine.lock().await;
            escalation_engine.stop_escalation(alert_id)?;
            
            // Broadcast updated alert
            let _ = self.alert_sender.send(alert.clone());
        }
        Ok(())
    }

    pub async fn resolve_alert(&self, alert_id: Uuid) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.get_mut(&alert_id) {
            alert.resolve();
            
            // Stop escalation
            let mut escalation_engine = self.escalation_engine.lock().await;
            escalation_engine.stop_escalation(alert_id)?;
            
            // Broadcast updated alert
            let _ = self.alert_sender.send(alert.clone());
        }
        Ok(())
    }

    pub async fn get_alert(&self, alert_id: Uuid) -> Option<RiskAlert> {
        let alerts = self.alerts.read().await;
        alerts.get(&alert_id).cloned()
    }

    pub async fn get_active_alerts(&self) -> Vec<RiskAlert> {
        let alerts = self.alerts.read().await;
        alerts
            .values()
            .filter(|alert| alert.is_active())
            .cloned()
            .collect()
    }

    pub async fn get_alerts_by_user(&self, user_id: Uuid) -> Vec<RiskAlert> {
        let alerts = self.alerts.read().await;
        alerts
            .values()
            .filter(|alert| alert.user_id == Some(user_id))
            .cloned()
            .collect()
    }

    pub async fn get_alerts_by_category(&self, category: AlertCategory) -> Vec<RiskAlert> {
        let alerts = self.alerts.read().await;
        alerts
            .values()
            .filter(|alert| alert.category == category)
            .cloned()
            .collect()
    }

    pub async fn get_alert_statistics(&self) -> AlertStatistics {
        let alerts = self.alerts.read().await;
        let notifications = self.notifications.read().await;
        
        let mut stats = AlertStatistics::default();
        
        for alert in alerts.values() {
            stats.total_alerts += 1;
            
            match alert.status {
                AlertStatus::Active => stats.active_alerts += 1,
                AlertStatus::Acknowledged => stats.acknowledged_alerts += 1,
                AlertStatus::Resolved => stats.resolved_alerts += 1,
                AlertStatus::Escalated => stats.escalated_alerts += 1,
                AlertStatus::Suppressed => stats.suppressed_alerts += 1,
            }
            
            match alert.severity {
                AlertSeverity::Critical => stats.critical_alerts += 1,
                AlertSeverity::High => stats.high_alerts += 1,
                AlertSeverity::Medium => stats.medium_alerts += 1,
                AlertSeverity::Low => stats.low_alerts += 1,
            }
        }
        
        stats.total_notifications = notifications.len();
        stats.failed_notifications = notifications
            .values()
            .filter(|n| matches!(n.delivery_status, crate::risk_management::alert_management::DeliveryStatus::Failed))
            .count();
        
        stats
    }

    pub async fn update_threshold(&self, category: AlertCategory, severity: AlertSeverity, threshold_value: f64) -> Result<()> {
        let mut threshold_config = self.threshold_config.write().await;
        threshold_config.update_global_threshold(category, severity, threshold_value)
    }

    pub async fn set_user_threshold(&self, user_id: Uuid, threshold: crate::risk_management::alert_management::AlertThreshold) -> Result<()> {
        let mut threshold_config = self.threshold_config.write().await;
        threshold_config.set_user_threshold(user_id, threshold)
    }

    pub fn subscribe_to_alerts(&self) -> broadcast::Receiver<RiskAlert> {
        self.alert_sender.subscribe()
    }

    pub fn subscribe_to_notifications(&self) -> broadcast::Receiver<AlertNotification> {
        self.notification_sender.subscribe()
    }

    async fn send_alert_notifications(&self, alert: &RiskAlert) -> Result<()> {
        // Determine notification channels based on severity
        let channels = match alert.severity {
            AlertSeverity::Critical => vec![
                NotificationChannel::WebSocket,
                NotificationChannel::Email,
                NotificationChannel::Slack,
            ],
            AlertSeverity::High => vec![
                NotificationChannel::WebSocket,
                NotificationChannel::Email,
            ],
            AlertSeverity::Medium => vec![
                NotificationChannel::WebSocket,
            ],
            AlertSeverity::Low => vec![
                NotificationChannel::WebSocket,
            ],
        };

        for channel in channels {
            let recipient = self.get_default_recipient(&channel, &alert.category);
            
            match self.notification_manager.send_notification(alert, channel, &recipient).await {
                Ok(notification) => {
                    let mut notifications = self.notifications.write().await;
                    notifications.insert(notification.id, notification.clone());
                    let _ = self.notification_sender.send(notification);
                }
                Err(e) => {
                    eprintln!("Failed to send notification: {}", e);
                }
            }
        }
        
        Ok(())
    }

    async fn escalation_check_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.escalation_check_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_escalations().await {
                eprintln!("Error checking escalations: {}", e);
            }
        }
    }

    async fn check_escalations(&self) -> Result<()> {
        let escalation_engine = self.escalation_engine.lock().await;
        let ready_alerts = escalation_engine.get_alerts_ready_for_escalation();
        drop(escalation_engine);
        
        for alert_id in ready_alerts {
            let mut alerts = self.alerts.write().await;
            if let Some(alert) = alerts.get_mut(&alert_id) {
                let mut escalation_engine = self.escalation_engine.lock().await;
                
                if let Ok(escalation_info) = escalation_engine.escalate_alert(alert) {
                    drop(escalation_engine);
                    drop(alerts);
                    
                    // Send escalation notifications
                    self.send_escalation_notifications(&escalation_info).await?;
                    
                    // Broadcast escalated alert
                    let alerts = self.alerts.read().await;
                    if let Some(escalated_alert) = alerts.get(&alert_id) {
                        let _ = self.alert_sender.send(escalated_alert.clone());
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn send_escalation_notifications(&self, escalation_info: &EscalationInfo) -> Result<()> {
        let alerts = self.alerts.read().await;
        if let Some(alert) = alerts.get(&escalation_info.alert_id) {
            for (channel, recipient) in escalation_info.channels.iter().zip(&escalation_info.recipients) {
                match self.notification_manager.send_notification(alert, channel.clone(), recipient).await {
                    Ok(notification) => {
                        let mut notifications = self.notifications.write().await;
                        notifications.insert(notification.id, notification.clone());
                        let _ = self.notification_sender.send(notification);
                    }
                    Err(e) => {
                        eprintln!("Failed to send escalation notification: {}", e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn cleanup_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.cleanup_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.cleanup_old_alerts().await {
                eprintln!("Error during cleanup: {}", e);
            }
        }
    }

    async fn cleanup_old_alerts(&self) -> Result<()> {
        let cutoff_time = Utc::now() - chrono::Duration::hours(self.config.alert_retention_hours as i64);
        let mut resolved_alert_ids = Vec::new();
        
        {
            let mut alerts = self.alerts.write().await;
            let mut notifications = self.notifications.write().await;
            
            // Remove old resolved alerts
            alerts.retain(|id, alert| {
                let should_retain = alert.created_at > cutoff_time || alert.is_active();
                if !should_retain && matches!(alert.status, AlertStatus::Resolved) {
                    resolved_alert_ids.push(*id);
                }
                should_retain
            });
            
            // Remove old notifications
            notifications.retain(|_, notification| {
                notification.sent_at > cutoff_time
            });
        }
        
        // Cleanup escalation states for resolved alerts
        let mut escalation_engine = self.escalation_engine.lock().await;
        escalation_engine.cleanup_resolved_escalations(&resolved_alert_ids);
        
        Ok(())
    }

    async fn notification_retry_loop(&self) {
        let mut interval = interval(Duration::from_secs(60)); // Check every minute
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.retry_failed_notifications().await {
                eprintln!("Error retrying notifications: {}", e);
            }
        }
    }

    async fn retry_failed_notifications(&self) -> Result<()> {
        let now = Utc::now();
        let mut notifications_to_retry = Vec::new();
        
        {
            let notifications = self.notifications.read().await;
            for notification in notifications.values() {
                if matches!(notification.delivery_status, crate::risk_management::alert_management::DeliveryStatus::Failed) 
                    && notification.retry_count < self.config.notification_retry_attempts
                    && notification.next_retry_at.map_or(true, |retry_time| now >= retry_time) {
                    notifications_to_retry.push(notification.clone());
                }
            }
        }
        
        for mut notification in notifications_to_retry {
            let alerts = self.alerts.read().await;
            if let Some(alert) = alerts.get(&notification.alert_id) {
                if let Err(e) = self.notification_manager.retry_failed_notification(&mut notification, alert).await {
                    eprintln!("Failed to retry notification {}: {}", notification.id, e);
                }
                
                let mut notifications = self.notifications.write().await;
                notifications.insert(notification.id, notification.clone());
                let _ = self.notification_sender.send(notification);
            }
        }
        
        Ok(())
    }

    fn get_default_recipient(&self, channel: &NotificationChannel, category: &AlertCategory) -> String {
        match (channel, category) {
            (NotificationChannel::Email, AlertCategory::RiskThreshold) => "risk-team@hyperdex.com".to_string(),
            (NotificationChannel::Email, AlertCategory::LiquidityRisk) => "liquidity-team@hyperdex.com".to_string(),
            (NotificationChannel::Email, AlertCategory::SystemHealth) => "devops@hyperdex.com".to_string(),
            (NotificationChannel::Email, _) => "alerts@hyperdex.com".to_string(),
            (NotificationChannel::Slack, _) => "#alerts".to_string(),
            (NotificationChannel::WebSocket, _) => "all".to_string(),
            (NotificationChannel::Webhook, _) => "default".to_string(),
            _ => "default".to_string(),
        }
    }

    fn generate_alert_title(&self, category: &AlertCategory, severity: &AlertSeverity, current_value: f64, threshold_value: f64) -> String {
        match category {
            AlertCategory::RiskThreshold => format!("🚨 {:?} Risk Alert: {:.2}% (Threshold: {:.2}%)", severity, current_value * 100.0, threshold_value * 100.0),
            AlertCategory::PositionLimit => format!("📊 {:?} Position Alert: ${:.0} (Limit: ${:.0})", severity, current_value, threshold_value),
            AlertCategory::LiquidityRisk => format!("💧 {:?} Liquidity Alert: ${:.0} (Minimum: ${:.0})", severity, current_value, threshold_value),
            AlertCategory::PriceImpact => format!("📈 {:?} Price Impact Alert: {:.2}% (Threshold: {:.2}%)", severity, current_value * 100.0, threshold_value * 100.0),
            AlertCategory::GasPrice => format!("⛽ {:?} Gas Price Alert: {:.0} gwei (Threshold: {:.0} gwei)", severity, current_value, threshold_value),
            AlertCategory::SlippageExceeded => format!("🎯 {:?} Slippage Alert: {:.2}% (Threshold: {:.2}%)", severity, current_value * 100.0, threshold_value * 100.0),
            AlertCategory::FailedTransaction => format!("❌ {:?} Transaction Failure Alert", severity),
            AlertCategory::SystemHealth => format!("🏥 {:?} System Health Alert: {:.1}% (Minimum: {:.1}%)", severity, current_value * 100.0, threshold_value * 100.0),
        }
    }

    fn generate_alert_description(&self, category: &AlertCategory, severity: &AlertSeverity, current_value: f64, threshold_value: f64) -> String {
        match category {
            AlertCategory::RiskThreshold => format!("Portfolio risk has reached {:.2}%, exceeding the {:?} threshold of {:.2}%. Immediate attention required.", current_value * 100.0, severity, threshold_value * 100.0),
            AlertCategory::PositionLimit => format!("Position size of ${:.0} has exceeded the {:?} limit of ${:.0}. Consider reducing exposure.", current_value, severity, threshold_value),
            AlertCategory::LiquidityRisk => format!("Available liquidity of ${:.0} is below the {:?} minimum of ${:.0}. Trading may be impacted.", current_value, severity, threshold_value),
            AlertCategory::PriceImpact => format!("Price impact of {:.2}% exceeds the {:?} threshold of {:.2}%. Large trade detected.", current_value * 100.0, severity, threshold_value * 100.0),
            AlertCategory::GasPrice => format!("Gas price of {:.0} gwei exceeds the {:?} threshold of {:.0} gwei. Network congestion detected.", current_value, severity, threshold_value),
            AlertCategory::SlippageExceeded => format!("Slippage of {:.2}% exceeds the {:?} threshold of {:.2}%. Trade execution may be suboptimal.", current_value * 100.0, severity, threshold_value * 100.0),
            AlertCategory::FailedTransaction => format!("{:?} transaction failure detected. Investigation required.", severity),
            AlertCategory::SystemHealth => format!("System health at {:.1}% is below the {:?} minimum of {:.1}%. System performance degraded.", current_value * 100.0, severity, threshold_value * 100.0),
        }
    }
}

impl Clone for AlertManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            alerts: self.alerts.clone(),
            notifications: self.notifications.clone(),
            threshold_config: self.threshold_config.clone(),
            escalation_engine: self.escalation_engine.clone(),
            notification_manager: self.notification_manager.clone(),
            alert_sender: self.alert_sender.clone(),
            notification_sender: self.notification_sender.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertStatistics {
    pub total_alerts: usize,
    pub active_alerts: usize,
    pub acknowledged_alerts: usize,
    pub resolved_alerts: usize,
    pub escalated_alerts: usize,
    pub suppressed_alerts: usize,
    pub critical_alerts: usize,
    pub high_alerts: usize,
    pub medium_alerts: usize,
    pub low_alerts: usize,
    pub total_notifications: usize,
    pub failed_notifications: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::alert_management::NotificationConfig;

    #[tokio::test]
    async fn test_alert_manager_creation() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        let stats = alert_manager.get_alert_statistics().await;
        assert_eq!(stats.total_alerts, 0);
        assert_eq!(stats.active_alerts, 0);
    }

    #[tokio::test]
    async fn test_check_and_create_alert() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        // Test risk threshold alert (default threshold is 5% for high severity)
        let result = alert_manager.check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.08, // 8% risk, should trigger high severity alert
            None,
            None,
        ).await;
        
        assert!(result.is_ok());
        let alert = result.unwrap();
        assert!(alert.is_some());
        
        let alert = alert.unwrap();
        assert_eq!(alert.category, AlertCategory::RiskThreshold);
        assert_eq!(alert.severity, AlertSeverity::High);
        assert_eq!(alert.current_value, 0.08);
        assert_eq!(alert.threshold_value, 0.05);
        
        // Check that alert was stored
        let stored_alert = alert_manager.get_alert(alert.id).await;
        assert!(stored_alert.is_some());
    }

    #[tokio::test]
    async fn test_acknowledge_alert() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        // Create an alert
        let alert = alert_manager.check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.08,
            None,
            None,
        ).await.unwrap().unwrap();
        
        let user_id = Uuid::new_v4();
        alert_manager.acknowledge_alert(alert.id, user_id).await.unwrap();
        
        let updated_alert = alert_manager.get_alert(alert.id).await.unwrap();
        assert_eq!(updated_alert.status, AlertStatus::Acknowledged);
        assert_eq!(updated_alert.acknowledged_by, Some(user_id));
        assert!(updated_alert.acknowledged_at.is_some());
    }

    #[tokio::test]
    async fn test_resolve_alert() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        // Create an alert
        let alert = alert_manager.check_and_create_alert(
            AlertCategory::LiquidityRisk,
            500000.0, // Below 1M threshold
            None,
            None,
        ).await.unwrap().unwrap();
        
        alert_manager.resolve_alert(alert.id).await.unwrap();
        
        let updated_alert = alert_manager.get_alert(alert.id).await.unwrap();
        assert_eq!(updated_alert.status, AlertStatus::Resolved);
        assert!(updated_alert.resolved_at.is_some());
    }

    #[tokio::test]
    async fn test_get_active_alerts() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        // Create multiple alerts
        let alert1 = alert_manager.check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.08,
            None,
            None,
        ).await.unwrap().unwrap();
        
        let alert2 = alert_manager.check_and_create_alert(
            AlertCategory::LiquidityRisk,
            500000.0,
            None,
            None,
        ).await.unwrap().unwrap();
        
        // Resolve one alert
        alert_manager.resolve_alert(alert2.id).await.unwrap();
        
        let active_alerts = alert_manager.get_active_alerts().await;
        assert_eq!(active_alerts.len(), 1);
        assert_eq!(active_alerts[0].id, alert1.id);
    }

    #[tokio::test]
    async fn test_alert_statistics() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        // Create alerts of different severities
        alert_manager.check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.12, // Should trigger critical (10% threshold)
            None,
            None,
        ).await.unwrap();
        
        alert_manager.check_and_create_alert(
            AlertCategory::PositionLimit,
            150000.0, // Should trigger medium (100k threshold)
            None,
            None,
        ).await.unwrap();
        
        let stats = alert_manager.get_alert_statistics().await;
        assert_eq!(stats.total_alerts, 2);
        assert_eq!(stats.active_alerts, 2);
        assert_eq!(stats.critical_alerts, 1);
        assert_eq!(stats.medium_alerts, 1);
    }

    #[tokio::test]
    async fn test_user_specific_alerts() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        let user_id = Uuid::new_v4();
        
        // Create user-specific alert
        alert_manager.check_and_create_alert(
            AlertCategory::SlippageExceeded,
            0.02, // 2% slippage, should trigger medium (1% threshold)
            Some(user_id),
            None,
        ).await.unwrap();
        
        let user_alerts = alert_manager.get_alerts_by_user(user_id).await;
        assert_eq!(user_alerts.len(), 1);
        assert_eq!(user_alerts[0].user_id, Some(user_id));
        assert_eq!(user_alerts[0].category, AlertCategory::SlippageExceeded);
    }

    #[tokio::test]
    async fn test_alert_subscription() {
        let config = AlertManagerConfig::default();
        let threshold_config = ThresholdConfig::new();
        let notification_config = NotificationConfig::default();
        let notification_manager = NotificationManager::new(notification_config);
        
        let alert_manager = AlertManager::new(config, threshold_config, notification_manager);
        
        let mut alert_receiver = alert_manager.subscribe_to_alerts();
        
        // Create an alert in a separate task
        let manager_clone = alert_manager.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            manager_clone.check_and_create_alert(
                AlertCategory::GasPrice,
                150.0, // Should trigger medium (100 gwei threshold)
                None,
                None,
            ).await.unwrap();
        });
        
        // Should receive the alert
        let received_alert = tokio::time::timeout(Duration::from_secs(1), alert_receiver.recv()).await;
        assert!(received_alert.is_ok());
        
        let alert = received_alert.unwrap().unwrap();
        assert_eq!(alert.category, AlertCategory::GasPrice);
        assert_eq!(alert.current_value, 150.0);
    }
}
