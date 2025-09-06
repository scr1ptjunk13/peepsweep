use crate::risk_management::alert_management::{AlertCategory, AlertSeverity, AlertThreshold, ComparisonOperator};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    global_thresholds: HashMap<String, AlertThreshold>,
    user_thresholds: HashMap<Uuid, HashMap<String, AlertThreshold>>,
}

impl ThresholdConfig {
    pub fn new() -> Self {
        let mut config = Self {
            global_thresholds: HashMap::new(),
            user_thresholds: HashMap::new(),
        };
        
        // Initialize with default global thresholds
        config.initialize_default_thresholds();
        config
    }

    fn initialize_default_thresholds(&mut self) {
        let default_thresholds = vec![
            // Risk thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::RiskThreshold,
                severity: AlertSeverity::High,
                threshold_value: 0.05, // 5% portfolio risk
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::RiskThreshold,
                severity: AlertSeverity::Critical,
                threshold_value: 0.10, // 10% portfolio risk
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Position limit thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::PositionLimit,
                severity: AlertSeverity::Medium,
                threshold_value: 100000.0, // $100k position size
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::PositionLimit,
                severity: AlertSeverity::High,
                threshold_value: 500000.0, // $500k position size
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Liquidity risk thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::LiquidityRisk,
                severity: AlertSeverity::Medium,
                threshold_value: 1000000.0, // $1M liquidity
                comparison_operator: ComparisonOperator::LessThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::LiquidityRisk,
                severity: AlertSeverity::High,
                threshold_value: 500000.0, // $500k liquidity
                comparison_operator: ComparisonOperator::LessThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Price impact thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::PriceImpact,
                severity: AlertSeverity::Medium,
                threshold_value: 0.02, // 2% price impact
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::PriceImpact,
                severity: AlertSeverity::High,
                threshold_value: 0.05, // 5% price impact
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Gas price thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::GasPrice,
                severity: AlertSeverity::Medium,
                threshold_value: 100.0, // 100 gwei
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::GasPrice,
                severity: AlertSeverity::High,
                threshold_value: 200.0, // 200 gwei
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Slippage thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::SlippageExceeded,
                severity: AlertSeverity::Medium,
                threshold_value: 0.01, // 1% slippage
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::SlippageExceeded,
                severity: AlertSeverity::High,
                threshold_value: 0.03, // 3% slippage
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // System health thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::SystemHealth,
                severity: AlertSeverity::Medium,
                threshold_value: 0.90, // 90% system health
                comparison_operator: ComparisonOperator::LessThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::SystemHealth,
                severity: AlertSeverity::Critical,
                threshold_value: 0.80, // 80% system health
                comparison_operator: ComparisonOperator::LessThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Failed transaction thresholds
            AlertThreshold {
                id: Uuid::new_v4(),
                category: AlertCategory::FailedTransaction,
                severity: AlertSeverity::High,
                threshold_value: 0.5, // Binary threshold (0.5 = trigger on any failure)
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ];

        for threshold in default_thresholds {
            let key = self.threshold_key(&threshold.category, &threshold.severity);
            self.global_thresholds.insert(key, threshold);
        }
    }

    pub fn get_threshold(&self, category: &AlertCategory, severity: &AlertSeverity, user_id: Option<Uuid>) -> Option<&AlertThreshold> {
        let key = self.threshold_key(category, severity);
        
        // Check user-specific thresholds first
        if let Some(user_id) = user_id {
            if let Some(user_thresholds) = self.user_thresholds.get(&user_id) {
                if let Some(threshold) = user_thresholds.get(&key) {
                    if threshold.enabled {
                        return Some(threshold);
                    }
                }
            }
        }
        
        // Fall back to global thresholds
        self.global_thresholds.get(&key).filter(|t| t.enabled)
    }

    pub fn set_user_threshold(&mut self, user_id: Uuid, threshold: AlertThreshold) -> Result<()> {
        let key = self.threshold_key(&threshold.category, &threshold.severity);
        
        self.user_thresholds
            .entry(user_id)
            .or_insert_with(HashMap::new)
            .insert(key, threshold);
        
        Ok(())
    }

    pub fn update_global_threshold(&mut self, category: AlertCategory, severity: AlertSeverity, threshold_value: f64) -> Result<()> {
        let key = self.threshold_key(&category, &severity);
        
        if let Some(threshold) = self.global_thresholds.get_mut(&key) {
            threshold.threshold_value = threshold_value;
            threshold.updated_at = Utc::now();
            Ok(())
        } else {
            // Create new threshold if it doesn't exist
            let new_threshold = AlertThreshold {
                id: Uuid::new_v4(),
                category,
                severity,
                threshold_value,
                comparison_operator: ComparisonOperator::GreaterThan,
                enabled: true,
                user_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.global_thresholds.insert(key, new_threshold);
            Ok(())
        }
    }

    pub fn enable_threshold(&mut self, category: &AlertCategory, severity: &AlertSeverity, user_id: Option<Uuid>) -> Result<()> {
        let key = self.threshold_key(category, severity);
        
        if let Some(user_id) = user_id {
            if let Some(user_thresholds) = self.user_thresholds.get_mut(&user_id) {
                if let Some(threshold) = user_thresholds.get_mut(&key) {
                    threshold.enabled = true;
                    threshold.updated_at = Utc::now();
                    return Ok(());
                }
            }
        }
        
        if let Some(threshold) = self.global_thresholds.get_mut(&key) {
            threshold.enabled = true;
            threshold.updated_at = Utc::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Threshold not found"))
        }
    }

    pub fn disable_threshold(&mut self, category: &AlertCategory, severity: &AlertSeverity, user_id: Option<Uuid>) -> Result<()> {
        let key = self.threshold_key(category, severity);
        
        if let Some(user_id) = user_id {
            if let Some(user_thresholds) = self.user_thresholds.get_mut(&user_id) {
                if let Some(threshold) = user_thresholds.get_mut(&key) {
                    threshold.enabled = false;
                    threshold.updated_at = Utc::now();
                    return Ok(());
                }
            }
        }
        
        if let Some(threshold) = self.global_thresholds.get_mut(&key) {
            threshold.enabled = false;
            threshold.updated_at = Utc::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Threshold not found"))
        }
    }

    pub fn get_all_thresholds_for_user(&self, user_id: Uuid) -> Vec<&AlertThreshold> {
        let mut thresholds = Vec::new();
        
        // Add user-specific thresholds
        if let Some(user_thresholds) = self.user_thresholds.get(&user_id) {
            thresholds.extend(user_thresholds.values());
        }
        
        // Add global thresholds that don't have user overrides
        for (key, global_threshold) in &self.global_thresholds {
            if let Some(user_thresholds) = self.user_thresholds.get(&user_id) {
                if !user_thresholds.contains_key(key) {
                    thresholds.push(global_threshold);
                }
            } else {
                thresholds.push(global_threshold);
            }
        }
        
        thresholds
    }

    pub fn get_all_global_thresholds(&self) -> Vec<&AlertThreshold> {
        self.global_thresholds.values().collect()
    }

    pub fn should_trigger_alert(&self, category: &AlertCategory, severity: &AlertSeverity, current_value: f64, user_id: Option<Uuid>) -> bool {
        if let Some(threshold) = self.get_threshold(category, severity, user_id) {
            threshold.comparison_operator.evaluate(current_value, threshold.threshold_value)
        } else {
            false
        }
    }

    fn threshold_key(&self, category: &AlertCategory, severity: &AlertSeverity) -> String {
        format!("{:?}_{:?}", category, severity)
    }
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_config_initialization() {
        let config = ThresholdConfig::new();
        
        // Should have default global thresholds
        assert!(!config.global_thresholds.is_empty());
        assert!(config.user_thresholds.is_empty());
        
        // Check specific default thresholds
        let risk_threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None);
        assert!(risk_threshold.is_some());
        assert_eq!(risk_threshold.unwrap().threshold_value, 0.05);
    }

    #[test]
    fn test_user_threshold_override() {
        let mut config = ThresholdConfig::new();
        let user_id = Uuid::new_v4();
        
        // Create custom user threshold
        let custom_threshold = AlertThreshold {
            id: Uuid::new_v4(),
            category: AlertCategory::RiskThreshold,
            severity: AlertSeverity::High,
            threshold_value: 0.03, // Lower than default 0.05
            comparison_operator: ComparisonOperator::GreaterThan,
            enabled: true,
            user_id: Some(user_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        config.set_user_threshold(user_id, custom_threshold).unwrap();
        
        // User should get custom threshold
        let user_threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, Some(user_id));
        assert!(user_threshold.is_some());
        assert_eq!(user_threshold.unwrap().threshold_value, 0.03);
        
        // Other users should get global threshold
        let other_user_id = Uuid::new_v4();
        let global_threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, Some(other_user_id));
        assert!(global_threshold.is_some());
        assert_eq!(global_threshold.unwrap().threshold_value, 0.05);
    }

    #[test]
    fn test_threshold_enable_disable() {
        let mut config = ThresholdConfig::new();
        
        // Disable a global threshold
        config.disable_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None).unwrap();
        
        // Should not return disabled threshold
        let threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None);
        assert!(threshold.is_none());
        
        // Re-enable threshold
        config.enable_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None).unwrap();
        
        // Should return enabled threshold
        let threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None);
        assert!(threshold.is_some());
    }

    #[test]
    fn test_should_trigger_alert() {
        let config = ThresholdConfig::new();
        
        // Risk threshold is 5% (0.05) with GreaterThan operator
        assert!(config.should_trigger_alert(&AlertCategory::RiskThreshold, &AlertSeverity::High, 0.08, None));
        assert!(!config.should_trigger_alert(&AlertCategory::RiskThreshold, &AlertSeverity::High, 0.03, None));
        
        // Liquidity threshold is $1M with LessThan operator
        assert!(config.should_trigger_alert(&AlertCategory::LiquidityRisk, &AlertSeverity::Medium, 500000.0, None));
        assert!(!config.should_trigger_alert(&AlertCategory::LiquidityRisk, &AlertSeverity::Medium, 1500000.0, None));
    }

    #[test]
    fn test_update_global_threshold() {
        let mut config = ThresholdConfig::new();
        
        // Update existing threshold
        config.update_global_threshold(AlertCategory::RiskThreshold, AlertSeverity::High, 0.08).unwrap();
        
        let threshold = config.get_threshold(&AlertCategory::RiskThreshold, &AlertSeverity::High, None);
        assert!(threshold.is_some());
        assert_eq!(threshold.unwrap().threshold_value, 0.08);
    }

    #[test]
    fn test_get_all_thresholds_for_user() {
        let mut config = ThresholdConfig::new();
        let user_id = Uuid::new_v4();
        
        // Add custom user threshold
        let custom_threshold = AlertThreshold {
            id: Uuid::new_v4(),
            category: AlertCategory::RiskThreshold,
            severity: AlertSeverity::High,
            threshold_value: 0.03,
            comparison_operator: ComparisonOperator::GreaterThan,
            enabled: true,
            user_id: Some(user_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        config.set_user_threshold(user_id, custom_threshold).unwrap();
        
        let user_thresholds = config.get_all_thresholds_for_user(user_id);
        
        // Should include custom threshold and all other global thresholds
        assert!(!user_thresholds.is_empty());
        
        // Find the custom threshold
        let custom_found = user_thresholds.iter().any(|t| {
            t.category == AlertCategory::RiskThreshold && 
            t.severity == AlertSeverity::High && 
            t.threshold_value == 0.03
        });
        assert!(custom_found);
    }
}
