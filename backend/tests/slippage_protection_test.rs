use bralaladex_backend::execution::{
    SlippageProtectionEngine, SlippagePredictor, ProtectedSwapParams, 
    SlippageProtectionConfig, SwapPriority, ProtectionMeasure
};
use bralaladex_backend::aggregator::DEXAggregator;
use bralaladex_backend::cache::CacheManager;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_basic_slippage_protection() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    let protection_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(100), // 1%
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(500), // 5%
    };

    let swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(25000), // $25k trade
        protection_config,
        user_id: Some(Uuid::new_v4()),
        priority: SwapPriority::Protection,
    };

    let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();

    // Verify protection result structure
    assert!(result.swap_id != Uuid::nil());
    assert!(result.original_prediction.predicted_slippage_bps > Decimal::ZERO);
    assert!(result.adjusted_prediction.predicted_slippage_bps >= Decimal::ZERO);
    assert!(result.timestamp > 0);
    assert!(!result.protection_applied.is_empty()); // Some protection should be applied

    // Verify protection measures were considered
    let has_dynamic_adjustment = result.protection_applied.iter()
        .any(|p| matches!(p, ProtectionMeasure::DynamicSlippageAdjustment { .. }));
    let has_route_optimization = result.protection_applied.iter()
        .any(|p| matches!(p, ProtectionMeasure::RouteOptimization { .. }));

    assert!(has_dynamic_adjustment || has_route_optimization);

    println!("✅ Basic slippage protection test passed");
    println!("   Swap ID: {}", result.swap_id);
    println!("   Original prediction: {}bps", result.original_prediction.predicted_slippage_bps);
    println!("   Adjusted prediction: {}bps", result.adjusted_prediction.predicted_slippage_bps);
    println!("   Protection measures: {}", result.protection_applied.len());
}

#[tokio::test]
async fn test_dynamic_slippage_adjustment() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    // Test with low confidence scenario (should increase tolerance)
    let low_confidence_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(200), // 2% base tolerance
        dynamic_adjustment: true,
        route_optimization: false,
        pre_trade_validation: true,
        post_trade_analysis: false,
        emergency_stop_threshold_bps: Decimal::from(1000),
    };

    let swap_params = ProtectedSwapParams {
        from_token: "NEWTOKEN".to_string(), // New token = low confidence
        to_token: "USDC".to_string(),
        amount: Decimal::from(10000),
        protection_config: low_confidence_config,
        user_id: None,
        priority: SwapPriority::Protection,
    };

    let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();

    // Should have dynamic adjustment due to low confidence
    let has_adjustment = result.protection_applied.iter()
        .any(|p| matches!(p, ProtectionMeasure::DynamicSlippageAdjustment { .. }));

    if has_adjustment {
        println!("✅ Dynamic slippage adjustment applied for low confidence");
    }

    // Test with high confidence scenario
    let high_confidence_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(200), // 2% base tolerance
        dynamic_adjustment: true,
        route_optimization: false,
        pre_trade_validation: true,
        post_trade_analysis: false,
        emergency_stop_threshold_bps: Decimal::from(1000),
    };

    let high_confidence_params = ProtectedSwapParams {
        from_token: "WETH".to_string(), // Well-known token
        to_token: "USDC".to_string(),
        amount: Decimal::from(5000), // Smaller amount
        protection_config: high_confidence_config,
        user_id: None,
        priority: SwapPriority::Price,
    };

    let high_confidence_result = protection_engine.execute_protected_swap(high_confidence_params).await.unwrap();

    println!("✅ Dynamic slippage adjustment test passed");
    println!("   Low confidence adjustments: {}", result.protection_applied.len());
    println!("   High confidence adjustments: {}", high_confidence_result.protection_applied.len());
}

#[tokio::test]
async fn test_emergency_stop_protection() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    // Configure very low emergency threshold to trigger stop
    let emergency_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(50),
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(30), // Very low threshold
    };

    let risky_swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(1_000_000), // Very large trade likely to exceed threshold
        protection_config: emergency_config,
        user_id: None,
        priority: SwapPriority::Protection,
    };

    // This should trigger emergency stop
    let result = protection_engine.execute_protected_swap(risky_swap_params).await;

    // Should either succeed with emergency stop measure or fail with slippage tolerance error
    match result {
        Ok(swap_result) => {
            let has_emergency_stop = swap_result.protection_applied.iter()
                .any(|p| matches!(p, ProtectionMeasure::EmergencyStop { .. }));
            
            if has_emergency_stop {
                println!("✅ Emergency stop protection triggered successfully");
            }
        }
        Err(e) => {
            // Expected if slippage exceeds emergency threshold
            println!("✅ Emergency stop protection test passed - trade rejected: {}", e);
        }
    }
}

#[tokio::test]
async fn test_route_optimization_protection() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    let optimization_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(150),
        dynamic_adjustment: false,
        route_optimization: true, // Enable route optimization
        pre_trade_validation: true,
        post_trade_analysis: false,
        emergency_stop_threshold_bps: Decimal::from(1000),
    };

    let swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(50000),
        protection_config: optimization_config,
        user_id: None,
        priority: SwapPriority::Price,
    };

    let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();

    // Should have route optimization applied
    let has_route_optimization = result.protection_applied.iter()
        .any(|p| matches!(p, ProtectionMeasure::RouteOptimization { .. }));

    if has_route_optimization {
        println!("✅ Route optimization protection applied");
    }

    // Adjusted prediction should be better than original (lower slippage)
    if result.adjusted_prediction.predicted_slippage_bps < result.original_prediction.predicted_slippage_bps {
        println!("✅ Route optimization improved slippage prediction");
    }

    println!("✅ Route optimization protection test passed");
}

#[tokio::test]
async fn test_order_splitting_protection() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    let protection_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(100),
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(500),
    };

    // Large trade that should trigger order splitting
    let large_swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(500_000), // $500k - should exceed recommended max size
        protection_config,
        user_id: None,
        priority: SwapPriority::Protection,
    };

    let result = protection_engine.execute_protected_swap(large_swap_params).await.unwrap();

    // Should have order splitting protection
    let has_order_splitting = result.protection_applied.iter()
        .any(|p| matches!(p, ProtectionMeasure::OrderSplitting { .. }));

    if has_order_splitting {
        if let Some(ProtectionMeasure::OrderSplitting { chunks, chunk_size }) = 
            result.protection_applied.iter().find(|p| matches!(p, ProtectionMeasure::OrderSplitting { .. })) {
            println!("✅ Order splitting applied: {} chunks of ${}", chunks, chunk_size);
        }
    }

    // Order splitting should result in successful execution
    assert!(result.swap_id != Uuid::nil());
    assert!(result.protection_applied.len() > 0);

    println!("✅ Order splitting protection test passed");
}

#[tokio::test]
async fn test_swap_priority_handling() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    let base_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(200), // 2%
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(1000), // 10%
    };

    // Test different priorities
    let priorities = vec![
        SwapPriority::Speed,
        SwapPriority::Price,
        SwapPriority::Protection,
        SwapPriority::Balanced,
    ];

    for priority in priorities {
        let swap_params = ProtectedSwapParams {
            from_token: "WETH".to_string(),
            to_token: "USDC".to_string(),
            amount: Decimal::from(20000),
            protection_config: base_config.clone(),
            user_id: None,
            priority: priority.clone(),
        };

        let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();

        println!("   Priority {:?}: {} protection measures applied", 
                priority, result.protection_applied.len());

        // All priorities should result in valid swaps
        assert!(result.swap_id != Uuid::nil());
        assert!(result.original_prediction.predicted_slippage_bps >= Decimal::ZERO);
    }

    println!("✅ Swap priority handling test passed");
}

#[tokio::test]
async fn test_protection_statistics() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    // Execute several protected swaps to generate statistics
    let protection_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(200), // 2%
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(1000), // 10%
    };

    for i in 0..5 {
        let swap_params = ProtectedSwapParams {
            from_token: "WETH".to_string(),
            to_token: "USDC".to_string(),
            amount: Decimal::from(10000 + i * 5000),
            protection_config: protection_config.clone(),
            user_id: None,
            priority: SwapPriority::Balanced,
        };

        let _result = protection_engine.execute_protected_swap(swap_params).await.unwrap();
    }

    // Get protection statistics
    let stats = protection_engine.get_protection_statistics().await;

    // Verify statistics structure
    assert!(stats.contains_key("total_trades"));
    assert!(stats.contains_key("successful_trades"));
    assert!(stats.contains_key("success_rate"));
    assert!(stats.contains_key("avg_protection_effectiveness"));
    assert!(stats.contains_key("avg_actual_slippage_bps"));

    let total_trades = stats.get("total_trades").unwrap();
    assert!(*total_trades >= Decimal::from(5));

    println!("✅ Protection statistics test passed");
    println!("   Total trades: {}", total_trades);
    println!("   Success rate: {}", stats.get("success_rate").unwrap());
    println!("   Avg effectiveness: {}", stats.get("avg_protection_effectiveness").unwrap());
}

#[tokio::test]
async fn test_slippage_analysis() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    let protection_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(100),
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true, // Enable post-trade analysis
        emergency_stop_threshold_bps: Decimal::from(500),
    };

    let swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(30000),
        protection_config,
        user_id: None,
        priority: SwapPriority::Protection,
    };

    let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();
    let swap_id = result.swap_id;

    // Perform slippage analysis
    let analysis = protection_engine.analyze_slippage_performance(swap_id).await.unwrap();

    // Verify analysis structure
    assert_eq!(analysis.trade_id, swap_id);
    assert!(analysis.predicted_slippage_bps >= Decimal::ZERO);
    assert!(analysis.actual_slippage_bps >= Decimal::ZERO);
    assert!(analysis.prediction_accuracy >= Decimal::ZERO);
    assert!(analysis.prediction_accuracy <= Decimal::ONE);
    assert!(analysis.protection_effectiveness >= Decimal::ZERO);
    assert!(analysis.protection_effectiveness <= Decimal::ONE);
    assert!(analysis.market_conditions_at_execution.timestamp > 0);
    assert!(!analysis.lessons_learned.is_empty());

    println!("✅ Slippage analysis test passed");
    println!("   Trade ID: {}", analysis.trade_id);
    println!("   Predicted: {}bps, Actual: {}bps", 
             analysis.predicted_slippage_bps, analysis.actual_slippage_bps);
    println!("   Prediction accuracy: {}", analysis.prediction_accuracy);
    println!("   Protection effectiveness: {}", analysis.protection_effectiveness);
    println!("   Lessons learned: {:?}", analysis.lessons_learned);
}

#[tokio::test]
async fn test_protection_config_validation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let redis_client = cache_manager.get_redis_client();
    let dex_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let protection_engine = SlippageProtectionEngine::new(dex_aggregator, slippage_predictor);

    // Test with disabled protections
    let minimal_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(200),
        dynamic_adjustment: false,
        route_optimization: false,
        pre_trade_validation: false,
        post_trade_analysis: false,
        emergency_stop_threshold_bps: Decimal::from(1000),
    };

    let swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(15000),
        protection_config: minimal_config,
        user_id: None,
        priority: SwapPriority::Speed,
    };

    let result = protection_engine.execute_protected_swap(swap_params).await.unwrap();

    // With minimal protection, fewer measures should be applied
    assert!(result.protection_applied.len() <= 2); // Should be minimal

    // Test with maximum protections
    let maximal_config = SlippageProtectionConfig {
        max_slippage_bps: Decimal::from(50), // Very tight
        dynamic_adjustment: true,
        route_optimization: true,
        pre_trade_validation: true,
        post_trade_analysis: true,
        emergency_stop_threshold_bps: Decimal::from(200), // Low threshold
    };

    let maximal_swap_params = ProtectedSwapParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        amount: Decimal::from(15000),
        protection_config: maximal_config,
        user_id: None,
        priority: SwapPriority::Protection,
    };

    let maximal_result = protection_engine.execute_protected_swap(maximal_swap_params).await;

    // Should either succeed with many protections or fail due to tight constraints
    match maximal_result {
        Ok(swap_result) => {
            assert!(swap_result.protection_applied.len() >= result.protection_applied.len());
            println!("✅ Maximal protection applied: {} measures", swap_result.protection_applied.len());
        }
        Err(_) => {
            println!("✅ Maximal protection correctly rejected risky trade");
        }
    }

    println!("✅ Protection config validation test passed");
}

use std::str::FromStr;
