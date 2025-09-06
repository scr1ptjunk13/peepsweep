use crate::analytics::gas_usage_tracker::*;
use crate::analytics::gas_optimization_analyzer::*;
use crate::risk_management::types::{UserId, RiskError};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use tokio;

#[tokio::test]
async fn test_gas_optimization_analyzer_creation() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor, oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker,
        route_analyzer,
        optimization_engine,
    );
    
    // Test that analyzer was created successfully
    assert!(true); // If we get here, creation succeeded
}

#[tokio::test]
async fn test_generate_optimization_insights() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor.clone(), oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker.clone(),
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    // Add some test data
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    usage_tracker.track_transaction(
        tx_hash.clone(),
        user_id,
        trade_id,
        21000,
        Decimal::from(30), // Higher gas price for inefficiency
        Decimal::from(1000),
        "Uniswap V3".to_string(),
        "direct".to_string(),
        "ETH/USDC".to_string(),
    ).await.unwrap();
    
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 25000, // Higher gas usage
        gas_price: Decimal::from(30),
        status: TransactionStatus::Confirmed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    usage_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    // Generate insights
    let insights = analyzer.generate_optimization_insights(user_id, 7).await.unwrap();
    
    assert_eq!(insights.user_id, user_id);
    assert!(insights.current_efficiency_score >= Decimal::ZERO);
    assert!(insights.current_efficiency_score <= Decimal::from(100));
    assert!(insights.potential_savings_usd >= Decimal::ZERO);
    assert!(!insights.recommendations.is_empty());
    assert!(!insights.inefficient_routes.is_empty());
    assert!(!insights.optimal_timing_windows.is_empty());
    assert!(insights.batch_opportunity_savings > Decimal::ZERO);
}

#[tokio::test]
async fn test_get_immediate_recommendations() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor.clone(), oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker.clone(),
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    // Add test transaction with high savings potential
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    usage_tracker.track_transaction(
        tx_hash.clone(),
        user_id,
        trade_id,
        21000,
        Decimal::from(50), // Very high gas price
        Decimal::from(100), // Small trade value for high efficiency ratio
        "Expensive DEX".to_string(),
        "multi-hop".to_string(),
        "ETH/USDC".to_string(),
    ).await.unwrap();
    
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 30000,
        gas_price: Decimal::from(50),
        status: TransactionStatus::Confirmed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    usage_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    let recommendations = analyzer.get_immediate_recommendations(user_id).await.unwrap();
    
    // Should have at least one easy-to-implement recommendation
    assert!(!recommendations.is_empty());
    
    for rec in &recommendations {
        assert!(matches!(rec.implementation_difficulty, DifficultyLevel::Easy));
        assert!(rec.potential_savings_usd >= Decimal::from(5));
    }
}

#[tokio::test]
async fn test_calculate_total_savings_potential() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor, oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker,
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    let recommendations = vec![
        GasOptimizationRecommendation {
            recommendation_id: Uuid::new_v4().to_string(),
            recommendation_type: RecommendationType::RouteOptimization,
            title: "Test Recommendation 1".to_string(),
            description: "Test description".to_string(),
            potential_savings_usd: Decimal::from(50),
            confidence_score: Decimal::from_str("0.8").unwrap(),
            implementation_difficulty: DifficultyLevel::Easy,
            estimated_impact: ImpactLevel::Medium,
            supporting_data: vec![],
        },
        GasOptimizationRecommendation {
            recommendation_id: Uuid::new_v4().to_string(),
            recommendation_type: RecommendationType::TimingOptimization,
            title: "Test Recommendation 2".to_string(),
            description: "Test description".to_string(),
            potential_savings_usd: Decimal::from(30),
            confidence_score: Decimal::from_str("0.6").unwrap(),
            implementation_difficulty: DifficultyLevel::Medium,
            estimated_impact: ImpactLevel::Low,
            supporting_data: vec![],
        },
    ];
    
    let total_savings = analyzer.calculate_total_savings_potential(user_id, &recommendations).await.unwrap();
    
    // Should be confidence-weighted sum: (50 * 0.8) + (30 * 0.6) = 40 + 18 = 58
    let expected = Decimal::from(58);
    assert_eq!(total_savings, expected);
}

#[tokio::test]
async fn test_efficiency_score_calculation() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor.clone(), oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker.clone(),
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    // Add efficient transaction
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    usage_tracker.track_transaction(
        tx_hash.clone(),
        user_id,
        trade_id,
        21000,
        Decimal::from(20), // Low gas price
        Decimal::from(10000), // High trade value
        "Efficient DEX".to_string(),
        "direct".to_string(),
        "ETH/USDC".to_string(),
    ).await.unwrap();
    
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 18000, // Low gas usage
        gas_price: Decimal::from(20),
        status: TransactionStatus::Confirmed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    usage_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    let insights = analyzer.generate_optimization_insights(user_id, 7).await.unwrap();
    
    // Should have high efficiency score due to low gas cost relative to trade value
    assert!(insights.current_efficiency_score > Decimal::from(50));
}

#[tokio::test]
async fn test_mock_route_analyzer() {
    let analyzer = MockRouteGasAnalyzer;
    
    // Create test records
    let records = vec![
        GasUsageRecord {
            transaction_hash: "0x1".to_string(),
            user_id: Uuid::new_v4(),
            trade_id: Uuid::new_v4(),
            gas_used: 21000,
            gas_price: Decimal::from(25),
            gas_cost_eth: Decimal::from_str("0.000525").unwrap(),
            gas_cost_usd: Decimal::from_str("1.68").unwrap(),
            trade_value_usd: Decimal::from(1000),
            gas_efficiency: Decimal::from_str("0.00168").unwrap(),
            dex_name: "Uniswap V3".to_string(),
            route_type: "direct".to_string(),
            token_pair: "ETH/USDC".to_string(),
            timestamp: Utc::now(),
            block_number: 18_500_000,
            transaction_status: TransactionStatus::Confirmed,
        },
        GasUsageRecord {
            transaction_hash: "0x2".to_string(),
            user_id: Uuid::new_v4(),
            trade_id: Uuid::new_v4(),
            gas_used: 35000,
            gas_price: Decimal::from(30),
            gas_cost_eth: Decimal::from_str("0.00105").unwrap(),
            gas_cost_usd: Decimal::from_str("3.36").unwrap(),
            trade_value_usd: Decimal::from(1000),
            gas_efficiency: Decimal::from_str("0.00336").unwrap(),
            dex_name: "Curve".to_string(),
            route_type: "multi-hop".to_string(),
            token_pair: "ETH/USDC".to_string(),
            timestamp: Utc::now(),
            block_number: 18_500_001,
            transaction_status: TransactionStatus::Confirmed,
        },
    ];
    
    let analysis = analyzer.analyze_route_efficiency(&records).await.unwrap();
    
    assert_eq!(analysis.len(), 2);
    
    // Check that different routes have different efficiency scores
    let uniswap_analysis = analysis.iter().find(|a| a.route_identifier.contains("Uniswap")).unwrap();
    let curve_analysis = analysis.iter().find(|a| a.route_identifier.contains("Curve")).unwrap();
    
    assert_ne!(uniswap_analysis.efficiency_score, curve_analysis.efficiency_score);
    assert!(uniswap_analysis.success_rate > Decimal::ZERO);
    assert!(curve_analysis.success_rate > Decimal::ZERO);
}

#[tokio::test]
async fn test_mock_optimization_engine() {
    let engine = MockGasOptimizationEngine;
    let user_id = Uuid::new_v4();
    
    // Test timing patterns
    let timing_patterns = engine.analyze_timing_patterns(user_id).await.unwrap();
    assert_eq!(timing_patterns.len(), 2);
    assert_eq!(timing_patterns[0].hour_of_day, 3);
    assert_eq!(timing_patterns[0].day_of_week, 2);
    assert!(timing_patterns[0].potential_savings_percent > Decimal::ZERO);
    
    // Test batch savings
    let batch_savings = engine.calculate_batch_savings_potential(user_id).await.unwrap();
    assert_eq!(batch_savings, Decimal::from(35));
    
    // Test gas price strategy
    let strategy = engine.generate_gas_price_strategy(user_id).await.unwrap();
    assert!(matches!(strategy.strategy_type, GasPriceStrategyType::Balanced));
    assert!(matches!(strategy.risk_level, RiskLevel::Low));
    assert!(strategy.recommended_gas_price_multiplier > Decimal::ONE);
    
    // Test savings prediction
    let recommendations = vec![
        GasOptimizationRecommendation {
            recommendation_id: Uuid::new_v4().to_string(),
            recommendation_type: RecommendationType::RouteOptimization,
            title: "Test".to_string(),
            description: "Test".to_string(),
            potential_savings_usd: Decimal::from(100),
            confidence_score: Decimal::from_str("0.8").unwrap(),
            implementation_difficulty: DifficultyLevel::Easy,
            estimated_impact: ImpactLevel::High,
            supporting_data: vec![],
        }
    ];
    
    let predicted_savings = engine.predict_gas_savings(user_id, &recommendations).await.unwrap();
    assert_eq!(predicted_savings, Decimal::from(80)); // 100 * 0.8
}

#[tokio::test]
async fn test_recommendation_generation_with_failed_transactions() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor.clone(), oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker.clone(),
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    // Add failed transaction
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    usage_tracker.track_transaction(
        tx_hash.clone(),
        user_id,
        trade_id,
        21000,
        Decimal::from(25),
        Decimal::from(1000),
        "Uniswap V3".to_string(),
        "direct".to_string(),
        "ETH/USDC".to_string(),
    ).await.unwrap();
    
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 21000,
        gas_price: Decimal::from(25),
        status: TransactionStatus::Failed, // Failed transaction
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    usage_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    let insights = analyzer.generate_optimization_insights(user_id, 7).await.unwrap();
    
    // Should have recommendation about reducing failed transactions
    let failed_tx_rec = insights.recommendations.iter()
        .find(|r| r.title.contains("failed transactions"));
    
    assert!(failed_tx_rec.is_some());
    let rec = failed_tx_rec.unwrap();
    assert!(matches!(rec.recommendation_type, RecommendationType::GasPriceStrategy));
    assert!(matches!(rec.estimated_impact, ImpactLevel::High));
}

#[tokio::test]
async fn test_insufficient_data_handling() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let usage_tracker = Arc::new(GasUsageTracker::new(monitor, oracle, calculator));
    
    let route_analyzer = Arc::new(MockRouteGasAnalyzer);
    let optimization_engine = Arc::new(MockGasOptimizationEngine);
    
    let analyzer = GasOptimizationAnalyzer::new(
        usage_tracker,
        route_analyzer,
        optimization_engine,
    );
    
    let user_id = Uuid::new_v4();
    
    // Try to generate insights with no data
    let result = analyzer.generate_optimization_insights(user_id, 7).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        RiskError::InsufficientData(msg) => {
            assert!(msg.contains("No gas usage data found"));
        },
        _ => panic!("Expected InsufficientData error"),
    }
}
