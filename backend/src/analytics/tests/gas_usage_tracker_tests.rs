use crate::analytics::gas_usage_tracker::*;
use crate::risk_management::types::RiskError;
use rust_decimal::Decimal;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use uuid::Uuid;
use tokio;
use std::str::FromStr;

#[tokio::test]
async fn test_gas_usage_tracker_creation() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    
    let tracker = GasUsageTracker::new(monitor, oracle, calculator);
    let health = tracker.get_health_status().await;
    
    assert!(health.is_operational);
    assert_eq!(health.pending_transactions, 0);
    assert_eq!(health.total_tracked_transactions, 0);
}

#[tokio::test]
async fn test_track_transaction() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    
    let tracker = GasUsageTracker::new(monitor, oracle, calculator);
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    let result = tracker.track_transaction(
        tx_hash.clone(),
        user_id,
        trade_id,
        21000, // gas limit
        Decimal::from(25), // 25 Gwei
        Decimal::from(1000), // $1000 trade
        "Uniswap V3".to_string(),
        "direct".to_string(),
        "ETH/USDC".to_string(),
    ).await;
    
    assert!(result.is_ok());
    
    let health = tracker.get_health_status().await;
    assert_eq!(health.pending_transactions, 1);
}

#[tokio::test]
async fn test_update_transaction_status() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    // Track transaction first
    gas_tracker.track_transaction(
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
    
    // Add mock receipt
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 19500,
        gas_price: Decimal::from(25),
        status: TransactionStatus::Confirmed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    
    // Update status
    let result = gas_tracker.update_transaction_status(&tx_hash).await;
    assert!(result.is_ok());
    
    let health = gas_tracker.get_health_status().await;
    assert_eq!(health.pending_transactions, 0);
    assert_eq!(health.total_tracked_transactions, 1);
}

#[tokio::test]
async fn test_get_user_gas_usage() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    // Track and confirm transaction
    gas_tracker.track_transaction(
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
        gas_used: 19500,
        gas_price: Decimal::from(25),
        status: TransactionStatus::Confirmed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    gas_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    // Get usage data
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(1);
    
    let records = gas_tracker.get_user_gas_usage(user_id, start_date, end_date).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].transaction_hash, tx_hash);
    assert_eq!(records[0].user_id, user_id);
    assert_eq!(records[0].dex_name, "Uniswap V3");
}

#[tokio::test]
async fn test_calculate_gas_efficiency_metrics() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    
    // Add multiple transactions
    for i in 0..3 {
        let trade_id = Uuid::new_v4();
        let tx_hash = format!("0x{:016x}", i);
        
        gas_tracker.track_transaction(
            tx_hash.clone(),
            user_id,
            trade_id,
            21000,
            Decimal::from(25 + i * 5), // Varying gas prices
            Decimal::from(1000),
            "Uniswap V3".to_string(),
            "direct".to_string(),
            "ETH/USDC".to_string(),
        ).await.unwrap();
        
        let receipt = TransactionReceipt {
            transaction_hash: tx_hash.clone(),
            block_number: 18_500_000 + i as u64,
            gas_used: 19500 + i as u64 * 1000,
            gas_price: Decimal::from(25 + i * 5),
            status: TransactionStatus::Confirmed,
            timestamp: Utc::now(),
        };
        
        monitor.add_receipt(tx_hash.clone(), receipt).await;
        gas_tracker.update_transaction_status(&tx_hash).await.unwrap();
    }
    
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(1);
    
    let metrics = gas_tracker.calculate_gas_efficiency_metrics(user_id, start_date, end_date).await.unwrap();
    
    assert_eq!(metrics.transaction_count, 3);
    assert_eq!(metrics.failed_transaction_count, 0);
    assert!(metrics.total_gas_spent_usd > Decimal::ZERO);
    assert!(metrics.average_gas_used > 0);
}

#[tokio::test]
async fn test_gas_efficiency_calculator() {
    let calculator = DefaultGasEfficiencyCalculator::new();
    
    // Test efficiency ratio calculation
    let gas_cost = Decimal::from(50); // $50
    let trade_value = Decimal::from(1000); // $1000
    let efficiency = calculator.calculate_efficiency_ratio(gas_cost, trade_value);
    assert_eq!(efficiency, Decimal::from_str("0.05").unwrap()); // 5%
    
    // Test gas per dollar calculation
    let gas_used = 21000u64;
    let gas_per_dollar = calculator.calculate_gas_per_dollar(gas_used, trade_value);
    assert_eq!(gas_per_dollar, Decimal::from(21)); // 21 gas per dollar
    
    // Test zero trade value handling
    let efficiency_zero = calculator.calculate_efficiency_ratio(gas_cost, Decimal::ZERO);
    assert_eq!(efficiency_zero, Decimal::MAX);
}

#[tokio::test]
async fn test_failed_transaction_tracking() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    let trade_id = Uuid::new_v4();
    let tx_hash = "0x123456789abcdef".to_string();
    
    // Track transaction
    gas_tracker.track_transaction(
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
    
    // Add failed receipt
    let receipt = TransactionReceipt {
        transaction_hash: tx_hash.clone(),
        block_number: 18_500_000,
        gas_used: 21000, // Full gas used on failure
        gas_price: Decimal::from(25),
        status: TransactionStatus::Failed,
        timestamp: Utc::now(),
    };
    
    monitor.add_receipt(tx_hash.clone(), receipt).await;
    gas_tracker.update_transaction_status(&tx_hash).await.unwrap();
    
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(1);
    
    let metrics = gas_tracker.calculate_gas_efficiency_metrics(user_id, start_date, end_date).await.unwrap();
    
    assert_eq!(metrics.transaction_count, 1);
    assert_eq!(metrics.failed_transaction_count, 1);
    assert!(metrics.gas_wasted_on_failures > Decimal::ZERO);
}

#[tokio::test]
async fn test_dex_gas_comparison() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    
    // Add transactions on different DEXs
    let dexs = vec!["Uniswap V3", "Curve", "1inch"];
    
    for (i, dex) in dexs.iter().enumerate() {
        let trade_id = Uuid::new_v4();
        let tx_hash = format!("0x{:016x}", i);
        
        gas_tracker.track_transaction(
            tx_hash.clone(),
            user_id,
            trade_id,
            21000,
            Decimal::from(25),
            Decimal::from(1000),
            dex.to_string(),
            "direct".to_string(),
            "ETH/USDC".to_string(),
        ).await.unwrap();
        
        let receipt = TransactionReceipt {
            transaction_hash: tx_hash.clone(),
            block_number: 18_500_000 + i as u64,
            gas_used: 19500 + i as u64 * 2000, // Different gas usage per DEX
            gas_price: Decimal::from(25),
            status: TransactionStatus::Confirmed,
            timestamp: Utc::now(),
        };
        
        monitor.add_receipt(tx_hash.clone(), receipt).await;
        gas_tracker.update_transaction_status(&tx_hash).await.unwrap();
    }
    
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(1);
    
    let comparison = gas_tracker.get_dex_gas_comparison(user_id, start_date, end_date).await.unwrap();
    
    assert_eq!(comparison.len(), 3);
    
    // Verify each DEX has different efficiency metrics
    let uniswap_route = comparison.iter().find(|r| r.route_identifier.contains("Uniswap")).unwrap();
    let curve_route = comparison.iter().find(|r| r.route_identifier.contains("Curve")).unwrap();
    
    assert_ne!(uniswap_route.average_gas_used, curve_route.average_gas_used);
}

#[tokio::test]
async fn test_gas_price_recommendations() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    
    let tracker = GasUsageTracker::new(monitor, oracle, calculator);
    
    let recommendations = tracker.get_gas_price_recommendations().await.unwrap();
    
    assert_eq!(recommendations.slow, Decimal::from(20));
    assert_eq!(recommendations.standard, Decimal::from(25));
    assert_eq!(recommendations.fast, Decimal::from(30));
    assert_eq!(recommendations.instant, Decimal::from(35));
    assert_eq!(recommendations.source, "mock");
}

#[tokio::test]
async fn test_process_pending_transactions() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    let gas_tracker = GasUsageTracker::new(
        monitor.clone(),
        oracle.clone(),
        calculator.clone(),
    );
    
    let user_id = Uuid::new_v4();
    
    // Add multiple pending transactions
    for i in 0..5 {
        let trade_id = Uuid::new_v4();
        let tx_hash = format!("0x{:016x}", i);
        
        gas_tracker.track_transaction(
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
        
        // Add receipts for some transactions
        if i < 3 {
            let receipt = TransactionReceipt {
                transaction_hash: tx_hash.clone(),
                block_number: 18_500_000 + i as u64,
                gas_used: 19500,
                gas_price: Decimal::from(25),
                status: TransactionStatus::Confirmed,
                timestamp: Utc::now(),
            };
            
            monitor.add_receipt(tx_hash.clone(), receipt).await;
        }
    }
    
    let processed_count = gas_tracker.process_pending_transactions().await.unwrap();
    assert_eq!(processed_count, 3); // Only 3 had receipts
    
    let health = gas_tracker.get_health_status().await;
    assert_eq!(health.pending_transactions, 2); // 2 still pending
    assert_eq!(health.total_tracked_transactions, 3); // 3 confirmed
}

#[tokio::test]
async fn test_insufficient_data_error() {
    let monitor = Arc::new(MockTransactionMonitor::new());
    let oracle = Arc::new(MockGasPriceOracle::new());
    let calculator = Arc::new(MockGasEfficiencyCalculator::new());
    
    let tracker = GasUsageTracker::new(monitor, oracle, calculator);
    
    let user_id = Uuid::new_v4();
    let end_date = Utc::now();
    let start_date = end_date - Duration::days(1);
    
    // Try to get metrics with no data
    let result = tracker.calculate_gas_efficiency_metrics(user_id, start_date, end_date).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        RiskError::InsufficientData(msg) => {
            assert!(msg.contains("No gas usage data found"));
        },
        _ => panic!("Expected InsufficientData error"),
    }
}
