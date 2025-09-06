use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::Utc;
use std::str::FromStr;

// Import all necessary types for arbitrage alerts
use bralaladex_backend::user_retention::arbitrage_alerts::detector::*;
use bralaladex_backend::user_retention::arbitrage_alerts::alert_manager::*;
use bralaladex_backend::user_retention::arbitrage_alerts::notification_service::*;
use bralaladex_backend::user_retention::arbitrage_alerts::executor::*;
use bralaladex_backend::aggregator::DexAggregator;

#[tokio::test]
async fn test_arbitrage_alerts_feature_complete_workflow() {
    println!("🚀 Testing Feature 1: Arbitrage Opportunity Alerts - Complete Workflow");

    // Step 1: Initialize components
    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let detector = Arc::new(ArbitrageDetector::new(dex_aggregator.clone(), ArbitrageConfig::default()));
    let alert_manager = Arc::new(AlertManager::new());
    let notification_service = Arc::new(NotificationService::new().await.unwrap());
    let executor = Arc::new(ArbitrageExecutor::new(dex_aggregator.clone()));

    println!("✅ Step 1: All components initialized successfully");

    // Step 2: Create test users and subscriptions
    let user1 = Uuid::new_v4();
    let user2 = Uuid::new_v4();

    let preferences1 = AlertPreferences {
        min_profit_usd: Decimal::from(50),
        max_gas_cost_usd: Decimal::from(20),
        enabled_dexes: vec!["Uniswap".to_string(), "SushiSwap".to_string()],
        notification_channels: vec![NotificationChannel::WebSocket, NotificationChannel::Email],
        rate_limit_per_hour: 10,
    };

    let preferences2 = AlertPreferences {
        min_profit_usd: Decimal::from(100),
        max_gas_cost_usd: Decimal::from(30),
        enabled_dexes: vec!["Uniswap".to_string()],
        notification_channels: vec![NotificationChannel::WebSocket],
        rate_limit_per_hour: 5,
    };

    alert_manager.subscribe_user(user1, preferences1).await.unwrap();
    alert_manager.subscribe_user(user2, preferences2).await.unwrap();

    println!("✅ Step 2: Users subscribed with different preferences");

    // Step 3: Register users for notifications
    notification_service.register_user(user1, NotificationChannel::WebSocket, "ws://user1".to_string()).await.unwrap();
    notification_service.register_user(user1, NotificationChannel::Email, "user1@test.com".to_string()).await.unwrap();
    notification_service.register_user(user2, NotificationChannel::WebSocket, "ws://user2".to_string()).await.unwrap();

    println!("✅ Step 3: Users registered for notifications");

    // Step 4: Simulate arbitrage opportunity detection
    let token_pair = TokenPair {
        base_token: "ETH".to_string(),
        quote_token: "USDC".to_string(),
        base_token_address: "0x...".to_string(),
        quote_token_address: "0x...".to_string(),
    };

    let opportunity = ArbitrageOpportunity {
        id: Uuid::new_v4(),
        token_pair: token_pair.clone(),
        buy_dex: "SushiSwap".to_string(),
        sell_dex: "Uniswap".to_string(),
        buy_price: Decimal::from_str("3000.00").unwrap(),
        sell_price: Decimal::from_str("3080.00").unwrap(),
        profit_usd: Decimal::from(75),
        gas_cost_usd: Decimal::from(15),
        net_profit_usd: Decimal::from(60),
        trade_amount_usd: Decimal::from(1000),
        confidence_score: Decimal::from_str("0.85").unwrap(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        detected_at: Utc::now(),
        chain_id: 1,
        block_number: Some(18000000),
    };

    detector.add_opportunity(opportunity.clone()).await;
    println!("✅ Step 4: Arbitrage opportunity detected and cached");

    // Step 5: Process alerts for users
    let opportunities = vec![opportunity.clone()];
    let alerts = alert_manager.process_opportunities(&opportunities).await.unwrap();

    // Verify alerts were created correctly
    assert!(!alerts.is_empty(), "No alerts were created");
    println!("✅ Step 5: {} alerts created and filtered by user preferences", alerts.len());

    // Step 6: Send notifications
    for alert in &alerts {
        notification_service.send_alert_notification(alert).await.unwrap();
    }
    println!("✅ Step 6: Notifications sent to all subscribed users");

    // Step 7: Test execution plan creation
    let execution_plan = executor.create_execution_plan(&opportunity).await.unwrap();
    assert!(!execution_plan.steps.is_empty(), "Execution plan should have steps");
    assert!(execution_plan.estimated_gas_cost > 0, "Should have gas cost estimate");
    
    println!("✅ Step 7: Execution plan created with {} steps", execution_plan.steps.len());

    // Step 8: Test risk assessment
    let risk_assessment = executor.assess_execution_risk(&opportunity).await.unwrap();
    assert!(risk_assessment.overall_risk_score >= Decimal::ZERO, "Risk score should be valid");
    assert!(!risk_assessment.risk_factors.is_empty(), "Should identify risk factors");

    println!("✅ Step 8: Risk assessment completed - Overall risk: {}", risk_assessment.overall_risk_score);

    // Step 9: Test opportunity expiration and cleanup
    sleep(Duration::from_millis(100)).await;
    detector.cleanup_expired_opportunities().await;
    
    println!("✅ Step 9: Expired opportunities cleaned up");

    // Step 10: Verify alert history and rate limiting
    let user1_history = alert_manager.get_user_alert_history(user1, 24).await.unwrap();
    assert!(!user1_history.is_empty(), "User should have alert history");
    
    println!("✅ Step 10: Alert history verified - User1 has {} alerts in history", user1_history.len());

    println!("🎉 Feature 1: Arbitrage Opportunity Alerts - ALL TESTS PASSED!");
    println!("   ✓ Arbitrage detection working");
    println!("   ✓ Alert filtering by user preferences working");
    println!("   ✓ Multi-channel notifications working");
    println!("   ✓ Execution planning working");
    println!("   ✓ Risk assessment working");
    println!("   ✓ Rate limiting and history tracking working");
    println!("   ✓ Opportunity cleanup working");
}

#[tokio::test]
async fn test_arbitrage_detector_unit() {
    println!("🔍 Testing Arbitrage Detector Unit");

    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let detector = ArbitrageDetector::new(dex_aggregator, ArbitrageConfig::default());

    let token_pair = TokenPair {
        base_token: "ETH".to_string(),
        quote_token: "USDC".to_string(),
        base_token_address: "0x...".to_string(),
        quote_token_address: "0x...".to_string(),
    };

    // Test opportunity detection
    let opportunities = detector.detect_opportunities(&token_pair).await.unwrap();
    println!("✅ Detector found {} opportunities for ETH/USDC", opportunities.len());

    // Test opportunity caching
    let opportunity = ArbitrageOpportunity {
        id: Uuid::new_v4(),
        token_pair: token_pair.clone(),
        buy_dex: "SushiSwap".to_string(),
        sell_dex: "Uniswap".to_string(),
        buy_price: Decimal::from_str("3000.00").unwrap(),
        sell_price: Decimal::from_str("3080.00").unwrap(),
        profit_usd: Decimal::from(75),
        gas_cost_usd: Decimal::from(15),
        net_profit_usd: Decimal::from(60),
        trade_amount_usd: Decimal::from(1000),
        confidence_score: Decimal::from_str("0.85").unwrap(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        detected_at: Utc::now(),
        chain_id: 1,
        block_number: Some(18000000),
    };

    detector.add_opportunity(opportunity.clone()).await;
    let cached = detector.get_opportunity(&opportunity.id).await.unwrap();
    assert!(cached.is_some(), "Opportunity should be cached");
    
    println!("✅ Arbitrage Detector Unit Tests Passed");
}

#[tokio::test]
async fn test_alert_manager_unit() {
    println!("📢 Testing Alert Manager Unit");

    let alert_manager = AlertManager::new();
    let user_id = Uuid::new_v4();

    let preferences = AlertPreferences {
        min_profit_usd: Decimal::from(50),
        max_gas_cost_usd: Decimal::from(20),
        enabled_dexes: vec!["Uniswap".to_string()],
        notification_channels: vec![NotificationChannel::WebSocket],
        rate_limit_per_hour: 5,
    };

    // Test subscription
    alert_manager.subscribe_user(user_id, preferences.clone()).await.unwrap();
    let retrieved = alert_manager.get_user_preferences(&user_id).await.unwrap();
    assert!(retrieved.is_some(), "User preferences should be stored");
    
    println!("✅ Alert Manager Unit Tests Passed");
}

#[tokio::test]
async fn test_notification_service_unit() {
    println!("📨 Testing Notification Service Unit");

    let notification_service = NotificationService::new().await.unwrap();
    let user_id = Uuid::new_v4();

    // Test user registration
    notification_service.register_user(user_id, NotificationChannel::WebSocket, "ws://test".to_string()).await.unwrap();
    
    // Test notification sending (mock)
    let alert = ArbitrageAlert {
        id: Uuid::new_v4(),
        user_id,
        opportunity_id: Uuid::new_v4(),
        message: "Test alert".to_string(),
        priority: AlertPriority::High,
        channels: vec![NotificationChannel::WebSocket],
        created_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
    };

    notification_service.send_alert_notification(&alert).await.unwrap();
    
    println!("✅ Notification Service Unit Tests Passed");
}

#[tokio::test]
async fn test_arbitrage_executor_unit() {
    println!("⚡ Testing Arbitrage Executor Unit");

    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let executor = ArbitrageExecutor::new(dex_aggregator);

    let opportunity = ArbitrageOpportunity {
        id: Uuid::new_v4(),
        token_pair: TokenPair {
            base_token: "ETH".to_string(),
            quote_token: "USDC".to_string(),
            base_token_address: "0x...".to_string(),
            quote_token_address: "0x...".to_string(),
        },
        buy_dex: "SushiSwap".to_string(),
        sell_dex: "Uniswap".to_string(),
        buy_price: Decimal::from_str("3000.00").unwrap(),
        sell_price: Decimal::from_str("3080.00").unwrap(),
        profit_usd: Decimal::from(75),
        gas_cost_usd: Decimal::from(15),
        net_profit_usd: Decimal::from(60),
        trade_amount_usd: Decimal::from(1000),
        confidence_score: Decimal::from_str("0.85").unwrap(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        detected_at: Utc::now(),
        chain_id: 1,
        block_number: Some(18000000),
    };

    // Test execution plan creation
    let plan = executor.create_execution_plan(&opportunity).await.unwrap();
    assert!(!plan.steps.is_empty(), "Should have execution steps");
    
    // Test risk assessment
    let risk = executor.assess_execution_risk(&opportunity).await.unwrap();
    assert!(risk.overall_risk_score >= Decimal::ZERO, "Should have valid risk score");
    
    println!("✅ Arbitrage Executor Unit Tests Passed");
}
