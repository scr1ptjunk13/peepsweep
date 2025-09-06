use bralaladex_backend::risk_management::performance_tracker::*;
use bralaladex_backend::risk_management::types::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::test]
async fn test_performance_calculator_basic_metrics() {
    let calculator = PerformanceCalculator::new(Decimal::new(2, 2)); // 2% risk-free rate
    
    // Test basic performance metrics calculation
    let user_id = Uuid::new_v4();
    let initial_value = Decimal::from(10000);
    let current_value = Decimal::from(12000);
    let current_pnl = current_value - initial_value;
    
    // Create test data structures
    let return_history = ReturnHistory::new(100);
    let drawdown_history = DrawdownHistory::new(100);
    let trade_results = TradeResults::new();
    
    let metrics = calculator.calculate_metrics(
        user_id,
        current_value,
        current_pnl,
        initial_value,
        &return_history,
        &drawdown_history,
        &trade_results,
    );
    
    // ROI should be 20%
    let expected_roi = Decimal::from(20);
    assert_eq!(metrics.roi_percentage, expected_roi);
    assert_eq!(metrics.total_value_usd, current_value);
    assert_eq!(metrics.total_pnl, current_pnl);
    
    println!("✅ Basic performance metrics calculation test passed");
    println!("   Initial value: ${}", initial_value);
    println!("   Current value: ${}", current_value);
    println!("   ROI: {}%", metrics.roi_percentage);
}

#[tokio::test]
async fn test_performance_calculator_sharpe_ratio() {
    let calculator = PerformanceCalculator::new(Decimal::new(2, 2)); // 2% risk-free rate
    
    // Test Sharpe ratio calculation with sample returns
    let mut return_history = ReturnHistory::new(100);
    let timestamp = chrono::Utc::now().timestamp() as u64;
    
    // Add sample returns
    return_history.add_return(Decimal::from_f64(0.05).unwrap(), timestamp);
    return_history.add_return(Decimal::from_f64(0.03).unwrap(), timestamp + 1);
    return_history.add_return(Decimal::from_f64(0.08).unwrap(), timestamp + 2);
    return_history.add_return(Decimal::from_f64(0.01).unwrap(), timestamp + 3);
    return_history.add_return(Decimal::from_f64(0.06).unwrap(), timestamp + 4);
    
    let sharpe_ratio = calculator.calculate_sharpe_ratio(&return_history);
    
    // Sharpe ratio should be positive for returns above risk-free rate
    assert!(sharpe_ratio >= Decimal::ZERO);
    
    println!("✅ Sharpe ratio calculation test passed");
    println!("   Returns count: {}", return_history.returns.len());
    println!("   Sharpe ratio: {}", sharpe_ratio);
}

#[tokio::test]
async fn test_performance_calculator_max_drawdown() {
    let calculator = PerformanceCalculator::new(Decimal::new(2, 2));
    
    // Test maximum drawdown calculation using DrawdownHistory
    let mut drawdown_history = DrawdownHistory::new(100);
    let timestamp = chrono::Utc::now().timestamp() as u64;
    
    // Add portfolio values that show a drawdown pattern
    drawdown_history.add_value(Decimal::from(10000), timestamp);     // Initial
    drawdown_history.add_value(Decimal::from(12000), timestamp + 1); // Peak
    drawdown_history.add_value(Decimal::from(11000), timestamp + 2); // Drawdown
    drawdown_history.add_value(Decimal::from(9000), timestamp + 3);  // Max drawdown
    drawdown_history.add_value(Decimal::from(10500), timestamp + 4); // Recovery
    drawdown_history.add_value(Decimal::from(13000), timestamp + 5); // New peak
    
    let max_drawdown = drawdown_history.calculate_max_drawdown();
    
    // Max drawdown should be positive (representing percentage)
    assert!(max_drawdown >= Decimal::ZERO);
    
    println!("✅ Maximum drawdown calculation test passed");
    println!("   Values count: {}", drawdown_history.values.len());
    println!("   Max drawdown: {}%", max_drawdown);
}

#[tokio::test]
async fn test_performance_calculator_win_loss_ratio() {
    let calculator = PerformanceCalculator::new(Decimal::new(2, 2));
    
    // Test win/loss ratio calculation using TradeResults
    let mut trade_results = TradeResults::new();
    
    // Add winning trades
    trade_results.add_trade(Decimal::from(100), true);
    trade_results.add_trade(Decimal::from(200), true);
    trade_results.add_trade(Decimal::from(150), true);
    
    // Add losing trades
    trade_results.add_trade(Decimal::from(-50), false);
    trade_results.add_trade(Decimal::from(-30), false);
    
    let win_rate = trade_results.win_rate();
    let avg_win = trade_results.average_winning_trade();
    let avg_loss = trade_results.average_losing_trade();
    
    // Should have 3 wins out of 5 trades = 60%
    let expected_win_rate = Decimal::from(60);
    assert_eq!(win_rate, expected_win_rate);
    
    // Verify trade counts
    assert_eq!(trade_results.total_trades, 5);
    assert_eq!(trade_results.winning_trades, 3);
    assert_eq!(trade_results.losing_trades, 2);
    
    println!("✅ Win/loss ratio calculation test passed");
    println!("   Win rate: {}%", win_rate);
    println!("   Average win: ${}", avg_win);
    println!("   Average loss: ${}", avg_loss);
    println!("   Total trades: {}", trade_results.total_trades);
}

#[tokio::test]
async fn test_performance_metrics_structure() {
    // Test that PerformanceMetrics structure works correctly
    let user_id = Uuid::new_v4();
    let metrics = PerformanceMetrics {
        user_id,
        total_value_usd: Decimal::from(50000),
        total_pnl: Decimal::from(5000),
        roi_percentage: Decimal::from(10),
        sharpe_ratio: Decimal::from_f64(1.5).unwrap(),
        max_drawdown_percentage: Decimal::from(8),
        win_rate_percentage: Decimal::from(65),
        total_trades: 20,
        winning_trades: 13,
        losing_trades: 7,
        average_winning_trade: Decimal::from(800),
        average_losing_trade: Decimal::from(300),
        average_return_percentage: Decimal::from(5),
        return_volatility_percentage: Decimal::from(12),
        last_updated: chrono::Utc::now().timestamp() as u64,
    };
    
    // Validate all fields are accessible and have expected values
    assert_eq!(metrics.user_id, user_id);
    assert_eq!(metrics.total_value_usd, Decimal::from(50000));
    assert_eq!(metrics.total_pnl, Decimal::from(5000));
    assert_eq!(metrics.roi_percentage, Decimal::from(10));
    assert!(metrics.sharpe_ratio > Decimal::ZERO);
    assert_eq!(metrics.win_rate_percentage, Decimal::from(65));
    assert_eq!(metrics.total_trades, 20);
    assert_eq!(metrics.winning_trades, 13);
    assert_eq!(metrics.losing_trades, 7);
    
    println!("✅ Performance metrics structure test passed");
    println!("   All fields accessible and validated");
}

#[tokio::test]
async fn test_historical_performance_data() {
    let mut historical_data = HistoricalPerformanceData::new();
    let user_id = Uuid::new_v4();
    let timestamp = chrono::Utc::now().timestamp() as u64;
    
    // Test adding return data for a user
    let mut return_history = ReturnHistory::new(100);
    return_history.add_return(Decimal::from_f64(0.05).unwrap(), timestamp);
    return_history.add_return(Decimal::from_f64(-0.02).unwrap(), timestamp + 1);
    return_history.add_return(Decimal::from_f64(0.08).unwrap(), timestamp + 2);
    
    historical_data.return_history.insert(user_id, return_history);
    
    // Test adding portfolio values for a user
    let mut drawdown_history = DrawdownHistory::new(100);
    drawdown_history.add_value(Decimal::from(10000), timestamp);
    drawdown_history.add_value(Decimal::from(10500), timestamp + 1);
    drawdown_history.add_value(Decimal::from(10300), timestamp + 2);
    
    historical_data.drawdown_history.insert(user_id, drawdown_history);
    
    // Test adding trade results for a user
    let mut trade_results = TradeResults::new();
    trade_results.add_trade(Decimal::from(100), true);
    trade_results.add_trade(Decimal::from(-50), false);
    
    historical_data.trade_results.insert(user_id, trade_results);
    
    // Verify data was stored correctly
    assert!(historical_data.return_history.contains_key(&user_id));
    assert!(historical_data.drawdown_history.contains_key(&user_id));
    assert!(historical_data.trade_results.contains_key(&user_id));
    
    let stored_returns = &historical_data.return_history[&user_id];
    let stored_values = &historical_data.drawdown_history[&user_id];
    let stored_trades = &historical_data.trade_results[&user_id];
    
    assert_eq!(stored_returns.returns.len(), 3);
    assert_eq!(stored_values.values.len(), 3);
    assert_eq!(stored_trades.total_trades, 2);
    
    println!("✅ Historical performance data test passed");
    println!("   Returns stored: {}", stored_returns.returns.len());
    println!("   Values stored: {}", stored_values.values.len());
    println!("   Trades stored: {}", stored_trades.total_trades);
}
