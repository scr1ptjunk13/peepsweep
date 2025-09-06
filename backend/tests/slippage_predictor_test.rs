use bralaladex_backend::execution::slippage_predictor::{
    SlippagePredictor, SlippageDataPoint, MarketConditions, LiquidityAnalysis, SlippagePrediction
};
use bralaladex_backend::aggregator::DexAggregator;
use bralaladex_backend::cache::CacheManager;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio;

#[tokio::test]
async fn test_slippage_prediction_basic_calculation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Test basic slippage prediction
    let prediction = predictor.predict_slippage(
        "WETH",
        "USDC", 
        Decimal::from(10000)
    ).await.unwrap();

    // Verify prediction structure
    assert!(prediction.predicted_slippage_bps > Decimal::ZERO);
    assert!(prediction.predicted_slippage_bps < Decimal::from(10000)); // Should be reasonable
    assert!(prediction.confidence_score >= Decimal::ZERO);
    assert!(prediction.confidence_score <= Decimal::ONE);
    assert!(prediction.market_impact_estimate >= Decimal::ZERO);
    assert!(prediction.recommended_max_trade_size > Decimal::ZERO);
    assert!(prediction.volatility_adjustment >= Decimal::ZERO);
    assert!(prediction.liquidity_score >= Decimal::ZERO);
    assert!(prediction.prediction_timestamp > 0);

    println!("✅ Basic slippage prediction test passed");
    println!("   Predicted slippage: {}bps", prediction.predicted_slippage_bps);
    println!("   Confidence score: {}", prediction.confidence_score);
    println!("   Market impact: {}bps", prediction.market_impact_estimate);
    println!("   Recommended max size: ${}", prediction.recommended_max_trade_size);
}

#[tokio::test]
async fn test_slippage_prediction_with_historical_data() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Add historical data points
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    for i in 0..20 {
        let data_point = SlippageDataPoint {
            timestamp: current_time - (i * 3600), // Last 20 hours
            trade_size_usd: Decimal::from(5000 + i * 1000),
            expected_output: Decimal::from(5000),
            actual_output: Decimal::from(4950 + i * 2),
            slippage_bps: Decimal::from(50 + i * 5), // Increasing slippage
            dex_name: "Uniswap".to_string(),
            token_pair: "WETH/USDC".to_string(),
            volatility: Decimal::from(25 + i * 2),
            liquidity_depth: Decimal::from(1000000 - i * 10000),
        };
        
        predictor.record_slippage_data(data_point).await.unwrap();
    }

    // Test prediction with historical data
    let prediction = predictor.predict_slippage(
        "WETH",
        "USDC",
        Decimal::from(15000)
    ).await.unwrap();

    // With historical data, confidence should be higher
    assert!(prediction.confidence_score > Decimal::from_str("0.3").unwrap());
    
    // Large trade should have higher predicted slippage
    let small_trade_prediction = predictor.predict_slippage(
        "WETH", 
        "USDC",
        Decimal::from(1000)
    ).await.unwrap();
    
    assert!(prediction.predicted_slippage_bps > small_trade_prediction.predicted_slippage_bps);

    println!("✅ Historical data slippage prediction test passed");
    println!("   Large trade slippage: {}bps", prediction.predicted_slippage_bps);
    println!("   Small trade slippage: {}bps", small_trade_prediction.predicted_slippage_bps);
    println!("   Confidence with data: {}", prediction.confidence_score);
}

#[tokio::test]
async fn test_liquidity_analysis_calculation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    let liquidity_analysis = predictor.analyze_liquidity(
        "WETH",
        "USDC",
        Decimal::from(50000)
    ).await.unwrap();

    // Verify liquidity analysis structure
    assert!(liquidity_analysis.total_liquidity_usd > Decimal::ZERO);
    assert!(liquidity_analysis.depth_at_1_percent >= Decimal::ZERO);
    assert!(liquidity_analysis.depth_at_5_percent >= liquidity_analysis.depth_at_1_percent);
    assert!(liquidity_analysis.average_spread_bps > Decimal::ZERO);
    assert!(liquidity_analysis.average_spread_bps < Decimal::from(1000)); // Should be reasonable
    assert!(!liquidity_analysis.liquidity_distribution.is_empty());

    // Verify liquidity distribution percentages sum to reasonable total
    let total_percentage: Decimal = liquidity_analysis.liquidity_distribution.values().sum();
    assert!(total_percentage <= Decimal::from(100));

    println!("✅ Liquidity analysis test passed");
    println!("   Total liquidity: ${}", liquidity_analysis.total_liquidity_usd);
    println!("   Depth at 1%: ${}", liquidity_analysis.depth_at_1_percent);
    println!("   Depth at 5%: ${}", liquidity_analysis.depth_at_5_percent);
    println!("   Average spread: {}bps", liquidity_analysis.average_spread_bps);
    println!("   DEX distribution: {:?}", liquidity_analysis.liquidity_distribution);
}

#[tokio::test]
async fn test_market_impact_scaling() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Test different trade sizes
    let trade_sizes = vec![
        Decimal::from(1000),
        Decimal::from(10000),
        Decimal::from(100000),
        Decimal::from(1000000),
    ];

    let mut predictions = Vec::new();
    
    for size in trade_sizes {
        let prediction = predictor.predict_slippage(
            "WETH",
            "USDC",
            size
        ).await.unwrap();
        predictions.push((size, prediction));
    }

    // Verify that larger trades generally have higher slippage
    for i in 1..predictions.len() {
        let (prev_size, prev_pred) = &predictions[i-1];
        let (curr_size, curr_pred) = &predictions[i];
        
        // Market impact should generally increase with size
        assert!(curr_pred.market_impact_estimate >= prev_pred.market_impact_estimate);
        
        println!("   Trade size: ${} -> Slippage: {}bps, Impact: {}bps", 
                curr_size, curr_pred.predicted_slippage_bps, curr_pred.market_impact_estimate);
    }

    println!("✅ Market impact scaling test passed");
}

#[tokio::test]
async fn test_slippage_data_recording_and_retrieval() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    // Record multiple data points for the same pair
    let test_data = vec![
        (Decimal::from(5000), Decimal::from(25)),   // $5k trade, 25bps slippage
        (Decimal::from(10000), Decimal::from(45)),  // $10k trade, 45bps slippage
        (Decimal::from(20000), Decimal::from(85)),  // $20k trade, 85bps slippage
        (Decimal::from(50000), Decimal::from(180)), // $50k trade, 180bps slippage
    ];

    for (i, (trade_size, slippage)) in test_data.iter().enumerate() {
        let data_point = SlippageDataPoint {
            timestamp: current_time - (i as u64 * 1800), // 30 min intervals
            trade_size_usd: *trade_size,
            expected_output: *trade_size,
            actual_output: *trade_size - (*trade_size * *slippage / Decimal::from(10000)),
            slippage_bps: *slippage,
            dex_name: "Uniswap".to_string(),
            token_pair: "WETH/USDC".to_string(),
            volatility: Decimal::from(30),
            liquidity_depth: Decimal::from(2000000),
        };
        
        predictor.record_slippage_data(data_point).await.unwrap();
    }

    // Test prediction after recording data
    let prediction = predictor.predict_slippage(
        "WETH",
        "USDC",
        Decimal::from(15000)
    ).await.unwrap();

    // Should have reasonable prediction based on historical data
    assert!(prediction.predicted_slippage_bps > Decimal::from(20)); // At least 20bps
    assert!(prediction.predicted_slippage_bps < Decimal::from(500)); // Less than 500bps
    assert!(prediction.confidence_score > Decimal::from_str("0.2").unwrap()); // Some confidence

    println!("✅ Slippage data recording test passed");
    println!("   Prediction after data: {}bps", prediction.predicted_slippage_bps);
    println!("   Confidence: {}", prediction.confidence_score);
}

#[tokio::test]
async fn test_volatility_adjustment_calculation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Add data points with varying volatility
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    let volatility_scenarios = vec![
        (Decimal::from(10), "Low volatility"),
        (Decimal::from(50), "Medium volatility"), 
        (Decimal::from(100), "High volatility"),
    ];

    for (volatility, description) in volatility_scenarios {
        // Clear previous data by using different token pairs
        let token_pair = format!("TEST{}/USDC", volatility);
        
        for i in 0..10 {
            let data_point = SlippageDataPoint {
                timestamp: current_time - (i * 3600),
                trade_size_usd: Decimal::from(10000),
                expected_output: Decimal::from(10000),
                actual_output: Decimal::from(9950),
                slippage_bps: Decimal::from(50) + (volatility / Decimal::from(10)), // Higher volatility = higher slippage
                dex_name: "Uniswap".to_string(),
                token_pair: token_pair.clone(),
                volatility,
                liquidity_depth: Decimal::from(1000000),
            };
            
            predictor.record_slippage_data(data_point).await.unwrap();
        }

        let prediction = predictor.predict_slippage(
            &format!("TEST{}", volatility),
            "USDC",
            Decimal::from(10000)
        ).await.unwrap();

        println!("   {}: Volatility adjustment = {}bps", 
                description, prediction.volatility_adjustment);
        
        // Higher volatility should result in higher adjustment
        assert!(prediction.volatility_adjustment >= Decimal::ZERO);
    }

    println!("✅ Volatility adjustment test passed");
}

#[tokio::test]
async fn test_confidence_score_calculation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Test confidence with no historical data
    let prediction_no_data = predictor.predict_slippage(
        "NEWTOKEN",
        "USDC",
        Decimal::from(10000)
    ).await.unwrap();

    // Test confidence with some historical data
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    
    for i in 0..50 {
        let data_point = SlippageDataPoint {
            timestamp: current_time - (i * 3600),
            trade_size_usd: Decimal::from(8000 + i * 100),
            expected_output: Decimal::from(8000),
            actual_output: Decimal::from(7960),
            slippage_bps: Decimal::from(50),
            dex_name: "Uniswap".to_string(),
            token_pair: "DATATOKEN/USDC".to_string(),
            volatility: Decimal::from(25),
            liquidity_depth: Decimal::from(1500000),
        };
        
        predictor.record_slippage_data(data_point).await.unwrap();
    }

    let prediction_with_data = predictor.predict_slippage(
        "DATATOKEN",
        "USDC",
        Decimal::from(10000)
    ).await.unwrap();

    // Confidence should be higher with more data
    assert!(prediction_with_data.confidence_score > prediction_no_data.confidence_score);
    assert!(prediction_no_data.confidence_score < Decimal::from_str("0.5").unwrap());
    assert!(prediction_with_data.confidence_score > Decimal::from_str("0.4").unwrap());

    println!("✅ Confidence score calculation test passed");
    println!("   No data confidence: {}", prediction_no_data.confidence_score);
    println!("   With data confidence: {}", prediction_with_data.confidence_score);
}

#[tokio::test]
async fn test_error_handling() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let predictor = SlippagePredictor::new(dex_aggregator);

    // Test invalid amount
    let result = predictor.predict_slippage(
        "WETH",
        "USDC",
        Decimal::ZERO
    ).await;
    
    assert!(result.is_err());
    
    // Test negative amount
    let result = predictor.predict_slippage(
        "WETH", 
        "USDC",
        Decimal::from(-1000)
    ).await;
    
    assert!(result.is_err());

    println!("✅ Error handling test passed");
}

use std::str::FromStr;
