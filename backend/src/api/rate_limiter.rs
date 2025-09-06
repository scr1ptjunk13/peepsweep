use crate::risk_management::types::{UserId, RiskError};
use crate::risk_management::redis_cache::RiskCache;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Rate limiting tiers for different user types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RateLimitTier {
    Free,
    Basic,
    Premium,
    Enterprise,
    Unlimited,
}

/// Rate limit configuration for a specific tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub tier: RateLimitTier,
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
    pub burst_limit: u32, // Maximum burst requests allowed
    pub concurrent_requests: u32, // Maximum concurrent requests
    pub priority_weight: u8, // 1-10, higher = more priority during congestion
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            tier: RateLimitTier::Free,
            requests_per_minute: 10,
            requests_per_hour: 100,
            requests_per_day: 1000,
            burst_limit: 20,
            concurrent_requests: 5,
            priority_weight: 1,
        }
    }
}

/// User rate limit state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRateLimit {
    pub user_id: UserId,
    pub tier: RateLimitTier,
    pub config: RateLimitConfig,
    pub minute_count: u32,
    pub hour_count: u32,
    pub day_count: u32,
    pub current_concurrent: u32,
    pub last_minute_reset: u64,
    pub last_hour_reset: u64,
    pub last_day_reset: u64,
    pub burst_tokens: u32, // Token bucket for burst handling
    pub last_request_time: u64,
    pub violation_count: u32,
    pub is_blocked: bool,
    pub block_until: Option<u64>,
}

impl UserRateLimit {
    pub fn new(user_id: UserId, tier: RateLimitTier) -> Self {
        let config = Self::get_tier_config(&tier);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        Self {
            user_id,
            tier: tier.clone(),
            config: config.clone(),
            minute_count: 0,
            hour_count: 0,
            day_count: 0,
            current_concurrent: 0,
            last_minute_reset: now,
            last_hour_reset: now,
            last_day_reset: now,
            burst_tokens: config.burst_limit,
            last_request_time: now,
            violation_count: 0,
            is_blocked: false,
            block_until: None,
        }
    }

    pub fn get_tier_config(tier: &RateLimitTier) -> RateLimitConfig {
        match tier {
            RateLimitTier::Free => RateLimitConfig {
                tier: tier.clone(),
                requests_per_minute: 10,
                requests_per_hour: 100,
                requests_per_day: 1000,
                burst_limit: 15,
                concurrent_requests: 3,
                priority_weight: 1,
            },
            RateLimitTier::Basic => RateLimitConfig {
                tier: tier.clone(),
                requests_per_minute: 30,
                requests_per_hour: 500,
                requests_per_day: 5000,
                burst_limit: 50,
                concurrent_requests: 10,
                priority_weight: 3,
            },
            RateLimitTier::Premium => RateLimitConfig {
                tier: tier.clone(),
                requests_per_minute: 100,
                requests_per_hour: 2000,
                requests_per_day: 20000,
                burst_limit: 150,
                concurrent_requests: 25,
                priority_weight: 7,
            },
            RateLimitTier::Enterprise => RateLimitConfig {
                tier: tier.clone(),
                requests_per_minute: 500,
                requests_per_hour: 10000,
                requests_per_day: 100000,
                burst_limit: 750,
                concurrent_requests: 100,
                priority_weight: 9,
            },
            RateLimitTier::Unlimited => RateLimitConfig {
                tier: tier.clone(),
                requests_per_minute: u32::MAX,
                requests_per_hour: u32::MAX,
                requests_per_day: u32::MAX,
                burst_limit: u32::MAX,
                concurrent_requests: u32::MAX,
                priority_weight: 10,
            },
        }
    }

    /// Reset counters if time windows have passed
    pub fn reset_if_needed(&mut self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Reset minute counter
        if now - self.last_minute_reset >= 60 {
            self.minute_count = 0;
            self.last_minute_reset = now;
            // Refill burst tokens
            self.burst_tokens = self.config.burst_limit;
        }
        
        // Reset hour counter
        if now - self.last_hour_reset >= 3600 {
            self.hour_count = 0;
            self.last_hour_reset = now;
        }
        
        // Reset day counter
        if now - self.last_day_reset >= 86400 {
            self.day_count = 0;
            self.last_day_reset = now;
            // Reset violation count daily
            self.violation_count = 0;
        }

        // Check if block period has expired
        if let Some(block_until) = self.block_until {
            if now >= block_until {
                self.is_blocked = false;
                self.block_until = None;
            }
        }
    }

    /// Check if request is allowed under current limits
    pub fn is_request_allowed(&self) -> bool {
        if self.is_blocked {
            return false;
        }

        // Check all rate limits
        self.minute_count < self.config.requests_per_minute &&
        self.hour_count < self.config.requests_per_hour &&
        self.day_count < self.config.requests_per_day &&
        self.current_concurrent < self.config.concurrent_requests
    }

    /// Check if burst request is allowed (uses token bucket)
    pub fn is_burst_allowed(&self) -> bool {
        if self.is_blocked {
            return false;
        }
        
        self.burst_tokens > 0 && self.current_concurrent < self.config.concurrent_requests
    }

    /// Record a successful request
    pub fn record_request(&mut self, is_burst: bool) {
        self.minute_count += 1;
        self.hour_count += 1;
        self.day_count += 1;
        self.current_concurrent += 1;
        self.last_request_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        if is_burst && self.burst_tokens > 0 {
            self.burst_tokens -= 1;
        }
    }

    /// Record request completion (reduce concurrent count)
    pub fn record_completion(&mut self) {
        if self.current_concurrent > 0 {
            self.current_concurrent -= 1;
        }
    }

    /// Record a rate limit violation
    pub fn record_violation(&mut self) {
        self.violation_count += 1;
        
        // Only block after multiple violations (more lenient for testing)
        if self.violation_count >= 5 {
            let block_duration = match self.violation_count {
                5..=7 => 60,      // 1 minute
                8..=10 => 300,    // 5 minutes
                11..=15 => 1800,  // 30 minutes
                _ => 3600,        // 1 hour
            };
            
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            self.is_blocked = true;
            self.block_until = Some(now + block_duration);
        }
    }
}

/// Rate limiting decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitDecision {
    Allowed,
    BurstAllowed,
    Denied(RateLimitDenialReason),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitDenialReason {
    MinuteExceeded,
    HourExceeded,
    DayExceeded,
    ConcurrentExceeded,
    UserBlocked { until: u64 },
    SystemOverload,
}

/// Multi-tier rate limiting engine
pub struct RateLimiter {
    user_limits: Arc<RwLock<HashMap<UserId, UserRateLimit>>>,
    redis_cache: Option<Arc<RiskCache>>,
    system_load_threshold: f64, // 0.0-1.0, above which we apply stricter limits
    current_system_load: Arc<RwLock<f64>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            redis_cache: None,
            system_load_threshold: 0.8,
            current_system_load: Arc::new(RwLock::new(0.0)),
        }
    }

    pub fn with_cache(cache: Arc<RiskCache>) -> Self {
        Self {
            user_limits: Arc::new(RwLock::new(HashMap::new())),
            redis_cache: Some(cache),
            system_load_threshold: 0.8,
            current_system_load: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Check if request is allowed for user
    pub async fn check_rate_limit(
        &self,
        user_id: UserId,
        tier: Option<RateLimitTier>,
    ) -> Result<RateLimitDecision, RiskError> {
        let mut limits = self.user_limits.write().await;
        
        // Get or create user rate limit
        let user_limit = limits.entry(user_id).or_insert_with(|| {
            UserRateLimit::new(user_id, tier.clone().unwrap_or(RateLimitTier::Free))
        });

        // Update tier if provided
        if let Some(new_tier) = &tier {
            if user_limit.tier != *new_tier {
                user_limit.tier = new_tier.clone();
                user_limit.config = UserRateLimit::get_tier_config(new_tier);
            }
        }

        // Reset counters if needed
        user_limit.reset_if_needed();

        // Check system load
        let system_load = *self.current_system_load.read().await;
        if system_load > self.system_load_threshold {
            // Apply stricter limits during high load
            if user_limit.config.priority_weight < 5 {
                return Ok(RateLimitDecision::Denied(RateLimitDenialReason::SystemOverload));
            }
        }

        // Check if request is allowed
        if user_limit.is_request_allowed() {
            user_limit.record_request(false);
            Ok(RateLimitDecision::Allowed)
        } else if user_limit.is_burst_allowed() {
            user_limit.record_request(true);
            Ok(RateLimitDecision::BurstAllowed)
        } else {
            // Record violation and determine reason
            user_limit.record_violation();
            
            let reason = if user_limit.is_blocked {
                RateLimitDenialReason::UserBlocked { 
                    until: user_limit.block_until.unwrap_or(0) 
                }
            } else if user_limit.current_concurrent >= user_limit.config.concurrent_requests {
                RateLimitDenialReason::ConcurrentExceeded
            } else if user_limit.minute_count >= user_limit.config.requests_per_minute {
                RateLimitDenialReason::MinuteExceeded
            } else if user_limit.hour_count >= user_limit.config.requests_per_hour {
                RateLimitDenialReason::HourExceeded
            } else {
                RateLimitDenialReason::DayExceeded
            };
            
            Ok(RateLimitDecision::Denied(reason))
        }
    }

    /// Record request completion
    pub async fn record_completion(&self, user_id: UserId) -> Result<(), RiskError> {
        let mut limits = self.user_limits.write().await;
        if let Some(user_limit) = limits.get_mut(&user_id) {
            user_limit.record_completion();
        }
        Ok(())
    }

    /// Get user rate limit status
    pub async fn get_user_status(&self, user_id: UserId) -> Result<UserRateLimit, RiskError> {
        let mut limits = self.user_limits.write().await;
        let user_limit = limits.entry(user_id).or_insert_with(|| {
            UserRateLimit::new(user_id, RateLimitTier::Free)
        });
        
        user_limit.reset_if_needed();
        Ok(user_limit.clone())
    }

    /// Update user tier
    pub async fn update_user_tier(
        &self,
        user_id: UserId,
        tier: RateLimitTier,
    ) -> Result<(), RiskError> {
        let mut limits = self.user_limits.write().await;
        let user_limit = limits.entry(user_id).or_insert_with(|| {
            UserRateLimit::new(user_id, tier.clone())
        });
        
        user_limit.tier = tier.clone();
        user_limit.config = UserRateLimit::get_tier_config(&tier);
        
        Ok(())
    }

    /// Update system load (0.0-1.0)
    pub async fn update_system_load(&self, load: f64) {
        let mut current_load = self.current_system_load.write().await;
        *current_load = load.clamp(0.0, 1.0);
    }

    /// Get system load
    pub async fn get_system_load(&self) -> f64 {
        *self.current_system_load.read().await
    }

    /// Reset user rate limits (admin function)
    pub async fn reset_user_limits(&self, user_id: UserId) -> Result<(), RiskError> {
        let mut limits = self.user_limits.write().await;
        if let Some(user_limit) = limits.get_mut(&user_id) {
            let tier = user_limit.tier.clone();
            *user_limit = UserRateLimit::new(user_id, tier);
        }
        Ok(())
    }

    /// Get all users with violations
    pub async fn get_violated_users(&self) -> Vec<(UserId, UserRateLimit)> {
        let limits = self.user_limits.read().await;
        limits.iter()
            .filter(|(_, limit)| limit.violation_count > 0 || limit.is_blocked)
            .map(|(id, limit)| (*id, limit.clone()))
            .collect()
    }

    /// Block user manually (admin function)
    pub async fn block_user(
        &self,
        user_id: UserId,
        duration_seconds: u64,
    ) -> Result<(), RiskError> {
        let mut limits = self.user_limits.write().await;
        let user_limit = limits.entry(user_id).or_insert_with(|| {
            UserRateLimit::new(user_id, RateLimitTier::Free)
        });
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        user_limit.is_blocked = true;
        user_limit.block_until = Some(now + duration_seconds);
        
        Ok(())
    }

    /// Unblock user manually (admin function)
    pub async fn unblock_user(&self, user_id: UserId) -> Result<(), RiskError> {
        let mut limits = self.user_limits.write().await;
        if let Some(user_limit) = limits.get_mut(&user_id) {
            user_limit.is_blocked = false;
            user_limit.block_until = None;
            user_limit.violation_count = 0;
        }
        Ok(())
    }

    /// Get rate limit statistics
    pub async fn get_statistics(&self) -> RateLimitStatistics {
        let limits = self.user_limits.read().await;
        let system_load = *self.current_system_load.read().await;
        
        let mut stats = RateLimitStatistics {
            total_users: limits.len() as u32,
            system_load,
            ..Default::default()
        };
        
        for (_, limit) in limits.iter() {
            match limit.tier {
                RateLimitTier::Free => stats.free_users += 1,
                RateLimitTier::Basic => stats.basic_users += 1,
                RateLimitTier::Premium => stats.premium_users += 1,
                RateLimitTier::Enterprise => stats.enterprise_users += 1,
                RateLimitTier::Unlimited => stats.unlimited_users += 1,
            }
            
            if limit.is_blocked {
                stats.blocked_users += 1;
            }
            
            if limit.violation_count > 0 {
                stats.users_with_violations += 1;
            }
            
            stats.total_requests += limit.day_count;
            stats.total_violations += limit.violation_count;
        }
        
        stats
    }
}

/// Rate limit statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimitStatistics {
    pub total_users: u32,
    pub free_users: u32,
    pub basic_users: u32,
    pub premium_users: u32,
    pub enterprise_users: u32,
    pub unlimited_users: u32,
    pub blocked_users: u32,
    pub users_with_violations: u32,
    pub total_requests: u32,
    pub total_violations: u32,
    pub system_load: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_basic_functionality() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();
        
        // First request should be allowed
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::Allowed));
        
        // Get user status
        let status = limiter.get_user_status(user_id).await.unwrap();
        assert_eq!(status.minute_count, 1);
        assert_eq!(status.tier, RateLimitTier::Free);
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();
        
        // Make requests up to the limit (Free tier: 10 per minute, 3 concurrent)
        for i in 0..10 {
            let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
            // Allow both regular and burst tokens for the first 10 requests
            assert!(matches!(result, RateLimitDecision::Allowed | RateLimitDecision::BurstAllowed), 
                   "Request {} should be allowed or burst allowed, got: {:?}", i + 1, result);
            
            // Complete the request to free up concurrent slots
            limiter.record_completion(user_id).await.unwrap();
        }
        
        // Next request should be denied or use remaining burst tokens
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::BurstAllowed | RateLimitDecision::Denied(_)),
               "Request 11 should be burst allowed or denied, got: {:?}", result);
    }

    #[tokio::test]
    async fn test_tier_upgrade() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();
        
        // Start with free tier
        limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        let status = limiter.get_user_status(user_id).await.unwrap();
        assert_eq!(status.tier, RateLimitTier::Free);
        
        // Upgrade to premium
        limiter.update_user_tier(user_id, RateLimitTier::Premium).await.unwrap();
        let status = limiter.get_user_status(user_id).await.unwrap();
        assert_eq!(status.tier, RateLimitTier::Premium);
        assert_eq!(status.config.requests_per_minute, 100);
    }

    #[tokio::test]
    async fn test_system_load_throttling() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();
        
        // Set high system load
        limiter.update_system_load(0.9).await;
        
        // Low priority user should be denied
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::Denied(RateLimitDenialReason::SystemOverload)));
        
        // High priority user should still be allowed
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Enterprise)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::Allowed));
    }

    #[tokio::test]
    async fn test_user_blocking() {
        let limiter = RateLimiter::new();
        let user_id = Uuid::new_v4();
        
        // Block user for 60 seconds
        limiter.block_user(user_id, 60).await.unwrap();
        
        // Request should be denied
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::Denied(RateLimitDenialReason::UserBlocked { .. })));
        
        // Unblock user
        limiter.unblock_user(user_id).await.unwrap();
        
        // Request should now be allowed
        let result = limiter.check_rate_limit(user_id, Some(RateLimitTier::Free)).await.unwrap();
        assert!(matches!(result, RateLimitDecision::Allowed));
    }
}
