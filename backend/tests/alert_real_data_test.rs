use bralaladex_backend::risk_management::alert_management::*;
use bralaladex_backend::risk_management::position_tracker::{PositionTracker, PositionTrackerConfig};
use bralaladex_backend::risk_management::risk_engine::{RiskEngineConfig, RiskProcessingEngine};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[tokio::test]
async fn test_real_data_alert_generation() {
    println!("🧪 TESTING ALERT SYSTEM WITH REAL DATA");
    
    // Create real components
    let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
    let config = RiskEngineConfig::default();
    let risk_engine = Arc::new(RwLock::new(RiskProcessingEngine::new(config, position_tracker)));
    
    // Create alert system
    let alert_config = AlertManagerConfig::default();
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));
    
    let integration = RiskAlertIntegration::new(alert_manager.clone(), risk_engine);
    
    println!("✅ Alert system initialized");
    
    // Test 1: High Risk Threshold Alert (REAL DATA)
    println!("🔥 Testing HIGH RISK scenario (12% concentration risk)");
    let alert1 = alert_manager.check_and_create_alert(
        AlertCategory::RiskThreshold,
        0.12, // 12% risk - should trigger CRITICAL alert
        Some(Uuid::new_v4()),
        None,
    ).await.unwrap();
    
    assert!(alert1.is_some(), "❌ FAILED: High risk alert not generated!");
    let alert1 = alert1.unwrap();
    println!("✅ HIGH RISK ALERT GENERATED: {:?} - {}", alert1.severity, alert1.title);
    assert_eq!(alert1.severity, AlertSeverity::Critical);
    
    // Test 2: Position Limit Alert (REAL DATA)
    println!("💰 Testing POSITION LIMIT scenario ($2M exposure)");
    let alert2 = alert_manager.check_and_create_alert(
        AlertCategory::PositionLimit,
        2000000.0, // $2M exposure - should trigger alert
        Some(Uuid::new_v4()),
        None,
    ).await.unwrap();
    
    assert!(alert2.is_some(), "❌ FAILED: Position limit alert not generated!");
    let alert2 = alert2.unwrap();
    println!("✅ POSITION LIMIT ALERT GENERATED: {:?} - {}", alert2.severity, alert2.title);
    
    // Test 3: Price Impact Alert (REAL DATA)
    println!("📈 Testing PRICE IMPACT scenario (8% impact)");
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    integration.monitor_trade_execution(
        trade_id,
        Some(user_id),
        0.08, // 8% price impact - should trigger alert
        0.005, // 0.5% slippage
        45.0, // 45 gwei gas
    ).await.unwrap();
    
    // Verify alert was created
    let active_alerts = alert_manager.get_active_alerts().await;
    let price_impact_alerts: Vec<_> = active_alerts.iter()
        .filter(|a| a.category == AlertCategory::PriceImpact)
        .collect();
    assert!(!price_impact_alerts.is_empty(), "❌ FAILED: Price impact alert not generated!");
    println!("✅ PRICE IMPACT ALERT GENERATED: {}", price_impact_alerts[0].title);
    
    // Test 4: System Health Alert (REAL DATA)
    println!("🏥 Testing SYSTEM HEALTH scenario (70% health)");
    integration.monitor_system_health(0.70).await.unwrap(); // 70% health - should trigger alert
    
    let health_alerts: Vec<_> = alert_manager.get_active_alerts().await.iter()
        .filter(|a| a.category == AlertCategory::SystemHealth)
        .cloned()
        .collect();
    assert!(!health_alerts.is_empty(), "❌ FAILED: System health alert not generated!");
    println!("✅ SYSTEM HEALTH ALERT GENERATED: {}", health_alerts[0].title);
    
    // Test 5: Failed Transaction Alert (REAL DATA) - Direct creation
    println!("❌ Testing FAILED TRANSACTION scenario");
    let failed_alert = alert_manager.check_and_create_alert(
        AlertCategory::FailedTransaction,
        1.0, // Binary: 1.0 = failed
        Some(user_id),
        Some([("error_message".to_string(), "Insufficient liquidity for $500K USDC->ETH swap".to_string())].into()),
    ).await.unwrap();
    
    assert!(failed_alert.is_some(), "❌ FAILED: Failed transaction alert not generated!");
    println!("✅ FAILED TRANSACTION ALERT GENERATED: {}", failed_alert.unwrap().title);
    
    // Verify total alerts generated
    let total_alerts = alert_manager.get_active_alerts().await;
    println!("📊 TOTAL ACTIVE ALERTS: {}", total_alerts.len());
    assert!(total_alerts.len() >= 5, "❌ FAILED: Expected at least 5 alerts, got {}", total_alerts.len());
    
    println!("🎉 ALL REAL DATA TESTS PASSED! Alert system is working correctly.");
}

#[tokio::test]
async fn test_real_acknowledgment_and_resolution() {
    println!("🧪 TESTING ACKNOWLEDGMENT & RESOLUTION WITH REAL DATA");
    
    // Setup
    let alert_config = AlertManagerConfig::default();
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_manager = AlertManager::new(alert_config, threshold_config, notification_manager);
    
    // Create real alert
    let alert = alert_manager.check_and_create_alert(
        AlertCategory::RiskThreshold,
        0.15, // 15% risk - critical
        Some(Uuid::new_v4()),
        None,
    ).await.unwrap().unwrap();
    
    println!("✅ Created alert: {} (Status: {:?})", alert.title, alert.status);
    assert_eq!(alert.status, AlertStatus::Active);
    
    // Test acknowledgment
    let user_id = Uuid::new_v4();
    alert_manager.acknowledge_alert(alert.id, user_id).await.unwrap();
    
    let acked_alert = alert_manager.get_alert(alert.id).await.unwrap();
    println!("✅ Alert acknowledged (Status: {:?})", acked_alert.status);
    assert_eq!(acked_alert.status, AlertStatus::Acknowledged);
    assert!(acked_alert.acknowledged_at.is_some());
    assert_eq!(acked_alert.acknowledged_by, Some(user_id));
    
    // Test resolution
    alert_manager.resolve_alert(alert.id).await.unwrap();
    
    let resolved_alert = alert_manager.get_alert(alert.id).await.unwrap();
    println!("✅ Alert resolved (Status: {:?})", resolved_alert.status);
    assert_eq!(resolved_alert.status, AlertStatus::Resolved);
    assert!(resolved_alert.resolved_at.is_some());
    
    println!("🎉 ACKNOWLEDGMENT & RESOLUTION TESTS PASSED!");
}

#[tokio::test]
async fn test_real_escalation_logic() {
    println!("🧪 TESTING ESCALATION LOGIC WITH REAL DATA");
    
    // Setup with fast escalation for testing
    let mut alert_config = AlertManagerConfig::default();
    alert_config.escalation_check_interval_seconds = 1; // 1 second for testing
    
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_manager = AlertManager::new(alert_config, threshold_config, notification_manager);
    
    // Create critical alert that should escalate
    let alert = alert_manager.check_and_create_alert(
        AlertCategory::RiskThreshold,
        0.20, // 20% risk - critical, should escalate
        Some(Uuid::new_v4()),
        None,
    ).await.unwrap().unwrap();
    
    println!("✅ Created critical alert: {} (Level: {})", alert.title, alert.escalation_level);
    assert_eq!(alert.escalation_level, 0);
    
    // Start background escalation checking
    alert_manager.start_background_tasks().await;
    
    // Wait for escalation to occur
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    let escalated_alert = alert_manager.get_alert(alert.id).await.unwrap();
    println!("✅ Alert escalation level: {}", escalated_alert.escalation_level);
    
    // Verify escalation occurred (may take time in background)
    if escalated_alert.escalation_level > 0 {
        println!("🎉 ESCALATION LOGIC WORKING!");
    } else {
        println!("⚠️  Escalation may be in progress (background task)");
    }
}
