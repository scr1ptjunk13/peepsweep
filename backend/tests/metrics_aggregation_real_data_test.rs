use bralaladex_backend::performance::PerformanceMonitor;
use bralaladex_backend::routing::liquidity_tracker::LiquidityTracker;
use bralaladex_backend::risk_management::metrics_aggregation::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use chrono::{DateTime, Utc};

#[tokio::test]
async fn test_real_data_metrics_collection() {
    println!("🧪 TESTING REAL-TIME METRICS AGGREGATION WITH REAL DATA");
    
    // Create real components
    let performance_monitor = Arc::new(PerformanceMonitor::new());
    let liquidity_tracker = Arc::new(LiquidityTracker::new().await);
    let config = MetricsAggregationConfig::default();
    
    let aggregator = Arc::new(MetricsAggregator::new(
        config,
        performance_monitor.clone(),
        liquidity_tracker,
        None, // No bridge manager for this test
    ));
    
    println!("✅ Metrics aggregator initialized");
    
    // Test 1: Collect Performance Metrics with Real Data
    println!("📊 Testing PERFORMANCE METRICS collection");
    
    // Simulate some real performance data
    for i in 0..10 {
        let request_id = i as u64;
        performance_monitor.start_request(request_id);
        
        // Simulate processing time
        sleep(Duration::from_millis(50 + i * 10)).await;
        
        performance_monitor.end_request(request_id);
    }
    
    let snapshot = aggregator.collect_snapshot().await.unwrap();
    println!("✅ PERFORMANCE METRICS COLLECTED:");
    println!("   - Active connections: {}", snapshot.performance_metrics.active_connections);
    println!("   - Total processed: {}", snapshot.performance_metrics.total_processed);
    println!("   - Avg response time: {:.2}ms", snapshot.performance_metrics.avg_response_time_ms);
    println!("   - P95 response time: {:.2}ms", snapshot.performance_metrics.p95_response_time_ms);
    println!("   - Requests/sec: {:.2}", snapshot.performance_metrics.requests_per_second);
    println!("   - Cache hit rate: {:.1}%", snapshot.performance_metrics.cache_hit_rate * 100.0);
    
    assert!(snapshot.performance_metrics.total_processed > 0);
    assert!(snapshot.performance_metrics.avg_response_time_ms > 0.0);
    
    // Test 2: DEX Liquidity Metrics with Real Data
    println!("💰 Testing DEX LIQUIDITY METRICS collection");
    println!("✅ DEX LIQUIDITY METRICS COLLECTED:");
    println!("   - Total liquidity: ${:.2}M", snapshot.dex_liquidity_metrics.total_liquidity_usd as f64 / 1_000_000.0);
    println!("   - Active pools: {}", snapshot.dex_liquidity_metrics.active_pools);
    println!("   - 24h volume: ${:.2}M", snapshot.dex_liquidity_metrics.total_volume_24h as f64 / 1_000_000.0);
    println!("   - Average pool size: ${:.2}M", snapshot.dex_liquidity_metrics.average_pool_size as f64 / 1_000_000.0);
    println!("   - Top pools count: {}", snapshot.dex_liquidity_metrics.top_pools.len());
    
    assert!(snapshot.dex_liquidity_metrics.total_liquidity_usd > 0);
    assert!(snapshot.dex_liquidity_metrics.active_pools > 0);
    assert!(!snapshot.dex_liquidity_metrics.top_pools.is_empty());
    assert!(!snapshot.dex_liquidity_metrics.dex_breakdown.is_empty());
    
    // Verify DEX breakdown
    for (dex_name, dex_metrics) in &snapshot.dex_liquidity_metrics.dex_breakdown {
        println!("   - {}: ${:.2}M liquidity, {} pools", 
            dex_name, 
            dex_metrics.total_liquidity as f64 / 1_000_000.0,
            dex_metrics.pool_count
        );
        assert!(dex_metrics.total_liquidity > 0);
        assert!(dex_metrics.pool_count > 0);
    }
    
    // Test 3: Bridge Status Metrics with Real Data
    println!("🌉 Testing BRIDGE STATUS METRICS collection");
    println!("✅ BRIDGE STATUS METRICS COLLECTED:");
    println!("   - Total bridges: {}", snapshot.bridge_status_metrics.total_bridges);
    println!("   - Active bridges: {}", snapshot.bridge_status_metrics.active_bridges);
    println!("   - 24h volume: ${:.2}M", snapshot.bridge_status_metrics.total_volume_24h as f64 / 1_000_000.0);
    println!("   - Avg bridge time: {:.1}s", snapshot.bridge_status_metrics.average_bridge_time);
    println!("   - Cross-chain routes: {}", snapshot.bridge_status_metrics.cross_chain_routes.len());
    
    assert!(snapshot.bridge_status_metrics.total_bridges > 0);
    assert!(snapshot.bridge_status_metrics.active_bridges > 0);
    assert!(!snapshot.bridge_status_metrics.bridge_breakdown.is_empty());
    
    // Verify bridge breakdown
    for (bridge_name, bridge_metrics) in &snapshot.bridge_status_metrics.bridge_breakdown {
        println!("   - {}: {:.1}% success rate, {:.1}s avg time", 
            bridge_name, 
            bridge_metrics.success_rate * 100.0,
            bridge_metrics.avg_completion_time
        );
        assert!(bridge_metrics.success_rate > 0.0);
        assert!(bridge_metrics.avg_completion_time > 0.0);
    }
    
    // Test 4: System Health Metrics with Real Data
    println!("🏥 Testing SYSTEM HEALTH METRICS collection");
    println!("✅ SYSTEM HEALTH METRICS COLLECTED:");
    println!("   - Overall health: {:.1}%", snapshot.system_health_metrics.overall_health_score * 100.0);
    println!("   - CPU usage: {:.1}%", snapshot.system_health_metrics.cpu_usage);
    println!("   - Memory usage: {:.1}%", snapshot.system_health_metrics.memory_usage);
    println!("   - Disk usage: {:.1}%", snapshot.system_health_metrics.disk_usage);
    println!("   - Network latency: {:.1}ms", snapshot.system_health_metrics.network_latency);
    println!("   - Database status: {:?}", snapshot.system_health_metrics.database_health.status);
    println!("   - Redis status: {:?}", snapshot.system_health_metrics.redis_health.status);
    println!("   - External APIs: {}", snapshot.system_health_metrics.external_api_health.len());
    
    assert!(snapshot.system_health_metrics.overall_health_score > 0.0);
    assert!(snapshot.system_health_metrics.cpu_usage >= 0.0);
    assert!(snapshot.system_health_metrics.memory_usage >= 0.0);
    assert!(!snapshot.system_health_metrics.external_api_health.is_empty());
    
    // Verify external API health
    for (api_name, api_health) in &snapshot.system_health_metrics.external_api_health {
        println!("   - {}: {:?}, {:.1}ms response", 
            api_name, 
            api_health.status,
            api_health.response_time_ms
        );
    }
    
    println!("🎉 ALL REAL DATA METRICS COLLECTION TESTS PASSED!");
}

#[tokio::test]
async fn test_real_time_metrics_aggregation() {
    println!("🧪 TESTING REAL-TIME METRICS AGGREGATION");
    
    let performance_monitor = Arc::new(PerformanceMonitor::new());
    let liquidity_tracker = Arc::new(LiquidityTracker::new().await);
    let mut config = MetricsAggregationConfig::default();
    config.collection_interval_seconds = 1; // 1 second for testing
    config.max_snapshots = 10;
    
    let aggregator = Arc::new(MetricsAggregator::new(
        config,
        performance_monitor.clone(),
        liquidity_tracker,
        None,
    ));
    
    // Subscribe to real-time updates
    let mut receiver = aggregator.subscribe_to_metrics();
    
    // Collect multiple snapshots
    for i in 0..3 {
        println!("📊 Collecting snapshot #{}", i + 1);
        
        // Generate some performance data
        for j in 0..5 {
            let request_id = (i * 5 + j) as u64;
            performance_monitor.start_request(request_id);
            sleep(Duration::from_millis(20)).await;
            performance_monitor.end_request(request_id);
        }
        
        let snapshot = aggregator.collect_snapshot().await.unwrap();
        println!("✅ Snapshot #{} collected at {}", i + 1, snapshot.timestamp);
        
        // Verify snapshot data
        assert_eq!(snapshot.performance_metrics.total_processed, ((i + 1) * 5) as u64);
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Test metrics querying
    println!("🔍 Testing METRICS QUERYING");
    let start_time = Utc::now() - chrono::Duration::minutes(1);
    let query = MetricsQuery {
        start_time: Some(start_time),
        end_time: Some(Utc::now() + chrono::Duration::minutes(1)),
        metric_types: vec![MetricType::Performance, MetricType::DexLiquidity],
        aggregation: AggregationType::Average,
        interval: None,
    };
    
    let aggregated = aggregator.query_metrics(query).await.unwrap();
    println!("✅ METRICS QUERY RESULTS:");
    println!("   - Data points: {}", aggregated.data_points.len());
    println!("   - Time range: {} to {}", 
        aggregated.summary.time_range.0, 
        aggregated.summary.time_range.1
    );
    println!("   - Key insights: {}", aggregated.summary.key_insights.len());
    
    assert!(aggregated.data_points.len() > 0);
    assert!(aggregated.summary.total_data_points > 0);
    
    for insight in &aggregated.summary.key_insights {
        println!("   💡 {}", insight);
    }
    
    println!("🎉 REAL-TIME AGGREGATION TESTS PASSED!");
}

#[tokio::test]
async fn test_anomaly_detection() {
    println!("🧪 TESTING ANOMALY DETECTION WITH REAL DATA");
    
    let performance_monitor = Arc::new(PerformanceMonitor::new());
    let liquidity_tracker = Arc::new(LiquidityTracker::new().await);
    let config = MetricsAggregationConfig::default();
    
    let aggregator = Arc::new(MetricsAggregator::new(
        config,
        performance_monitor.clone(),
        liquidity_tracker,
        None,
    ));
    
    // Generate baseline data
    println!("📊 Generating baseline performance data");
    for i in 0..20 {
        let request_id = i as u64;
        performance_monitor.start_request(request_id);
        sleep(Duration::from_millis(50)).await; // Normal response time
        performance_monitor.end_request(request_id);
        
        let _ = aggregator.collect_snapshot().await.unwrap();
        sleep(Duration::from_millis(10)).await;
    }
    
    // Generate anomalous data
    println!("⚠️  Generating anomalous performance data");
    for i in 20..25 {
        let request_id = i as u64;
        performance_monitor.start_request(request_id);
        sleep(Duration::from_millis(500)).await; // Abnormally high response time
        performance_monitor.end_request(request_id);
        
        let snapshot = aggregator.collect_snapshot().await.unwrap();
        println!("   - Snapshot #{}: {:.2}ms response time", i + 1, snapshot.performance_metrics.avg_response_time_ms);
        sleep(Duration::from_millis(10)).await;
    }
    
    // Query for anomalies
    let query = MetricsQuery {
        start_time: Some(chrono::Utc::now() - chrono::Duration::minutes(5)),
        end_time: Some(chrono::Utc::now()),
        metric_types: vec![MetricType::All],
        aggregation: AggregationType::Raw,
        interval: None,
    };
    
    let aggregated = aggregator.query_metrics(query).await.unwrap();
    
    println!("🔍 ANOMALY DETECTION RESULTS:");
    println!("   - Total anomalies detected: {}", aggregated.summary.anomalies_detected.len());
    
    for anomaly in &aggregated.summary.anomalies_detected {
        println!("   🚨 {}: expected {:.2}, got {:.2} (severity: {:?})", 
            anomaly.metric_name,
            anomaly.expected_value,
            anomaly.actual_value,
            anomaly.severity
        );
    }
    
    // We should detect some anomalies from the high response times
    assert!(aggregated.summary.anomalies_detected.len() > 0);
    
    println!("🎉 ANOMALY DETECTION TESTS PASSED!");
}

#[tokio::test]
async fn test_health_status_monitoring() {
    println!("🧪 TESTING HEALTH STATUS MONITORING");
    
    let performance_monitor = Arc::new(PerformanceMonitor::new());
    let liquidity_tracker = Arc::new(LiquidityTracker::new().await);
    let config = MetricsAggregationConfig::default();
    
    let aggregator = Arc::new(MetricsAggregator::new(
        config,
        performance_monitor,
        liquidity_tracker,
        None,
    ));
    
    // Collect initial snapshot
    let _ = aggregator.collect_snapshot().await.unwrap();
    
    // Test health status
    let health_status = aggregator.get_health_status().await;
    println!("✅ HEALTH STATUS: {:?}", health_status);
    
    // Health status should be available
    assert!(matches!(health_status, HealthStatus::Healthy | HealthStatus::Warning | HealthStatus::Critical | HealthStatus::Down));
    
    // Get latest snapshot and verify health metrics
    let latest = aggregator.get_latest_snapshot().await.expect("Should have latest snapshot");
    println!("📊 LATEST HEALTH METRICS:");
    println!("   - Overall health score: {:.1}%", latest.system_health_metrics.overall_health_score * 100.0);
    println!("   - Database connections: {}/{}", 
        latest.system_health_metrics.database_health.active_connections,
        latest.system_health_metrics.database_health.connection_pool_size
    );
    println!("   - Redis connected: {}", latest.system_health_metrics.redis_health.connected);
    println!("   - Redis hit rate: {:.1}%", latest.system_health_metrics.redis_health.hit_rate * 100.0);
    
    assert!(latest.system_health_metrics.overall_health_score >= 0.0);
    assert!(latest.system_health_metrics.overall_health_score <= 1.0);
    
    println!("🎉 HEALTH STATUS MONITORING TESTS PASSED!");
}

#[tokio::test]
async fn test_comprehensive_metrics_integration() {
    println!("🧪 TESTING COMPREHENSIVE METRICS INTEGRATION");
    
    let performance_monitor = Arc::new(PerformanceMonitor::new());
    let liquidity_tracker = Arc::new(LiquidityTracker::new().await);
    let config = MetricsAggregationConfig::default();
    
    let aggregator = Arc::new(MetricsAggregator::new(
        config,
        performance_monitor.clone(),
        liquidity_tracker,
        None,
    ));
    
    // Simulate real trading activity
    println!("🔄 Simulating real trading activity");
    
    for round in 0..3 {
        println!("   Round {}: Simulating {} requests", round + 1, (round + 1) * 10);
        
        // Simulate varying load
        for i in 0..(round + 1) * 10 {
            let request_id = (round * 100 + i) as u64;
            performance_monitor.start_request(request_id);
            
            // Vary response times to simulate real conditions
            let delay = match round {
                0 => 30 + i * 2,  // Light load
                1 => 50 + i * 3,  // Medium load
                2 => 80 + i * 5,  // Heavy load
                _ => 50,
            };
            
            sleep(Duration::from_millis(delay as u64)).await;
            performance_monitor.end_request(request_id);
        }
        
        // Collect comprehensive snapshot
        let snapshot = aggregator.collect_snapshot().await.unwrap();
        
        println!("📊 Round {} Metrics Summary:", round + 1);
        println!("   Performance: {:.2}ms avg, {} active", 
            snapshot.performance_metrics.avg_response_time_ms,
            snapshot.performance_metrics.active_connections
        );
        println!("   DEX Liquidity: ${:.1}M total, {} pools", 
            snapshot.dex_liquidity_metrics.total_liquidity_usd as f64 / 1_000_000.0,
            snapshot.dex_liquidity_metrics.active_pools
        );
        println!("   Bridges: {}/{} active, {:.1}s avg time", 
            snapshot.bridge_status_metrics.active_bridges,
            snapshot.bridge_status_metrics.total_bridges,
            snapshot.bridge_status_metrics.average_bridge_time
        );
        println!("   System Health: {:.1}% overall", 
            snapshot.system_health_metrics.overall_health_score * 100.0
        );
        
        // Verify all metrics are populated
        assert!(snapshot.performance_metrics.total_processed > 0);
        assert!(snapshot.dex_liquidity_metrics.total_liquidity_usd > 0);
        assert!(snapshot.bridge_status_metrics.total_bridges > 0);
        assert!(snapshot.system_health_metrics.overall_health_score > 0.0);
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Final comprehensive query
    let comprehensive_query = MetricsQuery {
        start_time: Some(chrono::Utc::now() - chrono::Duration::minutes(10)),
        end_time: Some(chrono::Utc::now()),
        metric_types: vec![MetricType::All],
        aggregation: AggregationType::Raw,
        interval: None,
    };
    
    let final_results = aggregator.query_metrics(comprehensive_query).await.unwrap();
    
    println!("🎯 COMPREHENSIVE INTEGRATION RESULTS:");
    println!("   - Total data points collected: {}", final_results.summary.total_data_points);
    println!("   - Time span: {:.1} minutes", 
        (final_results.summary.time_range.1 - final_results.summary.time_range.0).num_seconds() as f64 / 60.0
    );
    println!("   - Key insights generated: {}", final_results.summary.key_insights.len());
    println!("   - Anomalies detected: {}", final_results.summary.anomalies_detected.len());
    
    // Verify comprehensive data collection
    assert!(final_results.summary.total_data_points > 0);
    assert!(!final_results.data_points.is_empty());
    assert!(!final_results.summary.key_insights.is_empty());
    
    for insight in &final_results.summary.key_insights {
        println!("   💡 {}", insight);
    }
    
    println!("🎉 COMPREHENSIVE METRICS INTEGRATION TESTS PASSED!");
    println!("🚀 REAL-TIME METRICS AGGREGATION SYSTEM IS FULLY OPERATIONAL!");
}
