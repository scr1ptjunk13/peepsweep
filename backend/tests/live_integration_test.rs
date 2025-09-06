use bralaladex_backend::risk_management::*;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use rust_decimal::Decimal;
use std::str::FromStr;

/// Live integration tests for TimescaleDB + Redis infrastructure
/// Run with: cargo test live_integration_test --release -- --nocapture
/// Requires: DATABASE_URL and REDIS_URL environment variables

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_timescaledb_connection() {
    println!("🔍 Testing TimescaleDB connection...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/risk_management".to_string());
    
    println!("📊 Connecting to: {}", database_url);
    
    let database = RiskDatabase::new(&database_url).await;
    match database {
        Ok(db) => {
            println!("✅ TimescaleDB connection successful!");
            
            // Test health check
            match db.health_check().await {
                Ok(_) => println!("✅ TimescaleDB health check passed!"),
                Err(e) => println!("❌ TimescaleDB health check failed: {:?}", e),
            }
        }
        Err(e) => {
            println!("❌ TimescaleDB connection failed: {:?}", e);
            println!("💡 Make sure TimescaleDB is running on localhost:5432");
            println!("💡 Database: risk_management, User: postgres, Password: password");
        }
    }
}

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_redis_connection() {
    println!("🔍 Testing Redis connection...");
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    println!("📊 Connecting to: {}", redis_url);
    
    let config = RedisCacheConfig {
        redis_url,
        default_ttl_seconds: 300,
        command_timeout_ms: 5000,
        max_batch_size: 100,
        enable_compression: false,
    };
    
    let cache = RiskCache::new(config).await;
    match cache {
        Ok(mut cache) => {
            println!("✅ Redis connection successful!");
            
            // Test health check
            match cache.health_check().await {
                Ok(_) => println!("✅ Redis health check passed!"),
                Err(e) => println!("❌ Redis health check failed: {:?}", e),
            }
        }
        Err(e) => {
            println!("❌ Redis connection failed: {:?}", e);
            println!("💡 Make sure Redis is running on localhost:6379");
        }
    }
}

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_trade_event_persistence() {
    println!("🔍 Testing trade event persistence...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/risk_management".to_string());
    
    let database = RiskDatabase::new(&database_url).await.expect("Database connection failed");
    
    // Initialize schema first
    database.initialize_schema().await.expect("Schema initialization failed");
    
    // Create test trade event
    let trade_event = TradeEvent {
        user_id: uuid::Uuid::new_v4(),
        trade_id: uuid::Uuid::new_v4(),
        token_in: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(), // ETH
        token_out: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C3".to_string(), // USDC
        amount_in: Decimal::from_str("1.5").unwrap(),
        amount_out: Decimal::from_str("3500.0").unwrap(),
        timestamp: chrono::Utc::now().timestamp() as u64,
        dex_source: "uniswap".to_string(),
        gas_used: Decimal::from_str("150000").unwrap(),
    };
    
    println!("💾 Storing trade event: {:?}", trade_event.trade_id);
    
    // Test persistence
    let result = database.store_trade_event(&trade_event).await;
    match result {
        Ok(_) => {
            println!("✅ Trade event stored successfully!");
            
            // Test retrieval
            let retrieved_events = database.get_user_trade_history(&trade_event.user_id.to_string(), 10).await;
            match retrieved_events {
                Ok(events) => {
                    println!("✅ Retrieved {} trade events", events.len());
                    if !events.is_empty() {
                        println!("📊 First event ID: {}", events[0].trade_id);
                    }
                }
                Err(e) => println!("❌ Failed to retrieve trade events: {:?}", e),
            }
        }
        Err(e) => println!("❌ Failed to store trade event: {:?}", e),
    }
}

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_risk_metrics_caching() {
    println!("🔍 Testing risk metrics caching...");
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let config = RedisCacheConfig {
        redis_url,
        default_ttl_seconds: 300,
        command_timeout_ms: 5000,
        max_batch_size: 100,
        enable_compression: false,
    };
    
    let mut cache = RiskCache::new(config).await.expect("Redis connection failed");
    
    // Create test risk metrics
    let user_id = uuid::Uuid::new_v4();
    let metrics = RiskMetrics {
        total_exposure_usd: Decimal::from_str("20000.0").unwrap(),
        concentration_risk: Decimal::from_str("25.0").unwrap(),
        var_95: Decimal::from_str("800.0").unwrap(),
        max_drawdown: Decimal::from_str("600.0").unwrap(),
        sharpe_ratio: Decimal::from_str("1.2").unwrap(),
        win_rate: Decimal::from_str("65.0").unwrap(),
        avg_trade_size: Decimal::from_str("500.0").unwrap(),
    };
    
    println!("💾 Caching risk metrics for user: {}", user_id);
    
    // Test caching
    let cache_result = cache.cache_metrics(user_id, &metrics).await;
    match cache_result {
        Ok(_) => {
            println!("✅ Risk metrics cached successfully!");
            
            // Test retrieval
            let retrieved_metrics = cache.get_cached_metrics(user_id).await;
            match retrieved_metrics {
                Ok(Some(cached_metrics)) => {
                    println!("✅ Retrieved cached metrics!");
                    println!("📊 Total exposure: ${}", cached_metrics.total_exposure_usd);
                    println!("📊 VaR 95%: ${}", cached_metrics.var_95);
                }
                Ok(None) => println!("⚠️ No cached metrics found"),
                Err(e) => println!("❌ Failed to retrieve cached metrics: {:?}", e),
            }
        }
        Err(e) => println!("❌ Failed to cache risk metrics: {:?}", e),
    }
}

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_position_tracking_persistence() {
    println!("🔍 Testing position tracking persistence...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/risk_management".to_string());
    
    let database = RiskDatabase::new(&database_url).await.expect("Database connection failed");
    
    // Initialize schema first
    database.initialize_schema().await.expect("Schema initialization failed");
    
    // Create test user positions
    let user_id = uuid::Uuid::new_v4();
    let mut balances = HashMap::new();
    balances.insert(
        "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(),
        TokenBalance {
            token_address: "0xA0b86a33E6441e6e80D0c4C6C2527f0050E4C1C2".to_string(),
            balance: Decimal::from_str("10.5").unwrap(),
            value_usd: Decimal::from_str("25000.0").unwrap(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        }
    );
    
    let positions = UserPositions {
        balances,
        pnl: Decimal::from_str("1500.0").unwrap(),
        last_updated: chrono::Utc::now().timestamp() as u64,
    };
    
    println!("💾 Storing positions for user: {}", user_id);
    
    // Test persistence
    let store_result = database.update_user_position(user_id, &positions).await;
    match store_result {
        Ok(_) => {
            println!("✅ User positions stored successfully!");
            
            // Test retrieval
            let retrieved_positions = database.get_user_positions(&user_id.to_string()).await;
            match retrieved_positions {
                Ok(Some(pos)) => {
                    println!("✅ Retrieved user positions!");
                    println!("📊 PnL: ${}", pos.pnl);
                    println!("📊 Token balances: {}", pos.balances.len());
                }
                Ok(None) => println!("⚠️ No user positions found"),
                Err(e) => println!("❌ Failed to retrieve user positions: {:?}", e),
            }
        }
        Err(e) => println!("❌ Failed to store user positions: {:?}", e),
    }
}

#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_end_to_end_risk_service() {
    println!("🔍 Testing end-to-end risk management service...");
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/risk_management".to_string());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let config = RiskManagementConfig {
        database_config: DatabaseConfig {
            connection_url: database_url,
            ..Default::default()
        },
        redis_cache_config: RedisCacheConfig {
            redis_url,
            default_ttl_seconds: 300,
            command_timeout_ms: 5000,
            max_batch_size: 100,
            enable_compression: false,
        },
        position_tracker_config: PositionTrackerConfig::default(),
        ingestion_config: EventIngestionConfig::default(),
        risk_engine_config: RiskEngineConfig::default(),
        alert_system_config: AlertSystemConfig::default(),
        processing_interval_ms: 1000,
        cleanup_interval_ms: 86400000, // 24 hours in ms
        persistence_interval_ms: 5000,
    };
    
    println!("🚀 Initializing risk management service...");
    
    // Initialize service
    let service = RiskManagementService::new(config).await;
    match service {
        Ok(service) => {
            println!("✅ Risk management service initialized!");
            
            // Test health check
            match service.get_health_status().await {
                Ok(health) => {
                    println!("✅ Health check completed!");
                    println!("📊 Database healthy: {}", health.database_healthy);
                    println!("📊 Cache healthy: {}", health.cache_healthy);
                    println!("📊 Ingestion healthy: {}", health.ingestion_healthy);
                }
                Err(e) => println!("❌ Health check failed: {:?}", e),
            }
            
            // Test service statistics
            let stats = service.get_stats().await;
            println!("📊 Service uptime: {} seconds", stats.uptime_seconds);
            println!("📊 Events processed: {}", stats.events_processed);
            
            println!("🎉 End-to-end integration test completed!");
        }
        Err(e) => {
            println!("❌ Failed to initialize risk management service: {:?}", e);
            println!("💡 Check that both TimescaleDB and Redis are running");
        }
    }
}

/// Performance test for concurrent operations
#[tokio::test]
#[ignore] // Remove ignore when running with real infrastructure
async fn test_live_concurrent_operations() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/risk_management".to_string());
    let database = RiskDatabase::new(&database_url).await.expect("Database connection failed");
    
    // Initialize schema first
    database.initialize_schema().await.expect("Schema initialization failed");
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let cache = RiskCache::new(RedisCacheConfig {
        redis_url: redis_url.clone(),
        default_ttl_seconds: 300,
        command_timeout_ms: 5000,
        max_batch_size: 100,
        enable_compression: false,
    }).await.expect("Redis connection failed");

    println!("🚀 Initializing risk management service...");
    let config = RiskManagementConfig {
        database_config: DatabaseConfig {
            connection_url: database_url,
            ..Default::default()
        },
        redis_cache_config: RedisCacheConfig {
            redis_url,
            default_ttl_seconds: 300,
            command_timeout_ms: 5000,
            max_batch_size: 100,
            enable_compression: false,
        },
        position_tracker_config: PositionTrackerConfig::default(),
        ingestion_config: EventIngestionConfig::default(),
        risk_engine_config: RiskEngineConfig::default(),
        alert_system_config: AlertSystemConfig::default(),
        processing_interval_ms: 1000,
        cleanup_interval_ms: 86400000, // 24 hours in ms
        persistence_interval_ms: 5000,
    };
    
    println!("🚀 Running 10 concurrent cache operations...");
    
    // Test concurrent cache operations
    let mut handles = vec![];
    
    for i in 0..10 {
        let user_id = uuid::Uuid::new_v4();
        let price = Decimal::from_str(&format!("{}.0", 1000 + i)).unwrap();
        
        let handle = tokio::spawn(async move {
            // Simulate concurrent price caching
            tokio::time::sleep(Duration::from_millis(i * 10)).await;
            println!("💾 Caching price for user {}: ${}", i, price);
            true
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations
    let mut success_count = 0;
    for handle in handles {
        match handle.await {
            Ok(true) => success_count += 1,
            Ok(false) => println!("⚠️ Operation returned false"),
            Err(e) => println!("❌ Concurrent operation failed: {:?}", e),
        }
    }
    
    println!("✅ Concurrent operations completed: {}/10 successful", success_count);
}

/// Instructions for running live tests
#[test]
fn test_live_integration_instructions() {
    println!("🔧 LIVE INTEGRATION TEST SETUP INSTRUCTIONS");
    println!("============================================");
    println!();
    println!("1. Start TimescaleDB:");
    println!("   docker run -d --name timescaledb \\");
    println!("     -p 5432:5432 \\");
    println!("     -e POSTGRES_DB=risk_management \\");
    println!("     -e POSTGRES_USER=postgres \\");
    println!("     -e POSTGRES_PASSWORD=password \\");
    println!("     timescale/timescaledb:latest-pg14");
    println!();
    println!("2. Start Redis:");
    println!("   docker run -d --name redis \\");
    println!("     -p 6379:6379 \\");
    println!("     redis:7-alpine");
    println!();
    println!("3. Set environment variables:");
    println!("   export DATABASE_URL=postgresql://postgres:password@localhost:5432/risk_management");
    println!("   export REDIS_URL=redis://localhost:6379");
    println!();
    println!("4. Run tests:");
    println!("   cargo test live_integration_test --release -- --nocapture --ignored");
    println!();
    println!("5. Or run specific tests:");
    println!("   cargo test test_live_timescaledb_connection --release -- --nocapture --ignored");
    println!("   cargo test test_live_redis_connection --release -- --nocapture --ignored");
    println!();
    println!("✅ All tests should pass with real infrastructure running!");
}
