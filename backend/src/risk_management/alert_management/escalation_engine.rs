use crate::risk_management::alert_management::{
    AlertCategory, AlertSeverity, EscalationRule, NotificationChannel, RiskAlert
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationEngine {
    rules: HashMap<String, EscalationRule>,
    active_escalations: HashMap<Uuid, EscalationState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EscalationState {
    alert_id: Uuid,
    current_level: u8,
    last_escalation: DateTime<Utc>,
    next_escalation: DateTime<Utc>,
    rule_id: Uuid,
}

impl EscalationEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            rules: HashMap::new(),
            active_escalations: HashMap::new(),
        };
        
        engine.initialize_default_rules();
        engine
    }

    fn initialize_default_rules(&mut self) {
        let default_rules = vec![
            // Critical alerts escalate every 5 minutes, max 3 levels
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::RiskThreshold,
                severity: AlertSeverity::Critical,
                escalation_delay_minutes: 5,
                max_escalation_level: 3,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Email,
                    NotificationChannel::Slack,
                ],
                escalation_recipients: vec![
                    "risk-team@hyperdex.com".to_string(),
                    "cto@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::LiquidityRisk,
                severity: AlertSeverity::Critical,
                escalation_delay_minutes: 3,
                max_escalation_level: 4,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Slack,
                    NotificationChannel::Telegram,
                ],
                escalation_recipients: vec![
                    "liquidity-team@hyperdex.com".to_string(),
                    "emergency@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::SystemHealth,
                severity: AlertSeverity::Critical,
                escalation_delay_minutes: 2,
                max_escalation_level: 5,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Slack,
                    NotificationChannel::Email,
                    NotificationChannel::Telegram,
                ],
                escalation_recipients: vec![
                    "devops@hyperdex.com".to_string(),
                    "oncall@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            // High severity alerts escalate every 15 minutes, max 2 levels
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::RiskThreshold,
                severity: AlertSeverity::High,
                escalation_delay_minutes: 15,
                max_escalation_level: 2,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Email,
                ],
                escalation_recipients: vec![
                    "risk-team@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::PositionLimit,
                severity: AlertSeverity::High,
                escalation_delay_minutes: 10,
                max_escalation_level: 2,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Slack,
                ],
                escalation_recipients: vec![
                    "trading-team@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::PriceImpact,
                severity: AlertSeverity::High,
                escalation_delay_minutes: 20,
                max_escalation_level: 2,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Email,
                ],
                escalation_recipients: vec![
                    "trading-team@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            // Medium severity alerts escalate every 30 minutes, max 1 level
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::SlippageExceeded,
                severity: AlertSeverity::Medium,
                escalation_delay_minutes: 30,
                max_escalation_level: 1,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                    NotificationChannel::Email,
                ],
                escalation_recipients: vec![
                    "support@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
            EscalationRule {
                id: Uuid::new_v4(),
                category: AlertCategory::GasPrice,
                severity: AlertSeverity::Medium,
                escalation_delay_minutes: 45,
                max_escalation_level: 1,
                escalation_channels: vec![
                    NotificationChannel::WebSocket,
                ],
                escalation_recipients: vec![
                    "operations@hyperdex.com".to_string(),
                ],
                enabled: true,
                created_at: Utc::now(),
            },
        ];

        for rule in default_rules {
            let key = self.rule_key(&rule.category, &rule.severity);
            self.rules.insert(key, rule);
        }
    }

    pub fn add_rule(&mut self, rule: EscalationRule) -> Result<()> {
        let key = self.rule_key(&rule.category, &rule.severity);
        self.rules.insert(key, rule);
        Ok(())
    }

    pub fn remove_rule(&mut self, category: &AlertCategory, severity: &AlertSeverity) -> Result<()> {
        let key = self.rule_key(category, severity);
        self.rules.remove(&key);
        Ok(())
    }

    pub fn get_rule(&self, category: &AlertCategory, severity: &AlertSeverity) -> Option<&EscalationRule> {
        let key = self.rule_key(category, severity);
        self.rules.get(&key).filter(|rule| rule.enabled)
    }

    pub fn start_escalation(&mut self, alert: &RiskAlert) -> Result<()> {
        if let Some(rule) = self.get_rule(&alert.category, &alert.severity) {
            let escalation_state = EscalationState {
                alert_id: alert.id,
                current_level: 0,
                last_escalation: alert.created_at,
                next_escalation: alert.created_at + chrono::Duration::minutes(rule.escalation_delay_minutes as i64),
                rule_id: rule.id,
            };
            
            self.active_escalations.insert(alert.id, escalation_state);
        }
        Ok(())
    }

    pub fn stop_escalation(&mut self, alert_id: Uuid) -> Result<()> {
        self.active_escalations.remove(&alert_id);
        Ok(())
    }

    pub fn should_escalate(&self, alert: &RiskAlert) -> bool {
        if let Some(escalation_state) = self.active_escalations.get(&alert.id) {
            if let Some(rule) = self.rules.values().find(|r| r.id == escalation_state.rule_id) {
                let now = Utc::now();
                return now >= escalation_state.next_escalation 
                    && escalation_state.current_level < rule.max_escalation_level
                    && rule.enabled;
            }
        }
        false
    }

    pub fn escalate_alert(&mut self, alert: &mut RiskAlert) -> Result<EscalationInfo> {
        if let Some(escalation_state) = self.active_escalations.get_mut(&alert.id) {
            if let Some(rule) = self.rules.values().find(|r| r.id == escalation_state.rule_id) {
                escalation_state.current_level += 1;
                escalation_state.last_escalation = Utc::now();
                escalation_state.next_escalation = Utc::now() + chrono::Duration::minutes(rule.escalation_delay_minutes as i64);
                
                alert.escalate();
                
                let escalation_info = EscalationInfo {
                    alert_id: alert.id,
                    escalation_level: escalation_state.current_level,
                    max_level: rule.max_escalation_level,
                    channels: rule.escalation_channels.clone(),
                    recipients: rule.escalation_recipients.clone(),
                    next_escalation: Some(escalation_state.next_escalation),
                    rule_id: rule.id,
                };
                
                return Ok(escalation_info);
            }
        }
        
        Err(anyhow::anyhow!("No escalation state found for alert"))
    }

    pub fn get_alerts_ready_for_escalation(&self) -> Vec<Uuid> {
        let now = Utc::now();
        self.active_escalations
            .iter()
            .filter(|(_, state)| {
                if let Some(rule) = self.rules.values().find(|r| r.id == state.rule_id) {
                    now >= state.next_escalation 
                        && state.current_level < rule.max_escalation_level
                        && rule.enabled
                } else {
                    false
                }
            })
            .map(|(alert_id, _)| *alert_id)
            .collect()
    }

    pub fn get_escalation_info(&self, alert_id: Uuid) -> Option<EscalationInfo> {
        if let Some(escalation_state) = self.active_escalations.get(&alert_id) {
            if let Some(rule) = self.rules.values().find(|r| r.id == escalation_state.rule_id) {
                return Some(EscalationInfo {
                    alert_id,
                    escalation_level: escalation_state.current_level,
                    max_level: rule.max_escalation_level,
                    channels: rule.escalation_channels.clone(),
                    recipients: rule.escalation_recipients.clone(),
                    next_escalation: Some(escalation_state.next_escalation),
                    rule_id: rule.id,
                });
            }
        }
        None
    }

    pub fn update_rule(&mut self, rule: EscalationRule) -> Result<()> {
        let key = self.rule_key(&rule.category, &rule.severity);
        self.rules.insert(key, rule);
        Ok(())
    }

    pub fn enable_rule(&mut self, category: &AlertCategory, severity: &AlertSeverity) -> Result<()> {
        let key = self.rule_key(category, severity);
        if let Some(rule) = self.rules.get_mut(&key) {
            rule.enabled = true;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Rule not found"))
        }
    }

    pub fn disable_rule(&mut self, category: &AlertCategory, severity: &AlertSeverity) -> Result<()> {
        let key = self.rule_key(category, severity);
        if let Some(rule) = self.rules.get_mut(&key) {
            rule.enabled = false;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Rule not found"))
        }
    }

    pub fn get_all_rules(&self) -> Vec<&EscalationRule> {
        self.rules.values().collect()
    }

    pub fn get_active_escalations(&self) -> Vec<(Uuid, &EscalationState)> {
        self.active_escalations.iter().map(|(id, state)| (*id, state)).collect()
    }

    pub fn cleanup_resolved_escalations(&mut self, resolved_alert_ids: &[Uuid]) {
        for alert_id in resolved_alert_ids {
            self.active_escalations.remove(alert_id);
        }
    }

    fn rule_key(&self, category: &AlertCategory, severity: &AlertSeverity) -> String {
        format!("{:?}_{:?}", category, severity)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationInfo {
    pub alert_id: Uuid,
    pub escalation_level: u8,
    pub max_level: u8,
    pub channels: Vec<NotificationChannel>,
    pub recipients: Vec<String>,
    pub next_escalation: Option<DateTime<Utc>>,
    pub rule_id: Uuid,
}

impl Default for EscalationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk_management::alert_management::RiskAlert;

    #[test]
    fn test_escalation_engine_initialization() {
        let engine = EscalationEngine::new();
        
        // Should have default rules
        assert!(!engine.rules.is_empty());
        assert!(engine.active_escalations.is_empty());
        
        // Check specific default rules
        let critical_risk_rule = engine.get_rule(&AlertCategory::RiskThreshold, &AlertSeverity::Critical);
        assert!(critical_risk_rule.is_some());
        assert_eq!(critical_risk_rule.unwrap().escalation_delay_minutes, 5);
        assert_eq!(critical_risk_rule.unwrap().max_escalation_level, 3);
    }

    #[test]
    fn test_add_and_get_rule() {
        let mut engine = EscalationEngine::new();
        
        let custom_rule = EscalationRule {
            id: Uuid::new_v4(),
            category: AlertCategory::FailedTransaction,
            severity: AlertSeverity::High,
            escalation_delay_minutes: 10,
            max_escalation_level: 2,
            escalation_channels: vec![NotificationChannel::WebSocket, NotificationChannel::Email],
            escalation_recipients: vec!["custom@hyperdex.com".to_string()],
            enabled: true,
            created_at: Utc::now(),
        };
        
        engine.add_rule(custom_rule.clone()).unwrap();
        
        let retrieved_rule = engine.get_rule(&AlertCategory::FailedTransaction, &AlertSeverity::High);
        assert!(retrieved_rule.is_some());
        assert_eq!(retrieved_rule.unwrap().escalation_delay_minutes, 10);
        assert_eq!(retrieved_rule.unwrap().max_escalation_level, 2);
    }

    #[test]
    fn test_start_escalation() {
        let mut engine = EscalationEngine::new();
        
        let alert = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::Critical,
            "Critical Risk Alert".to_string(),
            "Risk threshold exceeded".to_string(),
            0.10,
            0.15,
        );
        
        engine.start_escalation(&alert).unwrap();
        
        // Should have active escalation
        assert!(engine.active_escalations.contains_key(&alert.id));
        
        let escalation_state = engine.active_escalations.get(&alert.id).unwrap();
        assert_eq!(escalation_state.current_level, 0);
        assert_eq!(escalation_state.alert_id, alert.id);
    }

    #[test]
    fn test_should_escalate() {
        let mut engine = EscalationEngine::new();
        
        let mut alert = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::Critical,
            "Critical Risk Alert".to_string(),
            "Risk threshold exceeded".to_string(),
            0.10,
            0.15,
        );
        
        // Set created_at to past to simulate time passage
        alert.created_at = Utc::now() - chrono::Duration::minutes(10);
        
        engine.start_escalation(&alert).unwrap();
        
        // Should be ready for escalation (delay is 5 minutes for critical risk)
        assert!(engine.should_escalate(&alert));
    }

    #[test]
    fn test_escalate_alert() {
        let mut engine = EscalationEngine::new();
        
        let mut alert = RiskAlert::new(
            AlertCategory::LiquidityRisk,
            AlertSeverity::Critical,
            "Liquidity Crisis".to_string(),
            "Critical liquidity shortage".to_string(),
            1000000.0,
            250000.0,
        );
        
        engine.start_escalation(&alert).unwrap();
        
        let escalation_info = engine.escalate_alert(&mut alert).unwrap();
        
        assert_eq!(escalation_info.escalation_level, 1);
        assert_eq!(escalation_info.alert_id, alert.id);
        assert!(!escalation_info.channels.is_empty());
        assert!(!escalation_info.recipients.is_empty());
        
        // Alert should be escalated
        assert_eq!(alert.escalation_level, 1);
        assert!(alert.escalated_at.is_some());
    }

    #[test]
    fn test_stop_escalation() {
        let mut engine = EscalationEngine::new();
        
        let alert = RiskAlert::new(
            AlertCategory::SystemHealth,
            AlertSeverity::Critical,
            "System Health Alert".to_string(),
            "System health degraded".to_string(),
            0.95,
            0.75,
        );
        
        engine.start_escalation(&alert).unwrap();
        assert!(engine.active_escalations.contains_key(&alert.id));
        
        engine.stop_escalation(alert.id).unwrap();
        assert!(!engine.active_escalations.contains_key(&alert.id));
    }

    #[test]
    fn test_get_alerts_ready_for_escalation() {
        let mut engine = EscalationEngine::new();
        
        let mut alert1 = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::Critical,
            "Alert 1".to_string(),
            "Description 1".to_string(),
            0.10,
            0.15,
        );
        
        let mut alert2 = RiskAlert::new(
            AlertCategory::LiquidityRisk,
            AlertSeverity::Critical,
            "Alert 2".to_string(),
            "Description 2".to_string(),
            1000000.0,
            250000.0,
        );
        
        // Set created_at to past to simulate time passage
        alert1.created_at = Utc::now() - chrono::Duration::minutes(10);
        alert2.created_at = Utc::now() - chrono::Duration::minutes(5);
        
        engine.start_escalation(&alert1).unwrap();
        engine.start_escalation(&alert2).unwrap();
        
        let ready_alerts = engine.get_alerts_ready_for_escalation();
        
        // Both alerts should be ready (critical risk: 5min delay, liquidity: 3min delay)
        assert_eq!(ready_alerts.len(), 2);
        assert!(ready_alerts.contains(&alert1.id));
        assert!(ready_alerts.contains(&alert2.id));
    }

    #[test]
    fn test_enable_disable_rule() {
        let mut engine = EscalationEngine::new();
        
        // Disable a rule
        engine.disable_rule(&AlertCategory::RiskThreshold, &AlertSeverity::Critical).unwrap();
        
        let rule = engine.get_rule(&AlertCategory::RiskThreshold, &AlertSeverity::Critical);
        assert!(rule.is_none()); // Should not return disabled rule
        
        // Re-enable the rule
        engine.enable_rule(&AlertCategory::RiskThreshold, &AlertSeverity::Critical).unwrap();
        
        let rule = engine.get_rule(&AlertCategory::RiskThreshold, &AlertSeverity::Critical);
        assert!(rule.is_some()); // Should return enabled rule
    }

    #[test]
    fn test_cleanup_resolved_escalations() {
        let mut engine = EscalationEngine::new();
        
        let alert1 = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::High,
            "Alert 1".to_string(),
            "Description 1".to_string(),
            0.05,
            0.08,
        );
        
        let alert2 = RiskAlert::new(
            AlertCategory::PositionLimit,
            AlertSeverity::High,
            "Alert 2".to_string(),
            "Description 2".to_string(),
            100000.0,
            150000.0,
        );
        
        engine.start_escalation(&alert1).unwrap();
        engine.start_escalation(&alert2).unwrap();
        
        assert_eq!(engine.active_escalations.len(), 2);
        
        // Cleanup one resolved alert
        engine.cleanup_resolved_escalations(&[alert1.id]);
        
        assert_eq!(engine.active_escalations.len(), 1);
        assert!(!engine.active_escalations.contains_key(&alert1.id));
        assert!(engine.active_escalations.contains_key(&alert2.id));
    }

    #[test]
    fn test_max_escalation_level() {
        let mut engine = EscalationEngine::new();
        
        let mut alert = RiskAlert::new(
            AlertCategory::RiskThreshold,
            AlertSeverity::Critical,
            "Max Level Test".to_string(),
            "Testing max escalation level".to_string(),
            0.10,
            0.20,
        );
        
        engine.start_escalation(&alert).unwrap();
        
        // Escalate to max level (3 for critical risk threshold)
        for i in 1..=3 {
            let escalation_info = engine.escalate_alert(&mut alert).unwrap();
            assert_eq!(escalation_info.escalation_level, i);
        }
        
        // Should not escalate beyond max level
        assert!(!engine.should_escalate(&alert));
    }
}
