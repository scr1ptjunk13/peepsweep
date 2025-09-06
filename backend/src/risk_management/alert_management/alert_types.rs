use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Resolved,
    Escalated,
    Suppressed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AlertCategory {
    RiskThreshold,
    PositionLimit,
    LiquidityRisk,
    PriceImpact,
    GasPrice,
    SlippageExceeded,
    FailedTransaction,
    SystemHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub id: Uuid,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub status: AlertStatus,
    pub title: String,
    pub description: String,
    pub threshold_value: f64,
    pub current_value: f64,
    pub user_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub token_address: Option<String>,
    pub dex_name: Option<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub escalated_at: Option<DateTime<Utc>>,
    pub escalation_level: u8,
    pub notification_channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThreshold {
    pub id: Uuid,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub threshold_value: f64,
    pub comparison_operator: ComparisonOperator,
    pub enabled: bool,
    pub user_id: Option<Uuid>, // None for global thresholds
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertNotification {
    pub id: Uuid,
    pub alert_id: Uuid,
    pub channel: NotificationChannel,
    pub recipient: String,
    pub message: String,
    pub sent_at: DateTime<Utc>,
    pub delivery_status: DeliveryStatus,
    pub retry_count: u8,
    pub next_retry_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotificationChannel {
    WebSocket,
    Webhook,
    Email,
    Slack,
    Discord,
    Telegram,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
    Retrying,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub id: Uuid,
    pub category: AlertCategory,
    pub severity: AlertSeverity,
    pub escalation_delay_minutes: u32,
    pub max_escalation_level: u8,
    pub escalation_channels: Vec<NotificationChannel>,
    pub escalation_recipients: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl RiskAlert {
    pub fn new(
        category: AlertCategory,
        severity: AlertSeverity,
        title: String,
        description: String,
        threshold_value: f64,
        current_value: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            category,
            severity,
            status: AlertStatus::Active,
            title,
            description,
            threshold_value,
            current_value,
            user_id: None,
            trade_id: None,
            token_address: None,
            dex_name: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            escalated_at: None,
            escalation_level: 0,
            notification_channels: Vec::new(),
        }
    }

    pub fn acknowledge(&mut self, acknowledged_by: Uuid) {
        self.status = AlertStatus::Acknowledged;
        self.acknowledged_at = Some(Utc::now());
        self.acknowledged_by = Some(acknowledged_by);
        self.updated_at = Utc::now();
    }

    pub fn resolve(&mut self) {
        self.status = AlertStatus::Resolved;
        self.resolved_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    pub fn escalate(&mut self) {
        self.status = AlertStatus::Escalated;
        self.escalated_at = Some(Utc::now());
        self.escalation_level += 1;
        self.updated_at = Utc::now();
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, AlertStatus::Active | AlertStatus::Escalated)
    }

    pub fn should_escalate(&self, escalation_delay_minutes: u32) -> bool {
        if !self.is_active() {
            return false;
        }

        let escalation_time = if let Some(escalated_at) = self.escalated_at {
            escalated_at
        } else {
            self.created_at
        };

        let elapsed_minutes = (Utc::now() - escalation_time).num_minutes() as u32;
        elapsed_minutes >= escalation_delay_minutes
    }
}

impl ComparisonOperator {
    pub fn evaluate(&self, current_value: f64, threshold_value: f64) -> bool {
        match self {
            ComparisonOperator::GreaterThan => current_value > threshold_value,
            ComparisonOperator::LessThan => current_value < threshold_value,
            ComparisonOperator::GreaterThanOrEqual => current_value >= threshold_value,
            ComparisonOperator::LessThanOrEqual => current_value <= threshold_value,
            ComparisonOperator::Equal => (current_value - threshold_value).abs() < f64::EPSILON,
            ComparisonOperator::NotEqual => (current_value - threshold_value).abs() >= f64::EPSILON,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertStatistics {
    pub total_alerts: u64,
    pub active_alerts: u64,
    pub acknowledged_alerts: u64,
    pub resolved_alerts: u64,
    pub escalated_alerts: u64,
    pub alerts_by_category: HashMap<AlertCategory, u64>,
    pub alerts_by_severity: HashMap<AlertSeverity, u64>,
    pub average_resolution_time_minutes: f64,
    pub escalation_rate_percentage: f64,
}

impl Default for AlertStatistics {
    fn default() -> Self {
        Self {
            total_alerts: 0,
            active_alerts: 0,
            acknowledged_alerts: 0,
            resolved_alerts: 0,
            escalated_alerts: 0,
            alerts_by_category: HashMap::new(),
            alerts_by_severity: HashMap::new(),
            average_resolution_time_minutes: 0.0,
            escalation_rate_percentage: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_alert_creation() {
        let alert = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::High,
            "High Risk Detected".to_string(),
            "Portfolio risk exceeds threshold".to_string(),
            0.05,
            0.08,
        );

        assert_eq!(alert.category, AlertCategory::RiskThreshold);
        assert_eq!(alert.severity, AlertSeverity::High);
        assert_eq!(alert.status, AlertStatus::Active);
        assert_eq!(alert.threshold_value, 0.05);
        assert_eq!(alert.current_value, 0.08);
        assert!(alert.is_active());
    }

    #[test]
    fn test_alert_acknowledgment() {
        let mut alert = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::Medium,
            "Test Alert".to_string(),
            "Test Description".to_string(),
            1.0,
            2.0,
        );

        let user_id = Uuid::new_v4();
        alert.acknowledge(user_id);

        assert_eq!(alert.status, AlertStatus::Acknowledged);
        assert!(alert.acknowledged_at.is_some());
        assert_eq!(alert.acknowledged_by, Some(user_id));
        assert!(!alert.is_active());
    }

    #[test]
    fn test_alert_resolution() {
        let mut alert = RiskAlert::new(
            AlertCategory::SlippageExceeded,
            AlertSeverity::Low,
            "Slippage Alert".to_string(),
            "Slippage exceeded threshold".to_string(),
            0.01,
            0.02,
        );

        alert.resolve();

        assert_eq!(alert.status, AlertStatus::Resolved);
        assert!(alert.resolved_at.is_some());
        assert!(!alert.is_active());
    }

    #[test]
    fn test_alert_escalation() {
        let mut alert = RiskAlert::new(
            AlertCategory::LiquidityRisk,
            AlertSeverity::Critical,
            "Liquidity Crisis".to_string(),
            "Low liquidity detected".to_string(),
            1000000.0,
            500000.0,
        );

        alert.escalate();

        assert_eq!(alert.status, AlertStatus::Escalated);
        assert!(alert.escalated_at.is_some());
        assert_eq!(alert.escalation_level, 1);
        assert!(alert.is_active());
    }

    #[test]
    fn test_should_escalate_timing() {
        let mut alert = RiskAlert::new(
            AlertCategory::SystemHealth,
            AlertSeverity::High,
            "System Alert".to_string(),
            "System health degraded".to_string(),
            0.95,
            0.80,
        );

        // Should not escalate immediately
        assert!(!alert.should_escalate(5));

        // Simulate time passage by setting created_at to past
        alert.created_at = Utc::now() - chrono::Duration::minutes(10);

        // Should escalate after delay
        assert!(alert.should_escalate(5));
    }

    #[test]
    fn test_comparison_operators() {
        assert!(ComparisonOperator::GreaterThan.evaluate(5.0, 3.0));
        assert!(!ComparisonOperator::GreaterThan.evaluate(3.0, 5.0));

        assert!(ComparisonOperator::LessThan.evaluate(3.0, 5.0));
        assert!(!ComparisonOperator::LessThan.evaluate(5.0, 3.0));

        assert!(ComparisonOperator::GreaterThanOrEqual.evaluate(5.0, 5.0));
        assert!(ComparisonOperator::GreaterThanOrEqual.evaluate(6.0, 5.0));

        assert!(ComparisonOperator::LessThanOrEqual.evaluate(5.0, 5.0));
        assert!(ComparisonOperator::LessThanOrEqual.evaluate(4.0, 5.0));

        assert!(ComparisonOperator::Equal.evaluate(5.0, 5.0));
        assert!(!ComparisonOperator::Equal.evaluate(5.0, 5.1));

        assert!(ComparisonOperator::NotEqual.evaluate(5.0, 6.0));
        assert!(!ComparisonOperator::NotEqual.evaluate(5.0, 5.0));
    }
}
