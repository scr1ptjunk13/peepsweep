use std::sync::Arc;
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::Utc;

use bralaladex_backend::user_retention::arbitrage_alerts::{
    detector::{ArbitrageDetector, ArbitrageConfig, TokenPair},
    alert_manager::{AlertManager, UserSubscription, AlertPreferences},
    notification_service::{NotificationService, NotificationChannel},
    executor::ArbitrageExecutor,
};
use bralaladex_backend::aggregator::DexAggregator;
use bralaladex_backend::mev_protection::MevProtectionSuite;

#[tokio::test]
async fn test_arbitrage_detection_basic() {
    // Initialize components
    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let detector = Arc::new(ArbitrageDetector::new(
        dex_aggregator.clone(),
        ArbitrageConfig::default(),
    ));

    // Create token pair
    let token_pair = TokenPair {
        base_token: "ETH".to_string(),
        quote_token: "USDC".to_string(),
        base_token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        quote_token_address: "0xA0b86a33E6441E6C7D3A8E7C3E8E0A0C8B8A8B8A".to_string(),
    };

    // Update prices to create arbitrage opportunity
    detector.update_price("Uniswap", &token_pair, Decimal::from(3400)).await.unwrap();
    detector.update_price("Curve", &token_pair, Decimal::from(3468)).await.unwrap();

    // Wait for detection
    sleep(Duration::from_millis(100)).await;

    // Check opportunities
    let opportunities = detector.get_opportunities().await;
    assert!(!opportunities.is_empty(), "Should detect arbitrage opportunity");

    let opportunity = &opportunities[0];
    assert_eq!(opportunity.token_pair.base_token, "ETH");
    assert!(opportunity.profit_percentage > Decimal::ZERO);
    assert!(opportunity.estimated_profit_usd > Decimal::ZERO);

    println!("✅ Arbitrage detection test passed");
    println!("   - Detected opportunity: ${:.2} profit", opportunity.estimated_profit_usd.to_f64().unwrap_or(0.0));
    println!("   - Profit percentage: {:.2}%", (opportunity.profit_percentage * Decimal::from(100)).to_f64().unwrap_or(0.0));
}

#[tokio::test]
async fn test_alert_manager_subscription() {
    let alert_manager = Arc::new(AlertManager::new());
    let user_id = Uuid::new_v4();

    // Subscribe user
    let subscription = UserSubscription {
        user_id,
        alert_preferences: AlertPreferences {
            min_profit_usd: Decimal::from(100),
            max_risk_level: "Medium".to_string(),
            preferred_tokens: vec!["ETH".to_string(), "USDC".to_string()],
            notification_channels: vec![NotificationChannel::WebSocket],
            enabled: true,
        },
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let result = alert_manager.subscribe_user(subscription).await;
    assert!(result.is_ok(), "Should successfully subscribe user");

    // Check subscription exists
    let subscriptions = alert_manager.get_user_subscriptions(user_id).await.unwrap();
    assert_eq!(subscriptions.len(), 1);
    assert_eq!(subscriptions[0].user_id, user_id);

    println!("✅ Alert manager subscription test passed");
    println!("   - User subscribed: {}", user_id);
    println!("   - Preferences: min_profit=${}", subscriptions[0].alert_preferences.min_profit_usd);
}

#[tokio::test]
async fn test_notification_service_basic() {
    let notification_service = Arc::new(NotificationService::new().await.unwrap());
    let user_id = Uuid::new_v4();

    // Register user for WebSocket
    let result = notification_service.register_websocket_user(user_id, "ws_connection_123".to_string()).await;
    assert!(result.is_ok(), "Should register WebSocket user");

    // Register user for email
    let result = notification_service.register_email_user(user_id, "test@example.com".to_string()).await;
    assert!(result.is_ok(), "Should register email user");

    println!("✅ Notification service test passed");
    println!("   - WebSocket registration: ✓");
    println!("   - Email registration: ✓");
}

#[tokio::test]
async fn test_execution_plan_creation() {
    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let mev_protection = Arc::new(MevProtectionSuite::new().await.unwrap());
    let executor = Arc::new(ArbitrageExecutor::new(dex_aggregator, mev_protection));

    // Create mock opportunity
    let opportunity = bralaladex_backend::user_retention::arbitrage_alerts::detector::ArbitrageOpportunity {
        id: Uuid::new_v4(),
        token_pair: TokenPair {
            base_token: "ETH".to_string(),
            quote_token: "USDC".to_string(),
            base_token_address: "0x123".to_string(),
            quote_token_address: "0x456".to_string(),
        },
        source_dex: "Uniswap".to_string(),
        target_dex: "Curve".to_string(),
        source_price: Decimal::from(3400),
        target_price: Decimal::from(3468),
        price_difference: Decimal::from(68),
        profit_percentage: Decimal::from_str("0.02").unwrap(),
        estimated_profit_usd: Decimal::from(680),
        estimated_gas_cost: Decimal::from(50),
        net_profit_usd: Decimal::from(630),
        liquidity_available: Decimal::from(50000),
        execution_time_estimate: 15000,
        confidence_score: 0.85,
        detected_at: Utc::now(),
        expires_at: Utc::now() + chrono::Duration::minutes(5),
        chain_id: 1,
    };

    let execution_amount = Decimal::from(10); // 10 ETH
    let plan = executor.create_execution_plan(&opportunity, execution_amount).await.unwrap();

    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].step_number, 1);
    assert_eq!(plan.steps[1].step_number, 2);
    assert!(plan.total_estimated_gas > 0);
    assert!(plan.total_estimated_profit > Decimal::ZERO);

    println!("✅ Execution plan creation test passed");
    println!("   - Steps: {}", plan.steps.len());
    println!("   - Estimated gas: {}", plan.total_estimated_gas);
    println!("   - Estimated profit: ${:.2}", plan.total_estimated_profit.to_f64().unwrap_or(0.0));
}

#[tokio::test]
async fn test_complete_arbitrage_workflow() {
    // Initialize all components
    let dex_aggregator = Arc::new(DexAggregator::new().await.unwrap());
    let mev_protection = Arc::new(MevProtectionSuite::new().await.unwrap());
    
    let detector = Arc::new(ArbitrageDetector::new(
        dex_aggregator.clone(),
        ArbitrageConfig::default(),
    ));
    let alert_manager = Arc::new(AlertManager::new());
    let notification_service = Arc::new(NotificationService::new().await.unwrap());
    let executor = Arc::new(ArbitrageExecutor::new(dex_aggregator.clone(), mev_protection));

    // Create and subscribe user
    let user_id = Uuid::new_v4();
    let subscription = UserSubscription {
        user_id,
        alert_preferences: AlertPreferences {
            min_profit_usd: Decimal::from(100),
            max_risk_level: "Medium".to_string(),
            preferred_tokens: vec!["ETH".to_string(), "USDC".to_string()],
            notification_channels: vec![NotificationChannel::WebSocket],
            enabled: true,
        },
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    alert_manager.subscribe_user(subscription).await.unwrap();

    // Register for notifications
    notification_service.register_websocket_user(user_id, "ws_123".to_string()).await.unwrap();

    // Create arbitrage opportunity
    let token_pair = TokenPair {
        base_token: "ETH".to_string(),
        quote_token: "USDC".to_string(),
        base_token_address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
        quote_token_address: "0xA0b86a33E6441E6C7D3A8E7C3E8E0A0C8B8A8B8A".to_string(),
    };

    detector.update_price("Uniswap", &token_pair, Decimal::from(3400)).await.unwrap();
    detector.update_price("Curve", &token_pair, Decimal::from(3468)).await.unwrap();

    // Wait for detection
    sleep(Duration::from_millis(100)).await;

    // Verify opportunity detected
    let opportunities = detector.get_opportunities().await;
    assert!(!opportunities.is_empty(), "Should detect opportunity");

    // Process opportunity through alert system
    let opportunity = &opportunities[0];
    alert_manager.process_opportunity(opportunity).await.unwrap();

    // Check alerts created
    let alerts = alert_manager.get_user_alerts(user_id).await.unwrap();
    assert!(!alerts.is_empty(), "Should create alert");

    // Create execution plan
    let execution_amount = Decimal::from(5); // 5 ETH
    let plan = executor.create_execution_plan(opportunity, execution_amount).await.unwrap();
    assert!(plan.total_estimated_profit > Decimal::ZERO);

    println!("✅ Complete arbitrage workflow test passed");
    println!("   - Opportunity detected: ✓");
    println!("   - User subscribed: ✓");
    println!("   - Alert created: ✓");
    println!("   - Execution plan created: ✓");
    println!("   - Estimated profit: ${:.2}", plan.total_estimated_profit.to_f64().unwrap_or(0.0));
}
