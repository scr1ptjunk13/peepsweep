use bralaladex_backend::risk_management::performance_tracker::*;
use bralaladex_backend::risk_management::position_tracker::*;
use bralaladex_backend::risk_management::types::*;
use bralaladex_backend::risk_management::redis_cache::{RiskCache, RedisCacheConfig};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use std::collections::HashMap;

/// Test data setup for portfolio performance testing
struct TestPortfolioData {
    user_id: UserId,
    position_tracker: Arc<PositionTracker>,
    performance_tracker: PortfolioPerformanceTracker,
}

impl TestPortfolioData {
    async fn new() -> Self {
        let user_id = Uuid::new_v4();
        let position_tracker = Arc::new(PositionTracker::new(PositionTrackerConfig::default()));
        
        // Create Redis cache for testing
        let config = RedisCacheConfig::default();
        let redis_cache = Arc::new(RwLock::new(RiskCache::new(config).await.unwrap()));
        
        let performance_tracker = PortfolioPerformanceTracker::new(
            position_tracker.clone(),
            redis_cache,
        ).await.unwrap();

        Self {
            user_id,
            position_tracker,
            performance_tracker,
        }
    }

    async fn setup_test_positions(&self) -> Result<(), RiskError> {
        // Add realistic test positions
        let mut balances = HashMap::new();
        
        // ETH position: 10 ETH at $3200 = $32,000
        balances.insert("ETH".to_string(), TokenBalance {
            token_address: "ETH".to_string(),
            balance: Decimal::from(10),
            value_usd: Decimal::from(32000),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        // USDC position: 15,000 USDC at $1 = $15,000
        balances.insert("USDC".to_string(), TokenBalance {
            token_address: "USDC".to_string(),
            balance: Decimal::from(15000),
            value_usd: Decimal::from(15000),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        // BTC position: 0.5 BTC at $65,000 = $32,500
        balances.insert("BTC".to_string(), TokenBalance {
            token_address: "BTC".to_string(),
            balance: Decimal::from_f64(0.5).unwrap(),
            value_usd: Decimal::from(32500),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });

        let positions = UserPositions {
            balances,
            pnl: Decimal::ZERO,
            last_updated: chrono::Utc::now().timestamp() as u64,
        };

        self.position_tracker.insert_user_position(self.user_id, positions);
        Ok(())
    }

    async fn simulate_trade_with_profit(&self) -> Result<(), RiskError> {
        // Simulate ETH price increase to $3500 (+$3000 profit)
        let mut balances = HashMap::new();
        
        balances.insert("ETH".to_string(), TokenBalance {
            token_address: "ETH".to_string(),
            balance: Decimal::from(10),
            value_usd: Decimal::from(35000), // Price increased
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("USDC".to_string(), TokenBalance {
            token_address: "USDC".to_string(),
            balance: Decimal::from(15000),
            value_usd: Decimal::from(15000),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("BTC".to_string(), TokenBalance {
            token_address: "BTC".to_string(),
            balance: Decimal::from_f64(0.5).unwrap(),
            value_usd: Decimal::from(32500),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });

        let positions = UserPositions {
            balances,
            pnl: Decimal::from(3000), // $3000 profit
            last_updated: chrono::Utc::now().timestamp() as u64,
        };

        self.position_tracker.insert_user_position(self.user_id, positions);
        Ok(())
    }
}

#[tokio::test]
async fn test_real_time_pnl_calculation() {
    let test_data = TestPortfolioData::new().await;
    
    // Setup initial positions
    test_data.setup_test_positions().await.unwrap();
    
    // Calculate initial performance metrics
    let initial_performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Initial portfolio value should be $79,500 (32k + 15k + 32.5k)
    assert_eq!(initial_performance.total_value_usd, Decimal::from(79500));
    assert_eq!(initial_performance.total_pnl, Decimal::ZERO);
    assert_eq!(initial_performance.roi_percentage, Decimal::ZERO);
    
    // Simulate profitable trade
    test_data.simulate_trade_with_profit().await.unwrap();
    
    // Calculate updated performance metrics
    let updated_performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Portfolio value should now be $82,500 with $3,000 profit
    assert_eq!(updated_performance.total_value_usd, Decimal::from(82500));
    assert_eq!(updated_performance.total_pnl, Decimal::from(3000));
    
    // ROI should be approximately 3.77% (3000/79500)
    let expected_roi = Decimal::from(3000) / Decimal::from(79500) * Decimal::from(100);
    assert!((updated_performance.roi_percentage - expected_roi).abs() < Decimal::from_f64(0.01).unwrap());
    
    println!("✅ Real-time P&L calculation test passed");
    println!("   Initial value: ${}", initial_performance.total_value_usd);
    println!("   Updated value: ${}", updated_performance.total_value_usd);
    println!("   P&L: ${}", updated_performance.total_pnl);
    println!("   ROI: {}%", updated_performance.roi_percentage);
}

#[tokio::test]
async fn test_sharpe_ratio_calculation() {
    let test_data = TestPortfolioData::new().await;
    
    // Setup initial positions
    test_data.setup_test_positions().await.unwrap();
    
    // Simulate multiple trades with varying returns
    let returns = vec![
        Decimal::from_f64(0.05).unwrap(),  // 5% return
        Decimal::from_f64(-0.02).unwrap(), // -2% return
        Decimal::from_f64(0.08).unwrap(),  // 8% return
        Decimal::from_f64(0.01).unwrap(),  // 1% return
        Decimal::from_f64(-0.01).unwrap(), // -1% return
    ];
    
    for (i, return_rate) in returns.iter().enumerate() {
        let new_value = Decimal::from(79500) * (Decimal::ONE + return_rate);
        let pnl = new_value - Decimal::from(79500);
        
        // Update positions with new values
        let mut balances = HashMap::new();
        balances.insert("ETH".to_string(), TokenBalance {
            token_address: "ETH".to_string(),
            balance: Decimal::from(10),
            value_usd: new_value * Decimal::from_f64(0.4).unwrap(), // 40% allocation
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("USDC".to_string(), TokenBalance {
            token_address: "USDC".to_string(),
            balance: Decimal::from(15000),
            value_usd: new_value * Decimal::from_f64(0.19).unwrap(), // 19% allocation
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("BTC".to_string(), TokenBalance {
            token_address: "BTC".to_string(),
            balance: Decimal::from_f64(0.5).unwrap(),
            value_usd: new_value * Decimal::from_f64(0.41).unwrap(), // 41% allocation
            last_updated: chrono::Utc::now().timestamp() as u64,
        });

        let positions = UserPositions {
            balances,
            pnl,
            last_updated: chrono::Utc::now().timestamp() as u64,
        };

        test_data.position_tracker.insert_user_position(test_data.user_id, positions);
        
        // Add return to performance history
        test_data.performance_tracker
            .add_return_to_history(test_data.user_id, *return_rate)
            .await
            .unwrap();
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Calculate Sharpe ratio
    let performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Sharpe ratio should be calculated based on return history
    // With mean return ~2.2% and some volatility, expect positive Sharpe ratio
    assert!(performance.sharpe_ratio > Decimal::ZERO);
    assert!(performance.sharpe_ratio < Decimal::from(20)); // Reasonable upper bound for high-performing portfolio
    
    println!("✅ Sharpe ratio calculation test passed");
    println!("   Sharpe ratio: {}", performance.sharpe_ratio);
    println!("   Average return: {}%", performance.average_return_percentage);
    println!("   Return volatility: {}%", performance.return_volatility_percentage);
}

#[tokio::test]
async fn test_maximum_drawdown_analysis() {
    let test_data = TestPortfolioData::new().await;
    
    // Setup initial positions
    test_data.setup_test_positions().await.unwrap();
    
    // Simulate portfolio value changes with a significant drawdown
    let portfolio_values = vec![
        Decimal::from(79500),  // Initial value
        Decimal::from(85000),  // +6.9% peak
        Decimal::from(82000),  // -3.5% from peak
        Decimal::from(75000),  // -11.8% from peak (max drawdown)
        Decimal::from(78000),  // Recovery
        Decimal::from(83000),  // New high
    ];
    
    for (i, value) in portfolio_values.iter().enumerate() {
        let pnl = *value - Decimal::from(79500);
        
        // Update portfolio value
        let mut balances = HashMap::new();
        balances.insert("ETH".to_string(), TokenBalance {
            token_address: "ETH".to_string(),
            balance: Decimal::from(10),
            value_usd: *value * Decimal::from_f64(0.4).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("USDC".to_string(), TokenBalance {
            token_address: "USDC".to_string(),
            balance: Decimal::from(15000),
            value_usd: *value * Decimal::from_f64(0.19).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("BTC".to_string(), TokenBalance {
            token_address: "BTC".to_string(),
            balance: Decimal::from_f64(0.5).unwrap(),
            value_usd: *value * Decimal::from_f64(0.41).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });

        let positions = UserPositions {
            balances,
            pnl,
            last_updated: chrono::Utc::now().timestamp() as u64,
        };

        test_data.position_tracker.insert_user_position(test_data.user_id, positions);
        
        // Add value to drawdown history
        test_data.performance_tracker
            .add_value_to_drawdown_history(test_data.user_id, *value)
            .await
            .unwrap();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Calculate maximum drawdown
    let performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Maximum drawdown should be approximately 11.8% (from 85000 to 75000)
    let expected_max_drawdown = (Decimal::from(85000) - Decimal::from(75000)) / Decimal::from(85000) * Decimal::from(100);
    
    assert!((performance.max_drawdown_percentage - expected_max_drawdown).abs() < Decimal::from(1));
    assert!(performance.max_drawdown_percentage > Decimal::from(10));
    assert!(performance.max_drawdown_percentage < Decimal::from(15));
    
    println!("✅ Maximum drawdown analysis test passed");
    println!("   Max drawdown: {}%", performance.max_drawdown_percentage);
    println!("   Expected: {}%", expected_max_drawdown);
}

#[tokio::test]
async fn test_win_loss_ratio_tracking() {
    let test_data = TestPortfolioData::new().await;
    
    // Setup initial positions
    test_data.setup_test_positions().await.unwrap();
    
    // Simulate a series of trades with wins and losses
    let trade_results = vec![
        (Decimal::from(1000), true),   // Win: +$1000
        (Decimal::from(-500), false),  // Loss: -$500
        (Decimal::from(1500), true),   // Win: +$1500
        (Decimal::from(-200), false),  // Loss: -$200
        (Decimal::from(800), true),    // Win: +$800
        (Decimal::from(-300), false),  // Loss: -$300
        (Decimal::from(2000), true),   // Win: +$2000
    ];
    
    for (pnl, is_win) in trade_results.iter() {
        test_data.performance_tracker
            .record_trade_result(test_data.user_id, *pnl, *is_win)
            .await
            .unwrap();
    }
    
    // Calculate win/loss metrics
    let performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Should have 4 wins out of 7 trades = 57.14% win rate
    let expected_win_rate = Decimal::from(4) / Decimal::from(7) * Decimal::from(100);
    
    assert!((performance.win_rate_percentage - expected_win_rate).abs() < Decimal::from_f64(0.1).unwrap());
    assert_eq!(performance.total_trades, 7);
    assert_eq!(performance.winning_trades, 4);
    assert_eq!(performance.losing_trades, 3);
    
    // Average winning trade should be $1325 ((1000+1500+800+2000)/4)
    let expected_avg_win = Decimal::from(5300) / Decimal::from(4);
    assert!((performance.average_winning_trade - expected_avg_win).abs() < Decimal::from(1));
    
    // Average losing trade should be $333.33 ((500+200+300)/3)
    let expected_avg_loss = Decimal::from(1000) / Decimal::from(3);
    assert!((performance.average_losing_trade - expected_avg_loss).abs() < Decimal::from(1));
    
    println!("✅ Win/loss ratio tracking test passed");
    println!("   Win rate: {}%", performance.win_rate_percentage);
    println!("   Total trades: {}", performance.total_trades);
    println!("   Average win: ${}", performance.average_winning_trade);
    println!("   Average loss: ${}", performance.average_losing_trade);
}

#[tokio::test]
async fn test_performance_metrics_integration() {
    let test_data = TestPortfolioData::new().await;
    
    // Setup initial positions
    test_data.setup_test_positions().await.unwrap();
    
    // Simulate realistic trading scenario over time
    let scenarios = vec![
        // Day 1: Small profit
        (Decimal::from(81000), Decimal::from(1500)),
        // Day 2: Loss
        (Decimal::from(77000), Decimal::from(-2500)),
        // Day 3: Recovery
        (Decimal::from(84000), Decimal::from(4500)),
        // Day 4: Small loss
        (Decimal::from(82500), Decimal::from(3000)),
        // Day 5: Big win
        (Decimal::from(88000), Decimal::from(8500)),
    ];
    
    for (value, pnl) in scenarios.iter() {
        // Update positions
        let mut balances = HashMap::new();
        balances.insert("ETH".to_string(), TokenBalance {
            token_address: "ETH".to_string(),
            balance: Decimal::from(10),
            value_usd: *value * Decimal::from_f64(0.4).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("USDC".to_string(), TokenBalance {
            token_address: "USDC".to_string(),
            balance: Decimal::from(15000),
            value_usd: *value * Decimal::from_f64(0.19).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });
        
        balances.insert("BTC".to_string(), TokenBalance {
            token_address: "BTC".to_string(),
            balance: Decimal::from_f64(0.5).unwrap(),
            value_usd: *value * Decimal::from_f64(0.41).unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        });

        let positions = UserPositions {
            balances,
            pnl: *pnl,
            last_updated: chrono::Utc::now().timestamp() as u64,
        };

        test_data.position_tracker.insert_user_position(test_data.user_id, positions);
        
        // Record performance data
        let return_rate = *pnl / Decimal::from(79500);
        test_data.performance_tracker
            .add_return_to_history(test_data.user_id, return_rate)
            .await
            .unwrap();
        
        test_data.performance_tracker
            .add_value_to_drawdown_history(test_data.user_id, *value)
            .await
            .unwrap();
        
        // Record trade result
        let is_win = *pnl > Decimal::ZERO;
        test_data.performance_tracker
            .record_trade_result(test_data.user_id, *pnl, is_win)
            .await
            .unwrap();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    
    // Get comprehensive performance metrics
    let performance = test_data.performance_tracker
        .calculate_performance_metrics(test_data.user_id)
        .await
        .unwrap();
    
    // Validate all metrics are calculated
    assert!(performance.total_value_usd > Decimal::ZERO);
    assert!(performance.total_pnl != Decimal::ZERO);
    assert!(performance.roi_percentage != Decimal::ZERO);
    assert!(performance.sharpe_ratio != Decimal::ZERO);
    assert!(performance.max_drawdown_percentage >= Decimal::ZERO);
    assert!(performance.win_rate_percentage >= Decimal::ZERO);
    assert!(performance.win_rate_percentage <= Decimal::from(100));
    assert!(performance.total_trades > 0);
    
    println!("✅ Performance metrics integration test passed");
    println!("   Final portfolio value: ${}", performance.total_value_usd);
    println!("   Total P&L: ${}", performance.total_pnl);
    println!("   ROI: {}%", performance.roi_percentage);
    println!("   Sharpe ratio: {}", performance.sharpe_ratio);
    println!("   Max drawdown: {}%", performance.max_drawdown_percentage);
    println!("   Win rate: {}%", performance.win_rate_percentage);
    println!("   Total trades: {}", performance.total_trades);
}
