use bralaladex_backend::risk_management::alert_management::{
    AlertCategory, AlertManager, AlertManagerConfig, AlertSeverity, NotificationConfig,
    NotificationManager, RiskAlert, RiskAlertIntegration, ThresholdConfig,
};
use bralaladex_backend::risk_management::{Position, RiskEngine, RiskMetrics};
use bralaladex_backend::trade_streaming::TradeEventStreamer;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[tokio::test]
async fn test_complete_alert_management_system() {
    // Initialize components
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_config = AlertManagerConfig::default();
    
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));

    let risk_engine = Arc::new(RwLock::new(RiskEngine::new()));
    let trade_streamer = Arc::new(TradeEventStreamer::new());
    
    let integration = Arc::new(RiskAlertIntegration::new(
        alert_manager.clone(),
        risk_engine.clone(),
        trade_streamer.clone(),
    ));

    // Test 1: Risk threshold alert
    let user_id = Uuid::new_v4();
    let mut metadata = HashMap::new();
    metadata.insert("test_scenario".to_string(), "risk_threshold".to_string());

    let alert = alert_manager
        .check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.08, // 8% risk, exceeds 5% high threshold
            Some(user_id),
            Some(metadata),
        )
        .await
        .unwrap();

    assert!(alert.is_some());
    let alert = alert.unwrap();
    assert_eq!(alert.category, AlertCategory::RiskThreshold);
    assert_eq!(alert.severity, AlertSeverity::High);
    assert_eq!(alert.current_value, 0.08);
    assert_eq!(alert.user_id, Some(user_id));

    // Test 2: Alert acknowledgment
    let acknowledger_id = Uuid::new_v4();
    alert_manager
        .acknowledge_alert(alert.id, acknowledger_id)
        .await
        .unwrap();

    let updated_alert = alert_manager.get_alert(alert.id).await.unwrap();
    assert_eq!(updated_alert.status, bralaladex_backend::risk_management::alert_management::AlertStatus::Acknowledged);
    assert_eq!(updated_alert.acknowledged_by, Some(acknowledger_id));

    // Test 3: Position limit alert
    let position_alert = alert_manager
        .check_and_create_alert(
            AlertCategory::PositionLimit,
            150000.0, // $150k position, exceeds $100k medium threshold
            Some(user_id),
            None,
        )
        .await
        .unwrap();

    assert!(position_alert.is_some());
    let position_alert = position_alert.unwrap();
    assert_eq!(position_alert.category, AlertCategory::PositionLimit);
    assert_eq!(position_alert.severity, AlertSeverity::Medium);

    // Test 4: Liquidity risk alert
    let liquidity_alert = alert_manager
        .check_and_create_alert(
            AlertCategory::LiquidityRisk,
            750000.0, // $750k liquidity, below $1M medium threshold
            None,
            None,
        )
        .await
        .unwrap();

    assert!(liquidity_alert.is_some());
    let liquidity_alert = liquidity_alert.unwrap();
    assert_eq!(liquidity_alert.category, AlertCategory::LiquidityRisk);
    assert_eq!(liquidity_alert.severity, AlertSeverity::Medium);

    // Test 5: Alert resolution
    alert_manager.resolve_alert(liquidity_alert.id).await.unwrap();
    let resolved_alert = alert_manager.get_alert(liquidity_alert.id).await.unwrap();
    assert_eq!(resolved_alert.status, bralaladex_backend::risk_management::alert_management::AlertStatus::Resolved);

    // Test 6: Get active alerts
    let active_alerts = alert_manager.get_active_alerts().await;
    assert_eq!(active_alerts.len(), 1); // Only position alert should be active

    // Test 7: Get user alerts
    let user_alerts = alert_manager.get_alerts_by_user(user_id).await;
    assert_eq!(user_alerts.len(), 2); // Risk and position alerts for this user

    // Test 8: Get category alerts
    let risk_alerts = alert_manager
        .get_alerts_by_category(AlertCategory::RiskThreshold)
        .await;
    assert_eq!(risk_alerts.len(), 1);

    // Test 9: Alert statistics
    let stats = alert_manager.get_alert_statistics().await;
    assert_eq!(stats.total_alerts, 3);
    assert_eq!(stats.active_alerts, 1);
    assert_eq!(stats.acknowledged_alerts, 1);
    assert_eq!(stats.resolved_alerts, 1);
    assert_eq!(stats.high_alerts, 1);
    assert_eq!(stats.medium_alerts, 2);

    // Test 10: Integration monitoring
    let trade_id = Uuid::new_v4();
    integration
        .monitor_trade_execution(
            trade_id,
            Some(user_id),
            0.06, // 6% price impact, exceeds 5% high threshold
            0.005, // 0.5% slippage, below 1% threshold
            150.0, // 150 gwei, exceeds 100 gwei medium threshold
        )
        .await
        .unwrap();

    // Should have created price impact and gas price alerts
    let final_stats = alert_manager.get_alert_statistics().await;
    assert!(final_stats.total_alerts > stats.total_alerts);

    println!("✅ Complete Alert Management System Test Passed");
    println!("📊 Final Statistics: {:?}", final_stats);
}

#[tokio::test]
async fn test_escalation_system() {
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_config = AlertManagerConfig {
        escalation_check_interval_seconds: 1, // Fast escalation for testing
        ..AlertManagerConfig::default()
    };
    
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));

    // Create critical alert that should escalate quickly
    let critical_alert = alert_manager
        .check_and_create_alert(
            AlertCategory::SystemHealth,
            0.75, // 75% health, below 80% critical threshold
            None,
            None,
        )
        .await
        .unwrap();

    assert!(critical_alert.is_some());
    let alert = critical_alert.unwrap();
    assert_eq!(alert.severity, AlertSeverity::Critical);
    assert_eq!(alert.escalation_level, 0);

    println!("✅ Escalation System Test Setup Complete");
    println!("🚨 Created critical alert: {}", alert.id);
}

#[tokio::test]
async fn test_notification_channels() {
    let mut notification_config = NotificationConfig::default();
    notification_config.websocket_enabled = true;
    notification_config.retry_attempts = 2;
    
    let notification_manager = NotificationManager::new(notification_config);
    
    let alert = RiskAlert::new(
        AlertCategory::SlippageExceeded,
        AlertSeverity::Medium,
        "Test Slippage Alert".to_string(),
        "Testing notification channels".to_string(),
        0.01,
        0.025,
    );

    // Test WebSocket notification
    let websocket_notification = notification_manager
        .send_notification(
            &alert,
            bralaladex_backend::risk_management::alert_management::NotificationChannel::WebSocket,
            "test_user",
        )
        .await;

    assert!(websocket_notification.is_ok());
    let notification = websocket_notification.unwrap();
    assert_eq!(notification.channel, bralaladex_backend::risk_management::alert_management::NotificationChannel::WebSocket);
    assert_eq!(notification.delivery_status, bralaladex_backend::risk_management::alert_management::DeliveryStatus::Delivered);

    println!("✅ Notification Channels Test Passed");
    println!("📨 WebSocket notification delivered successfully");
}

#[tokio::test]
async fn test_threshold_configuration() {
    let mut threshold_config = ThresholdConfig::new();
    let user_id = Uuid::new_v4();

    // Test custom user threshold
    let custom_threshold = bralaladex_backend::risk_management::alert_management::AlertThreshold {
        id: Uuid::new_v4(),
        category: AlertCategory::RiskThreshold,
        severity: AlertSeverity::High,
        threshold_value: 0.03, // 3% instead of default 5%
        comparison_operator: bralaladex_backend::risk_management::alert_management::ComparisonOperator::GreaterThan,
        enabled: true,
        user_id: Some(user_id),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    threshold_config
        .set_user_threshold(user_id, custom_threshold)
        .unwrap();

    // Test threshold evaluation
    assert!(threshold_config.should_trigger_alert(
        &AlertCategory::RiskThreshold,
        &AlertSeverity::High,
        0.04, // 4% risk
        Some(user_id)
    )); // Should trigger with custom 3% threshold

    assert!(!threshold_config.should_trigger_alert(
        &AlertCategory::RiskThreshold,
        &AlertSeverity::High,
        0.04, // 4% risk
        None
    )); // Should not trigger with global 5% threshold

    // Test threshold update
    threshold_config
        .update_global_threshold(AlertCategory::GasPrice, AlertSeverity::High, 250.0)
        .unwrap();

    let updated_threshold = threshold_config
        .get_threshold(&AlertCategory::GasPrice, &AlertSeverity::High, None);
    assert!(updated_threshold.is_some());
    assert_eq!(updated_threshold.unwrap().threshold_value, 250.0);

    println!("✅ Threshold Configuration Test Passed");
    println!("⚙️ Custom user thresholds and global updates working correctly");
}

#[tokio::test]
async fn test_real_world_scenario() {
    // Simulate a real trading scenario with multiple risk factors
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_config = AlertManagerConfig::default();
    
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));

    let risk_engine = Arc::new(RwLock::new(RiskEngine::new()));
    let trade_streamer = Arc::new(TradeEventStreamer::new());
    
    let integration = Arc::new(RiskAlertIntegration::new(
        alert_manager.clone(),
        risk_engine.clone(),
        trade_streamer.clone(),
    ));

    let trader_id = Uuid::new_v4();

    // Scenario: Large trade with high risk
    let trade_id = Uuid::new_v4();
    
    // 1. High price impact trade
    integration
        .monitor_trade_execution(
            trade_id,
            Some(trader_id),
            0.08, // 8% price impact - very high
            0.025, // 2.5% slippage - high
            180.0, // 180 gwei - high gas
        )
        .await
        .unwrap();

    // 2. Portfolio risk increase
    alert_manager
        .check_and_create_alert(
            AlertCategory::RiskThreshold,
            0.12, // 12% portfolio risk - critical
            Some(trader_id),
            None,
        )
        .await
        .unwrap();

    // 3. Large position alert
    alert_manager
        .check_and_create_alert(
            AlertCategory::PositionLimit,
            750000.0, // $750k position - high
            Some(trader_id),
            None,
        )
        .await
        .unwrap();

    // 4. System health degradation
    integration.monitor_system_health(0.82).await.unwrap(); // 82% health - medium alert

    // 5. Transaction failure
    integration
        .handle_failed_transaction(
            trade_id,
            Some(trader_id),
            "Insufficient liquidity for large swap",
        )
        .await
        .unwrap();

    // Check final state
    let trader_alerts = alert_manager.get_alerts_by_user(trader_id).await;
    let stats = alert_manager.get_alert_statistics().await;

    println!("✅ Real-World Scenario Test Complete");
    println!("👤 Trader {} has {} alerts", trader_id, trader_alerts.len());
    println!("📊 System Statistics:");
    println!("   - Total Alerts: {}", stats.total_alerts);
    println!("   - Critical Alerts: {}", stats.critical_alerts);
    println!("   - High Alerts: {}", stats.high_alerts);
    println!("   - Medium Alerts: {}", stats.medium_alerts);
    println!("   - Active Alerts: {}", stats.active_alerts);

    // Verify we have alerts for all risk categories
    assert!(stats.total_alerts >= 6); // At least 6 alerts from the scenario
    assert!(stats.critical_alerts >= 1); // Portfolio risk
    assert!(stats.high_alerts >= 3); // Price impact, position, gas price
    assert!(trader_alerts.len() >= 5); // All trader-specific alerts
}

#[tokio::test]
async fn test_alert_subscription_system() {
    let threshold_config = ThresholdConfig::new();
    let notification_config = NotificationConfig::default();
    let notification_manager = NotificationManager::new(notification_config);
    let alert_config = AlertManagerConfig::default();
    
    let alert_manager = Arc::new(AlertManager::new(
        alert_config,
        threshold_config,
        notification_manager,
    ));

    // Subscribe to alerts
    let mut alert_receiver = alert_manager.subscribe_to_alerts();
    let mut notification_receiver = alert_manager.subscribe_to_notifications();

    // Create alert in background task
    let manager_clone = alert_manager.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        manager_clone
            .check_and_create_alert(
                AlertCategory::LiquidityRisk,
                400000.0, // $400k liquidity, below $500k high threshold
                None,
                None,
            )
            .await
            .unwrap();
    });

    // Should receive the alert
    let received_alert = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        alert_receiver.recv(),
    )
    .await;

    assert!(received_alert.is_ok());
    let alert = received_alert.unwrap().unwrap();
    assert_eq!(alert.category, AlertCategory::LiquidityRisk);
    assert_eq!(alert.severity, AlertSeverity::High);

    println!("✅ Alert Subscription System Test Passed");
    println!("📡 Real-time alert broadcasting working correctly");
}
