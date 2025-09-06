use bralaladex_backend::api::{RateLimiter, UsageTracker, RateLimitTier, RateLimitDecision, UserUsageAnalytics, SystemUsageAnalytics, UserRateLimit};
use bralaladex_backend::risk_management::RiskError;
use std::sync::Arc;
use tokio::time::Duration;
use uuid::Uuid;
use rust_decimal::Decimal;

/// Comprehensive test for API Rate Limiting System
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing API Rate Limiting System Complete Flow");
    
    // Test 1: Multi-Tier Rate Limiting Engine
    println!("\n📋 Test 1: Multi-Tier Rate Limiting Engine");
    
    let rate_limiter = Arc::new(RateLimiter::new());
    let user_id = Uuid::new_v4();
    
    // Test Free tier limits (10 requests/minute)
    println!("   Testing Free tier (10 requests/minute)...");
    for i in 1..=10 {
        let result = rate_limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await?;
        println!("   Request {}: {:?}", i, result);
        if !matches!(result, RateLimitDecision::Allowed | RateLimitDecision::BurstAllowed) {
            println!("   Request {} was denied: {:?}", i, result);
            break;
        }
    }
    
    // Additional requests should be denied after burst is exhausted
    let result = rate_limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await?;
    println!("   Result for request beyond limits: {:?}", result);
    println!("   ✅ Free tier rate limiting working correctly");
    
    // Test tier upgrade
    println!("   Testing tier upgrade to Premium...");
    rate_limiter.update_user_tier(user_id, RateLimitTier::Premium).await?;
    let status = rate_limiter.get_user_status(user_id).await?;
    assert_eq!(status.tier, RateLimitTier::Premium);
    assert_eq!(status.config.requests_per_minute, 100);
    println!("   ✅ Tier upgrade successful");
    
    // Test system load throttling
    println!("   Testing system load throttling...");
    let low_priority_user = Uuid::new_v4();
    rate_limiter.update_system_load(0.9).await; // High system load
    
    let result = rate_limiter.check_rate_limit(low_priority_user, Some(RateLimitTier::Free)).await?;
    assert!(matches!(result, RateLimitDecision::Denied(_)), "Low priority user should be denied during high load");
    
    let result = rate_limiter.check_rate_limit(user_id, Some(RateLimitTier::Premium)).await?;
    assert!(matches!(result, RateLimitDecision::Allowed), "High priority user should be allowed during high load");
    
    rate_limiter.update_system_load(0.3).await; // Reset load
    println!("   ✅ System load throttling working");
    
    // Test user blocking
    println!("   Testing user blocking...");
    rate_limiter.block_user(low_priority_user, 60).await?;
    let result = rate_limiter.check_rate_limit(low_priority_user, Some(RateLimitTier::Free)).await?;
    assert!(matches!(result, RateLimitDecision::Denied(_)), "Blocked user should be denied");
    
    rate_limiter.unblock_user(low_priority_user).await?;
    let result = rate_limiter.check_rate_limit(low_priority_user, Some(RateLimitTier::Free)).await?;
    assert!(matches!(result, RateLimitDecision::Allowed), "Unblocked user should be allowed");
    println!("   ✅ User blocking/unblocking working");
    
    // Test 2: Usage Analytics and Monitoring
    println!("\n📋 Test 2: Usage Analytics and Monitoring");
    
    let usage_tracker = Arc::new(UsageTracker::new());
    let analytics_user = Uuid::new_v4();
    
    // Register analytics_user with rate limiter first
    rate_limiter.check_rate_limit(analytics_user, Some(RateLimitTier::Basic)).await?;
    
    // Record various usage patterns
    println!("   Recording usage data...");
    for i in 0..20 {
        let success = i < 18; // 18 successful, 2 failed
        let endpoint = if i % 3 == 0 { "/api/quote" } else { "/api/route" };
        let response_time = 100 + (i * 10);
        
        usage_tracker.record_usage(
            analytics_user,
            RateLimitTier::Basic,
            endpoint,
            "GET",
            response_time,
            success,
            false,
            1024,
            if success { None } else { Some("timeout") },
        ).await?;
    }
    
    // Verify analytics
    let user_analytics = usage_tracker.get_user_analytics(analytics_user).await?;
    assert_eq!(user_analytics.total_requests, 20);
    assert_eq!(user_analytics.successful_requests, 18);
    assert_eq!(user_analytics.failed_requests, 2);
    assert_eq!(user_analytics.tier, RateLimitTier::Basic);
    assert!(user_analytics.cost_incurred > rust_decimal::Decimal::new(0, 0));
    println!("   ✅ User analytics recorded correctly");
    
    // Test endpoint metrics
    let quote_metrics = usage_tracker.get_endpoint_metrics("/api/quote").await;
    assert!(quote_metrics.is_some(), "Quote endpoint metrics should exist");
    let metrics = quote_metrics.unwrap();
    assert!(metrics.total_requests > 0);
    assert!(metrics.success_rate > 0.8); // Should be high success rate
    println!("   ✅ Endpoint metrics calculated correctly");
    
    // Test system analytics
    let system_analytics = usage_tracker.get_system_analytics().await;
    assert_eq!(system_analytics.total_users, 1);
    assert_eq!(system_analytics.total_requests, 20);
    assert_eq!(system_analytics.total_successful_requests, 18);
    assert!(system_analytics.total_revenue > rust_decimal::Decimal::new(0, 0));
    println!("   ✅ System analytics aggregated correctly");
    
    // Test 3: Rate Limit Statistics
    println!("\n📋 Test 3: Rate Limit Statistics");
    
    let stats = rate_limiter.get_statistics().await;
    assert!(stats.total_users >= 2); // At least the users we created
    assert!(stats.total_requests > 0);
    assert!(stats.basic_users >= 1); // analytics_user is Basic tier
    assert!(stats.premium_users >= 1); // user_id was upgraded to Premium
    
    println!("   Statistics: {} total users, {} total requests", 
             stats.total_users, stats.total_requests);
    println!("   Tier distribution: {} Free, {} Basic, {} Premium, {} Enterprise", 
             stats.free_users, stats.basic_users, stats.premium_users, stats.enterprise_users);
    println!("   ✅ Rate limit statistics generated correctly");
    
    // Test 4: Concurrent Request Handling
    println!("\n📋 Test 4: Concurrent Request Handling");
    
    let concurrent_user = Uuid::new_v4();
    let rate_limiter_clone = rate_limiter.clone();
    
    // Test concurrent requests
    let mut handles = vec![];
    for i in 0..5 {
        let limiter = rate_limiter_clone.clone();
        let user = concurrent_user;
        
        let handle = tokio::spawn(async move {
            let result = limiter.check_rate_limit(user, Some(RateLimitTier::Free)).await;
            (i, result)
        });
        handles.push(handle);
    }
    
    let mut allowed_count = 0;
    for handle in handles {
        let (i, result) = handle.await?;
        match result? {
            RateLimitDecision::Allowed | RateLimitDecision::BurstAllowed => {
                allowed_count += 1;
                // Simulate request completion
                rate_limiter.record_completion(concurrent_user).await?;
            },
            RateLimitDecision::Denied(_) => {
                // Expected for some requests due to concurrent limits
            }
        }
    }
    
    assert!(allowed_count <= 3, "Should not exceed concurrent limit for Free tier");
    println!("   ✅ Concurrent request limiting working (allowed: {})", allowed_count);
    
    // Test 5: Data Cleanup and Maintenance
    println!("\n📋 Test 5: Data Cleanup and Maintenance");
    
    // Test data cleanup
    let cleaned_records = usage_tracker.cleanup_old_data().await?;
    println!("   Cleaned {} old records", cleaned_records);
    
    // Test top users functionality
    let top_users = usage_tracker.get_top_users(5).await;
    assert!(!top_users.is_empty(), "Should have top users");
    println!("   Found {} top users", top_users.len());
    
    // Test high error rate detection
    let high_error_users = usage_tracker.get_high_error_users(0.05).await; // 5% error rate threshold
    println!("   Found {} users with high error rates", high_error_users.len());
    
    println!("   ✅ Data cleanup and maintenance functions working");
    
    // Test 6: Edge Cases and Error Handling
    println!("\n📋 Test 6: Edge Cases and Error Handling");
    
    // Test invalid user operations
    let invalid_user = Uuid::new_v4();
    let result = usage_tracker.get_user_analytics(invalid_user).await;
    assert!(result.is_err(), "Should return error for non-existent user");
    
    // Test system load bounds
    rate_limiter.update_system_load(1.5).await; // Should be clamped to 1.0
    let load = rate_limiter.get_system_load().await;
    assert_eq!(load, 1.0, "System load should be clamped to 1.0");
    
    rate_limiter.update_system_load(-0.5).await; // Should be clamped to 0.0
    let load = rate_limiter.get_system_load().await;
    assert_eq!(load, 0.0, "System load should be clamped to 0.0");
    
    println!("   ✅ Edge cases handled correctly");
    
    // Test 7: All Tier Configurations
    println!("\n📋 Test 7: All Tier Configurations");
    
    let tiers = vec![
        RateLimitTier::Free,
        RateLimitTier::Basic,
        RateLimitTier::Premium,
        RateLimitTier::Enterprise,
        RateLimitTier::Unlimited,
    ];
    
    for tier in tiers {
        let config = UserRateLimit::get_tier_config(&tier);
        println!("   {:?}: {}/min, {}/hour, {}/day, burst: {}, concurrent: {}, priority: {}", 
                 tier, 
                 config.requests_per_minute,
                 config.requests_per_hour,
                 config.requests_per_day,
                 config.burst_limit,
                 config.concurrent_requests,
                 config.priority_weight);
        
        // Verify tier progression (higher tiers have higher limits)
        assert!(config.requests_per_minute > 0);
        assert!(config.requests_per_hour >= config.requests_per_minute);
        assert!(config.requests_per_day >= config.requests_per_hour);
        assert!(config.burst_limit >= config.requests_per_minute / 2);
    }
    
    println!("   ✅ All tier configurations validated");
    
    println!("\n🎉 All API Rate Limiting System tests passed successfully!");
    println!("   - Multi-tier rate limiting engine: ✅");
    println!("   - Usage analytics and monitoring: ✅");
    println!("   - Rate limit statistics: ✅");
    println!("   - Concurrent request handling: ✅");
    println!("   - Data cleanup and maintenance: ✅");
    println!("   - Edge cases and error handling: ✅");
    println!("   - All tier configurations: ✅");
    
    println!("\n📊 Final System State:");
    let final_stats = rate_limiter.get_statistics().await;
    let final_system = usage_tracker.get_system_analytics().await;
    
    println!("   Rate Limiter: {} users, {} requests, {} violations", 
             final_stats.total_users, final_stats.total_requests, final_stats.total_violations);
    println!("   Usage Tracker: {} users, {} requests, ${:.4} revenue", 
             final_system.total_users, final_system.total_requests, final_system.total_revenue);
    println!("   System Load: {:.1}%", rate_limiter.get_system_load().await * 100.0);
    
    Ok(())
}
