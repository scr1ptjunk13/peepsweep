use crate::risk_management::alert_management::{
    AlertCategory, AlertManager, AlertManagerConfig, NotificationConfig, NotificationManager, ThresholdConfig
};
use crate::risk_management::risk_engine;
use crate::risk_management::types::RiskMetrics;
use anyhow::Result;
use rust_decimal::prelude::ToPrimitive;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct RiskAlertIntegration {
    alert_manager: Arc<AlertManager>,
    risk_engine: Arc<RwLock<risk_engine::RiskProcessingEngine>>,
}

impl RiskAlertIntegration {
    pub fn new(
        alert_manager: Arc<AlertManager>,
        risk_engine: Arc<RwLock<risk_engine::RiskProcessingEngine>>,
    ) -> Self {
        Self {
            alert_manager,
            risk_engine,
        }
    }

    pub async fn monitor_risk_metrics(&self, user_id: Option<Uuid>) -> Result<()> {
        let risk_engine = self.risk_engine.read().await;
        let metrics = if let Some(uid) = user_id {
            risk_engine.calculate_user_risk_metrics(&uid).await?
        } else {
            // For system-wide monitoring, use a default user or aggregate metrics
            return Ok(());
        };
        drop(risk_engine);

        // Check portfolio risk threshold using concentration_risk as proxy for total risk
        let total_risk = metrics.concentration_risk.to_f64().unwrap_or(0.0);
        if let Some(alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::RiskThreshold,
            total_risk,
            user_id,
            Some(self.create_risk_metadata(&metrics)),
        ).await? {
            // Note: emit_risk_alert method would need to be added to TradeEventStreamer
            // For now, we'll skip this to avoid compilation errors
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        // Check exposure limits using total_exposure_usd
        let exposure_usd = metrics.total_exposure_usd.to_f64().unwrap_or(0.0);
        if let Some(alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::PositionLimit,
            exposure_usd,
            user_id,
            Some(self.create_exposure_metadata(&metrics)),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        Ok(())
    }

    pub async fn monitor_trade_execution(&self, trade_id: Uuid, user_id: Option<Uuid>, price_impact: f64, slippage: f64, gas_price: f64) -> Result<()> {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("trade_id".to_string(), trade_id.to_string());
        metadata.insert("gas_price_gwei".to_string(), gas_price.to_string());

        // Check price impact
        if let Some(_alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::PriceImpact,
            price_impact,
            user_id,
            Some(metadata.clone()),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        // Check slippage
        if let Some(_alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::SlippageExceeded,
            slippage,
            user_id,
            Some(metadata.clone()),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        // Check gas price
        if let Some(_alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::GasPrice,
            gas_price,
            user_id,
            Some(metadata),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        Ok(())
    }

    pub async fn monitor_system_health(&self, health_score: f64) -> Result<()> {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
        metadata.insert("component".to_string(), "system_health".to_string());

        if let Some(_alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::SystemHealth,
            health_score,
            None,
            Some(metadata),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        Ok(())
    }

    pub async fn handle_failed_transaction(&self, trade_id: Uuid, user_id: Option<Uuid>, error_message: &str) -> Result<()> {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("trade_id".to_string(), trade_id.to_string());
        metadata.insert("error_message".to_string(), error_message.to_string());
        metadata.insert("failure_time".to_string(), chrono::Utc::now().to_rfc3339());

        if let Some(_alert) = self.alert_manager.check_and_create_alert(
            AlertCategory::FailedTransaction,
            1.0, // Binary: 1.0 = failed, 0.0 = success
            user_id,
            Some(metadata),
        ).await? {
            // self.trade_streamer.emit_risk_alert(&alert).await?;
        }

        Ok(())
    }

    fn create_risk_metadata(&self, metrics: &RiskMetrics) -> std::collections::HashMap<String, String> {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("total_exposure_usd".to_string(), metrics.total_exposure_usd.to_string());
        metadata.insert("concentration_risk".to_string(), metrics.concentration_risk.to_string());
        metadata.insert("var_95".to_string(), metrics.var_95.to_string());
        metadata.insert("max_drawdown".to_string(), metrics.max_drawdown.to_string());
        metadata.insert("sharpe_ratio".to_string(), metrics.sharpe_ratio.to_string());
        metadata.insert("win_rate".to_string(), metrics.win_rate.to_string());
        metadata.insert("avg_trade_size".to_string(), metrics.avg_trade_size.to_string());
        metadata
    }

    fn create_exposure_metadata(&self, metrics: &RiskMetrics) -> std::collections::HashMap<String, String> {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("total_exposure_usd".to_string(), metrics.total_exposure_usd.to_string());
        metadata.insert("concentration_risk".to_string(), metrics.concentration_risk.to_string());
        metadata.insert("sharpe_ratio".to_string(), metrics.sharpe_ratio.to_string());
        metadata
    }
}

pub async fn create_integrated_alert_system(
    risk_engine: Arc<RwLock<risk_engine::RiskProcessingEngine>>,
) -> Result<Arc<RiskAlertIntegration>> {
    // Create notification configuration
    let mut notification_config = NotificationConfig::default();
    notification_config.websocket_enabled = true;
    notification_config.retry_attempts = 3;
    notification_config.retry_delay_seconds = 60;

    // Add webhook URLs for different alert types
    notification_config.webhook_urls.insert(
        "risk_alerts".to_string(),
        "https://api.hyperdex.com/webhooks/risk-alerts".to_string(),
    );
    notification_config.webhook_urls.insert(
        "trading_alerts".to_string(),
        "https://api.hyperdex.com/webhooks/trading-alerts".to_string(),
    );

    // Configure email notifications
    notification_config.email_config.enabled = true;
    notification_config.email_config.from_address = "alerts@hyperdex.com".to_string();

    // Configure Slack notifications
    notification_config.slack_config.enabled = true;
    notification_config.slack_config.channel = "#risk-alerts".to_string();
    notification_config.slack_config.username = "HyperDEX-RiskBot".to_string();

    // Create notification manager
    let notification_manager = NotificationManager::new(notification_config);

    // Create threshold configuration with production-ready defaults
    let threshold_config = ThresholdConfig::new();

    // Create alert manager configuration
    let alert_config = AlertManagerConfig {
        max_active_alerts: 50000,
        cleanup_interval_seconds: 300, // 5 minutes
        escalation_check_interval_seconds: 30, // 30 seconds for faster response
        notification_retry_attempts: 5,
        alert_retention_hours: 720, // 30 days
    };

    // Create alert manager
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));

    // Start background tasks
    alert_manager.start_background_tasks().await;

    // Create integration
    let integration = Arc::new(RiskAlertIntegration::new(
        alert_manager,
        risk_engine,
    ));

    Ok(integration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::types::RiskMetrics;
    use crate::risk_management::position_tracker::{PositionTracker, PositionTrackerConfig};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_risk_alert_integration_creation() {
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let config = risk_engine::RiskEngineConfig::default();
        let risk_engine = Arc::new(RwLock::new(risk_engine::RiskProcessingEngine::new(config, position_tracker)));
        
        let integration = create_integrated_alert_system(risk_engine).await;
        assert!(integration.is_ok());
    }

    #[test]
    fn test_create_risk_metadata() {
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let config = risk_engine::RiskEngineConfig::default();
        let risk_engine = Arc::new(RwLock::new(risk_engine::RiskProcessingEngine::new(config, position_tracker)));
        let alert_manager = Arc::new(AlertManager::new(
            AlertManagerConfig::default(),
            ThresholdConfig::new(),
            NotificationManager::new(NotificationConfig::default()),
        ));

        let integration = RiskAlertIntegration::new(alert_manager, risk_engine);

        let metrics = RiskMetrics {
            total_exposure_usd: Decimal::from_str("1000000.0").unwrap(),
            concentration_risk: Decimal::from_str("0.08").unwrap(),
            var_95: Decimal::from_str("0.12").unwrap(),
            max_drawdown: Decimal::from_str("0.15").unwrap(),
            sharpe_ratio: Decimal::from_str("1.5").unwrap(),
            win_rate: Decimal::from_str("0.65").unwrap(),
            avg_trade_size: Decimal::from_str("50000.0").unwrap(),
        };

        let metadata = integration.create_risk_metadata(&metrics);
        
        assert_eq!(metadata.get("total_exposure_usd").unwrap(), "1000000.0");
        assert_eq!(metadata.get("concentration_risk").unwrap(), "0.08");
        assert_eq!(metadata.get("var_95").unwrap(), "0.12");
        assert_eq!(metadata.get("sharpe_ratio").unwrap(), "1.5");
    }

    #[test]
    fn test_create_exposure_metadata() {
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        let config = risk_engine::RiskEngineConfig::default();
        let risk_engine = Arc::new(RwLock::new(risk_engine::RiskProcessingEngine::new(config, position_tracker)));
        let alert_manager = Arc::new(AlertManager::new(
            AlertManagerConfig::default(),
            ThresholdConfig::new(),
            NotificationManager::new(NotificationConfig::default()),
        ));

        let integration = RiskAlertIntegration::new(alert_manager, risk_engine);

        let metrics = RiskMetrics {
            total_exposure_usd: Decimal::from_str("125000.0").unwrap(),
            concentration_risk: Decimal::from_str("0.15").unwrap(),
            var_95: Decimal::from_str("0.08").unwrap(),
            max_drawdown: Decimal::from_str("0.12").unwrap(),
            sharpe_ratio: Decimal::from_str("1.2").unwrap(),
            win_rate: Decimal::from_str("0.70").unwrap(),
            avg_trade_size: Decimal::from_str("25000.0").unwrap(),
        };

        let metadata = integration.create_exposure_metadata(&metrics);
        
        assert_eq!(metadata.get("total_exposure_usd").unwrap(), "125000.0");
        assert_eq!(metadata.get("concentration_risk").unwrap(), "0.15");
        assert_eq!(metadata.get("sharpe_ratio").unwrap(), "1.2");
    }
}
