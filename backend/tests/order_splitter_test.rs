use bralaladex_backend::execution::{
    OrderSplitter, SlippagePredictor, OrderSplitParams, SplittingStrategy, 
    OrderChunk, SplitOrderExecution, ExecutionStatus, ChunkStatus
};
use bralaladex_backend::aggregator::DexAggregator;
use bralaladex_backend::cache::CacheManager;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio;
use uuid::Uuid;

#[tokio::test]
async fn test_twap_order_splitting() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    let split_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(100000), // $100k total
        strategy: SplittingStrategy::TWAP { intervals: 5 },
        max_slippage_bps: Decimal::from(100), // 1%
        time_window_seconds: 3600, // 1 hour
        min_chunk_size: None,
        max_chunks: Some(5),
    };

    let execution = order_splitter.split_order(split_params).await.unwrap();

    // Verify TWAP splitting
    assert_eq!(execution.chunks.len(), 5);
    assert_eq!(execution.status, ExecutionStatus::Planning);
    assert!(execution.order_id != Uuid::nil());

    // Verify chunk amounts
    let expected_chunk_size = Decimal::from(20000); // $100k / 5
    for chunk in &execution.chunks {
        assert!(chunk.amount <= expected_chunk_size * Decimal::from_str("1.1").unwrap()); // Allow 10% variance
        assert!(chunk.amount >= expected_chunk_size * Decimal::from_str("0.9").unwrap());
        assert_eq!(chunk.status, ChunkStatus::Pending);
        assert!(chunk.execution_time > 0);
        assert!(!chunk.target_dexs.is_empty());
    }

    // Verify timing intervals
    let mut execution_times: Vec<u64> = execution.chunks.iter().map(|c| c.execution_time).collect();
    execution_times.sort();
    
    for i in 1..execution_times.len() {
        let interval = execution_times[i] - execution_times[i-1];
        assert!(interval >= 600); // At least 10 minutes between chunks
        assert!(interval <= 900); // At most 15 minutes between chunks
    }

    println!("✅ TWAP order splitting test passed");
    println!("   Order ID: {}", execution.order_id);
    println!("   Chunks: {}", execution.chunks.len());
    println!("   Total amount: ${}", execution.chunks.iter().map(|c| c.amount).sum::<Decimal>());
}

#[tokio::test]
async fn test_vwap_order_splitting() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    let split_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(200000), // $200k total
        strategy: SplittingStrategy::VWAP { volume_target: Decimal::from(1000000) },
        max_slippage_bps: Decimal::from(150), // 1.5%
        time_window_seconds: 7200, // 2 hours
        min_chunk_size: Some(Decimal::from(5000)),
        max_chunks: Some(20),
    };

    let execution = order_splitter.split_order(split_params).await.unwrap();

    // Verify VWAP splitting
    assert!(execution.chunks.len() > 0);
    assert!(execution.chunks.len() <= 20);
    assert_eq!(execution.status, ExecutionStatus::Planning);

    // Verify chunk sizes respect minimum
    for chunk in &execution.chunks {
        assert!(chunk.amount >= Decimal::from(5000));
        assert_eq!(chunk.status, ChunkStatus::Pending);
    }

    // Verify total amount is preserved
    let total_chunk_amount: Decimal = execution.chunks.iter().map(|c| c.amount).sum();
    assert!((total_chunk_amount - Decimal::from(200000)).abs() < Decimal::from(1000)); // Within $1k

    println!("✅ VWAP order splitting test passed");
    println!("   Chunks: {}", execution.chunks.len());
    println!("   Total chunk amount: ${}", total_chunk_amount);
    println!("   Average chunk size: ${}", total_chunk_amount / Decimal::from(execution.chunks.len()));
}

#[tokio::test]
async fn test_iceberg_order_splitting() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    let split_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(500000), // $500k total
        strategy: SplittingStrategy::Iceberg { visible_size_percent: Decimal::from(10) }, // 10% visible
        max_slippage_bps: Decimal::from(200), // 2%
        time_window_seconds: 14400, // 4 hours
        min_chunk_size: None,
        max_chunks: None,
    };

    let execution = order_splitter.split_order(split_params).await.unwrap();

    // Verify Iceberg splitting
    assert!(execution.chunks.len() >= 10); // Should have many small chunks
    assert_eq!(execution.status, ExecutionStatus::Planning);

    // Calculate visible size (10% of $500k = $50k)
    let expected_visible_size = Decimal::from(50000);
    
    // Most chunks should be around the visible size
    let chunk_sizes: Vec<Decimal> = execution.chunks.iter().map(|c| c.amount).collect();
    let avg_chunk_size = chunk_sizes.iter().sum::<Decimal>() / Decimal::from(chunk_sizes.len());
    
    assert!(avg_chunk_size <= expected_visible_size * Decimal::from_str("1.2").unwrap());
    assert!(avg_chunk_size >= expected_visible_size * Decimal::from_str("0.8").unwrap());

    // Verify timing is spread out
    let execution_times: Vec<u64> = execution.chunks.iter().map(|c| c.execution_time).collect();
    let time_span = execution_times.iter().max().unwrap() - execution_times.iter().min().unwrap();
    assert!(time_span >= 3600); // At least 1 hour span

    println!("✅ Iceberg order splitting test passed");
    println!("   Chunks: {}", execution.chunks.len());
    println!("   Average chunk size: ${}", avg_chunk_size);
    println!("   Time span: {} seconds", time_span);
}

#[tokio::test]
async fn test_adaptive_order_splitting() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    // Test conservative adaptive strategy
    let conservative_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(150000),
        strategy: SplittingStrategy::Adaptive { aggressiveness: Decimal::from_str("0.2").unwrap() },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: None,
    };

    let conservative_execution = order_splitter.split_order(conservative_params).await.unwrap();

    // Test aggressive adaptive strategy
    let aggressive_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(150000),
        strategy: SplittingStrategy::Adaptive { aggressiveness: Decimal::from_str("0.8").unwrap() },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: None,
    };

    let aggressive_execution = order_splitter.split_order(aggressive_params).await.unwrap();

    // Conservative should have more, smaller chunks
    assert!(conservative_execution.chunks.len() >= aggressive_execution.chunks.len());

    // Aggressive should have larger chunks and higher slippage tolerance
    let conservative_avg_chunk = conservative_execution.chunks.iter().map(|c| c.amount).sum::<Decimal>() 
        / Decimal::from(conservative_execution.chunks.len());
    let aggressive_avg_chunk = aggressive_execution.chunks.iter().map(|c| c.amount).sum::<Decimal>() 
        / Decimal::from(aggressive_execution.chunks.len());

    assert!(aggressive_avg_chunk >= conservative_avg_chunk);

    println!("✅ Adaptive order splitting test passed");
    println!("   Conservative chunks: {} (avg: ${})", 
             conservative_execution.chunks.len(), conservative_avg_chunk);
    println!("   Aggressive chunks: {} (avg: ${})", 
             aggressive_execution.chunks.len(), aggressive_avg_chunk);
}

#[tokio::test]
async fn test_order_execution_status_tracking() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    let split_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(50000),
        strategy: SplittingStrategy::TWAP { intervals: 3 },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 600, // 10 minutes for quick test
        min_chunk_size: None,
        max_chunks: Some(3),
    };

    let execution = order_splitter.split_order(split_params).await.unwrap();
    let order_id = execution.order_id;

    // Check initial status
    let status = order_splitter.get_order_status(order_id).await.unwrap();
    assert_eq!(status.status, ExecutionStatus::Planning);
    assert_eq!(status.chunks.len(), 3);

    // All chunks should be pending initially
    for chunk in &status.chunks {
        assert_eq!(chunk.status, ChunkStatus::Pending);
    }

    println!("✅ Order execution status tracking test passed");
    println!("   Order ID: {}", order_id);
    println!("   Initial status: {:?}", status.status);
    println!("   Chunks tracked: {}", status.chunks.len());
}

#[tokio::test]
async fn test_order_cancellation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    let split_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(75000),
        strategy: SplittingStrategy::TWAP { intervals: 4 },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 7200, // 2 hours
        min_chunk_size: None,
        max_chunks: Some(4),
    };

    let execution = order_splitter.split_order(split_params).await.unwrap();
    let order_id = execution.order_id;

    // Cancel the order
    order_splitter.cancel_order(order_id).await.unwrap();

    // Check status after cancellation
    let status = order_splitter.get_order_status(order_id).await.unwrap();
    assert!(matches!(status.status, ExecutionStatus::Failed { .. }));

    // All pending chunks should be marked as failed
    for chunk in &status.chunks {
        assert!(matches!(chunk.status, ChunkStatus::Failed { .. }));
    }

    println!("✅ Order cancellation test passed");
    println!("   Order {} successfully cancelled", order_id);
}

#[tokio::test]
async fn test_order_parameter_validation() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    // Test invalid amount (zero)
    let invalid_amount_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::ZERO,
        strategy: SplittingStrategy::TWAP { intervals: 5 },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: Some(5),
    };

    let result = order_splitter.split_order(invalid_amount_params).await;
    assert!(result.is_err());

    // Test invalid slippage (negative)
    let invalid_slippage_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(10000),
        strategy: SplittingStrategy::TWAP { intervals: 5 },
        max_slippage_bps: Decimal::from(-10),
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: Some(5),
    };

    let result = order_splitter.split_order(invalid_slippage_params).await;
    assert!(result.is_err());

    // Test invalid time window (zero)
    let invalid_time_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(10000),
        strategy: SplittingStrategy::TWAP { intervals: 5 },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 0,
        min_chunk_size: None,
        max_chunks: Some(5),
    };

    let result = order_splitter.split_order(invalid_time_params).await;
    assert!(result.is_err());

    println!("✅ Order parameter validation test passed");
}

#[tokio::test]
async fn test_chunk_size_optimization() {
    let cache_manager = Arc::new(CacheManager::new("redis://127.0.0.1:6379/").await.unwrap());
    let dex_aggregator = Arc::new(DexAggregator::new(cache_manager));
    let slippage_predictor = Arc::new(SlippagePredictor::new(dex_aggregator.clone()));
    let order_splitter = OrderSplitter::new(dex_aggregator, slippage_predictor);

    // Test with very large order that should be split more aggressively
    let large_order_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(10_000_000), // $10M order
        strategy: SplittingStrategy::TWAP { intervals: 10 },
        max_slippage_bps: Decimal::from(50), // Tight slippage tolerance
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: Some(10),
    };

    let execution = order_splitter.split_order(large_order_params).await.unwrap();

    // Large orders should result in smaller chunks due to slippage concerns
    let avg_chunk_size = execution.chunks.iter().map(|c| c.amount).sum::<Decimal>() 
        / Decimal::from(execution.chunks.len());
    
    // Average chunk should be much smaller than naive division ($1M)
    assert!(avg_chunk_size < Decimal::from(800_000)); // Less than $800k per chunk

    // Test with small order
    let small_order_params = OrderSplitParams {
        from_token: "WETH".to_string(),
        to_token: "USDC".to_string(),
        total_amount: Decimal::from(5_000), // $5k order
        strategy: SplittingStrategy::TWAP { intervals: 5 },
        max_slippage_bps: Decimal::from(100),
        time_window_seconds: 3600,
        min_chunk_size: None,
        max_chunks: Some(5),
    };

    let small_execution = order_splitter.split_order(small_order_params).await.unwrap();
    
    // Small orders might not need as much splitting
    let small_avg_chunk = small_execution.chunks.iter().map(|c| c.amount).sum::<Decimal>() 
        / Decimal::from(small_execution.chunks.len());

    println!("✅ Chunk size optimization test passed");
    println!("   Large order avg chunk: ${}", avg_chunk_size);
    println!("   Small order avg chunk: ${}", small_avg_chunk);
}

use std::str::FromStr;
