use crate::risk_management::types::{UserId, RiskError};
use crate::risk_management::redis_cache::RiskCache;
use crate::api::rate_limiter::{RateLimitTier, RateLimitDecision};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;

/// API endpoint usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointUsage {
    pub endpoint: String,
    pub method: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub average_response_time_ms: f64,
    pub last_accessed: u64,
    pub peak_requests_per_minute: u32,
    pub total_bytes_transferred: u64,
}

impl Default for EndpointUsage {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            method: String::new(),
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            rate_limited_requests: 0,
            average_response_time_ms: 0.0,
            last_accessed: 0,
            peak_requests_per_minute: 0,
            total_bytes_transferred: 0,
        }
    }
}

/// User usage analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsageAnalytics {
    pub user_id: UserId,
    pub tier: RateLimitTier,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub total_response_time_ms: u64,
    pub average_response_time_ms: f64,
    pub first_request: u64,
    pub last_request: u64,
    pub peak_requests_per_hour: u32,
    pub endpoint_usage: HashMap<String, EndpointUsage>,
    pub daily_usage: HashMap<String, u32>, // Date -> request count
    pub error_patterns: HashMap<String, u32>, // Error type -> count
    pub total_bytes_transferred: u64,
    pub cost_incurred: Decimal, // Based on tier pricing
}

impl UserUsageAnalytics {
    pub fn new(user_id: UserId, tier: RateLimitTier) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        Self {
            user_id,
            tier,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            rate_limited_requests: 0,
            total_response_time_ms: 0,
            average_response_time_ms: 0.0,
            first_request: now,
            last_request: now,
            peak_requests_per_hour: 0,
            endpoint_usage: HashMap::new(),
            daily_usage: HashMap::new(),
            error_patterns: HashMap::new(),
            total_bytes_transferred: 0,
            cost_incurred: Decimal::new(0, 0),
        }
    }

    /// Record a new API request
    pub fn record_request(
        &mut self,
        endpoint: &str,
        method: &str,
        response_time_ms: u64,
        success: bool,
        rate_limited: bool,
        bytes_transferred: u64,
        error_type: Option<&str>,
    ) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Update overall stats
        self.total_requests += 1;
        self.last_request = now;
        self.total_response_time_ms += response_time_ms;
        self.average_response_time_ms = self.total_response_time_ms as f64 / self.total_requests as f64;
        self.total_bytes_transferred += bytes_transferred;
        
        if success {
            self.successful_requests += 1;
        } else {
            self.failed_requests += 1;
        }
        
        if rate_limited {
            self.rate_limited_requests += 1;
        }
        
        // Update endpoint-specific stats
        let endpoint_key = format!("{} {}", method, endpoint);
        let endpoint_usage = self.endpoint_usage.entry(endpoint_key.clone()).or_insert_with(|| {
            EndpointUsage {
                endpoint: endpoint.to_string(),
                method: method.to_string(),
                ..Default::default()
            }
        });
        
        endpoint_usage.total_requests += 1;
        endpoint_usage.last_accessed = now;
        endpoint_usage.total_bytes_transferred += bytes_transferred;
        
        // Update average response time for endpoint
        let total_time = endpoint_usage.average_response_time_ms * (endpoint_usage.total_requests - 1) as f64;
        endpoint_usage.average_response_time_ms = (total_time + response_time_ms as f64) / endpoint_usage.total_requests as f64;
        
        if success {
            endpoint_usage.successful_requests += 1;
        } else {
            endpoint_usage.failed_requests += 1;
        }
        
        if rate_limited {
            endpoint_usage.rate_limited_requests += 1;
        }
        
        // Update daily usage
        let date_key = chrono::DateTime::from_timestamp(now as i64, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d")
            .to_string();
        *self.daily_usage.entry(date_key).or_insert(0) += 1;
        
        // Record error patterns
        if let Some(error) = error_type {
            *self.error_patterns.entry(error.to_string()).or_insert(0) += 1;
        }
        
        // Calculate cost based on tier
        self.update_cost();
    }
    
    /// Update cost calculation based on usage and tier
    fn update_cost(&mut self) {
        let cost_per_request = match self.tier {
            RateLimitTier::Free => Decimal::new(0, 0),
            RateLimitTier::Basic => Decimal::new(1, 4), // $0.0001 per request
            RateLimitTier::Premium => Decimal::new(5, 5), // $0.00005 per request
            RateLimitTier::Enterprise => Decimal::new(1, 5), // $0.00001 per request
            RateLimitTier::Unlimited => Decimal::new(5, 6), // $0.000005 per request
        };
        
        self.cost_incurred = cost_per_request * Decimal::from(self.total_requests);
    }
    
    /// Get usage summary for a specific time period
    pub fn get_period_summary(&self, days: u32) -> UsagePeriodSummary {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let period_start = now - (days as u64 * 86400);
        
        let mut period_requests = 0u32;
        let mut period_cost = Decimal::new(0, 0);
        
        for (date_str, count) in &self.daily_usage {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                let date_timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as u64;
                if date_timestamp >= period_start {
                    period_requests += count;
                }
            }
        }
        
        let cost_per_request = match self.tier {
            RateLimitTier::Free => Decimal::new(0, 0),
            RateLimitTier::Basic => Decimal::new(1, 4),
            RateLimitTier::Premium => Decimal::new(5, 5),
            RateLimitTier::Enterprise => Decimal::new(1, 5),
            RateLimitTier::Unlimited => Decimal::new(5, 6),
        };
        
        period_cost = cost_per_request * Decimal::from(period_requests);
        
        UsagePeriodSummary {
            period_days: days,
            total_requests: period_requests,
            cost_incurred: period_cost,
            average_requests_per_day: period_requests as f64 / days as f64,
        }
    }
}

/// Usage summary for a specific time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePeriodSummary {
    pub period_days: u32,
    pub total_requests: u32,
    pub cost_incurred: Decimal,
    pub average_requests_per_day: f64,
}

/// System-wide usage analytics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemUsageAnalytics {
    pub total_users: u32,
    pub total_requests: u64,
    pub total_successful_requests: u64,
    pub total_failed_requests: u64,
    pub total_rate_limited_requests: u64,
    pub average_response_time_ms: f64,
    pub total_bytes_transferred: u64,
    pub total_revenue: Decimal,
    pub top_endpoints: Vec<(String, u64)>, // endpoint -> request count
    pub top_users: Vec<(UserId, u64)>, // user -> request count
    pub error_distribution: HashMap<String, u32>,
    pub tier_distribution: HashMap<RateLimitTier, u32>,
    pub daily_request_trend: Vec<(String, u32)>, // date -> request count
}

/// Usage tracking and analytics engine
pub struct UsageTracker {
    user_analytics: Arc<RwLock<HashMap<UserId, UserUsageAnalytics>>>,
    system_analytics: Arc<RwLock<SystemUsageAnalytics>>,
    redis_cache: Option<Arc<RiskCache>>,
    retention_days: u32,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            user_analytics: Arc::new(RwLock::new(HashMap::new())),
            system_analytics: Arc::new(RwLock::new(SystemUsageAnalytics::default())),
            redis_cache: None,
            retention_days: 90, // Keep 90 days of data
        }
    }

    pub fn with_cache(cache: Arc<RiskCache>) -> Self {
        Self {
            user_analytics: Arc::new(RwLock::new(HashMap::new())),
            system_analytics: Arc::new(RwLock::new(SystemUsageAnalytics::default())),
            redis_cache: Some(cache),
            retention_days: 90,
        }
    }

    /// Record API usage
    pub async fn record_usage(
        &self,
        user_id: UserId,
        tier: RateLimitTier,
        endpoint: &str,
        method: &str,
        response_time_ms: u64,
        success: bool,
        rate_limited: bool,
        bytes_transferred: u64,
        error_type: Option<&str>,
    ) -> Result<(), RiskError> {
        // Update user analytics
        {
            let mut analytics = self.user_analytics.write().await;
            let user_analytics = analytics.entry(user_id).or_insert_with(|| {
                UserUsageAnalytics::new(user_id, tier.clone())
            });
            
            // Update tier if changed
            user_analytics.tier = tier.clone();
            
            user_analytics.record_request(
                endpoint,
                method,
                response_time_ms,
                success,
                rate_limited,
                bytes_transferred,
                error_type,
            );
        }

        // Update system analytics
        {
            let mut system = self.system_analytics.write().await;
            system.total_requests += 1;
            
            if success {
                system.total_successful_requests += 1;
            } else {
                system.total_failed_requests += 1;
            }
            
            if rate_limited {
                system.total_rate_limited_requests += 1;
            }
            
            system.total_bytes_transferred += bytes_transferred;
            
            // Update average response time
            let total_time = system.average_response_time_ms * (system.total_requests - 1) as f64;
            system.average_response_time_ms = (total_time + response_time_ms as f64) / system.total_requests as f64;
            
            // Update error distribution
            if let Some(error) = error_type {
                *system.error_distribution.entry(error.to_string()).or_insert(0) += 1;
            }
            
            // Update tier distribution
            *system.tier_distribution.entry(tier).or_insert(0) += 1;
        }

        Ok(())
    }

    /// Get user analytics
    pub async fn get_user_analytics(&self, user_id: UserId) -> Result<UserUsageAnalytics, RiskError> {
        let analytics = self.user_analytics.read().await;
        analytics.get(&user_id)
            .cloned()
            .ok_or_else(|| RiskError::RoutingError("User analytics not found".to_string()))
    }

    /// Get system analytics
    pub async fn get_system_analytics(&self) -> SystemUsageAnalytics {
        let mut system = self.system_analytics.write().await;
        
        // Update derived fields
        self.update_system_analytics_derived_fields(&mut system).await;
        
        system.clone()
    }

    /// Update derived fields in system analytics
    async fn update_system_analytics_derived_fields(&self, system: &mut SystemUsageAnalytics) {
        let analytics = self.user_analytics.read().await;
        
        // Update user count
        system.total_users = analytics.len() as u32;
        
        // Calculate total revenue
        system.total_revenue = analytics.values()
            .map(|user| user.cost_incurred)
            .fold(Decimal::new(0, 0), |acc, cost| acc + cost);
        
        // Update top endpoints
        let mut endpoint_counts: HashMap<String, u64> = HashMap::new();
        for user in analytics.values() {
            for (endpoint, usage) in &user.endpoint_usage {
                *endpoint_counts.entry(endpoint.clone()).or_insert(0) += usage.total_requests;
            }
        }
        
        let mut top_endpoints: Vec<_> = endpoint_counts.into_iter().collect();
        top_endpoints.sort_by(|a, b| b.1.cmp(&a.1));
        top_endpoints.truncate(10);
        system.top_endpoints = top_endpoints;
        
        // Update top users
        let mut user_counts: Vec<_> = analytics.iter()
            .map(|(id, analytics)| (*id, analytics.total_requests))
            .collect();
        user_counts.sort_by(|a, b| b.1.cmp(&a.1));
        user_counts.truncate(10);
        system.top_users = user_counts;
        
        // Update daily request trend (last 30 days)
        let mut daily_counts: HashMap<String, u32> = HashMap::new();
        for user in analytics.values() {
            for (date, count) in &user.daily_usage {
                *daily_counts.entry(date.clone()).or_insert(0) += count;
            }
        }
        
        let mut daily_trend: Vec<_> = daily_counts.into_iter().collect();
        daily_trend.sort_by(|a, b| a.0.cmp(&b.0));
        daily_trend.truncate(30);
        system.daily_request_trend = daily_trend;
    }

    /// Get usage analytics for multiple users
    pub async fn get_users_analytics(&self, user_ids: Vec<UserId>) -> Vec<UserUsageAnalytics> {
        let analytics = self.user_analytics.read().await;
        user_ids.into_iter()
            .filter_map(|id| analytics.get(&id).cloned())
            .collect()
    }

    /// Get top users by request count
    pub async fn get_top_users(&self, limit: usize) -> Vec<(UserId, UserUsageAnalytics)> {
        let analytics = self.user_analytics.read().await;
        let mut users: Vec<_> = analytics.iter()
            .map(|(id, analytics)| (*id, analytics.clone()))
            .collect();
        
        users.sort_by(|a, b| b.1.total_requests.cmp(&a.1.total_requests));
        users.truncate(limit);
        users
    }

    /// Get users with high error rates
    pub async fn get_high_error_users(&self, error_rate_threshold: f64) -> Vec<(UserId, f64)> {
        let analytics = self.user_analytics.read().await;
        analytics.iter()
            .filter_map(|(id, analytics)| {
                if analytics.total_requests > 0 {
                    let error_rate = analytics.failed_requests as f64 / analytics.total_requests as f64;
                    if error_rate >= error_rate_threshold {
                        Some((*id, error_rate))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get endpoint performance metrics
    pub async fn get_endpoint_metrics(&self, endpoint: &str) -> Option<EndpointMetrics> {
        let analytics = self.user_analytics.read().await;
        let mut total_requests = 0u64;
        let mut total_response_time = 0f64;
        let mut successful_requests = 0u64;
        let mut failed_requests = 0u64;
        let mut rate_limited_requests = 0u64;
        let mut total_bytes = 0u64;
        
        for user in analytics.values() {
            for (ep, usage) in &user.endpoint_usage {
                if ep.contains(endpoint) {
                    total_requests += usage.total_requests;
                    total_response_time += usage.average_response_time_ms * usage.total_requests as f64;
                    successful_requests += usage.successful_requests;
                    failed_requests += usage.failed_requests;
                    rate_limited_requests += usage.rate_limited_requests;
                    total_bytes += usage.total_bytes_transferred;
                }
            }
        }
        
        if total_requests > 0 {
            Some(EndpointMetrics {
                endpoint: endpoint.to_string(),
                total_requests,
                successful_requests,
                failed_requests,
                rate_limited_requests,
                average_response_time_ms: total_response_time / total_requests as f64,
                success_rate: successful_requests as f64 / total_requests as f64,
                error_rate: failed_requests as f64 / total_requests as f64,
                rate_limit_rate: rate_limited_requests as f64 / total_requests as f64,
                total_bytes_transferred: total_bytes,
            })
        } else {
            None
        }
    }

    /// Clean up old data based on retention policy
    pub async fn cleanup_old_data(&self) -> Result<u32, RiskError> {
        let cutoff_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() - (self.retention_days as u64 * 86400);
        
        let mut cleaned_records = 0u32;
        let mut analytics = self.user_analytics.write().await;
        
        for user_analytics in analytics.values_mut() {
            // Clean up daily usage data
            let old_keys: Vec<_> = user_analytics.daily_usage.keys()
                .filter(|date_str| {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        let date_timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as u64;
                        date_timestamp < cutoff_timestamp
                    } else {
                        true // Remove invalid date strings
                    }
                })
                .cloned()
                .collect();
            
            for key in old_keys {
                user_analytics.daily_usage.remove(&key);
                cleaned_records += 1;
            }
        }
        
        Ok(cleaned_records)
    }

    /// Export usage data for a user
    pub async fn export_user_data(&self, user_id: UserId) -> Result<UserDataExport, RiskError> {
        let analytics = self.user_analytics.read().await;
        let user_analytics = analytics.get(&user_id)
            .ok_or_else(|| RiskError::RoutingError("User not found".to_string()))?;
        
        Ok(UserDataExport {
            user_id,
            analytics: user_analytics.clone(),
            exported_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        })
    }
}

/// Endpoint performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetrics {
    pub endpoint: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub average_response_time_ms: f64,
    pub success_rate: f64,
    pub error_rate: f64,
    pub rate_limit_rate: f64,
    pub total_bytes_transferred: u64,
}

/// User data export structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDataExport {
    pub user_id: UserId,
    pub analytics: UserUsageAnalytics,
    pub exported_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_usage_tracking() {
        let tracker = UsageTracker::new();
        let user_id = Uuid::new_v4();
        
        // Record some usage
        tracker.record_usage(
            user_id,
            RateLimitTier::Premium,
            "/api/quote",
            "GET",
            150,
            true,
            false,
            1024,
            None,
        ).await.unwrap();
        
        // Get user analytics
        let analytics = tracker.get_user_analytics(user_id).await.unwrap();
        assert_eq!(analytics.total_requests, 1);
        assert_eq!(analytics.successful_requests, 1);
        assert_eq!(analytics.tier, RateLimitTier::Premium);
    }

    #[tokio::test]
    async fn test_endpoint_metrics() {
        let tracker = UsageTracker::new();
        let user_id = Uuid::new_v4();
        
        // Record multiple requests to same endpoint
        for i in 0..5 {
            tracker.record_usage(
                user_id,
                RateLimitTier::Basic,
                "/api/quote",
                "GET",
                100 + i * 10,
                i < 4, // 4 successful, 1 failed
                false,
                1024,
                if i == 4 { Some("timeout") } else { None },
            ).await.unwrap();
        }
        
        let metrics = tracker.get_endpoint_metrics("/api/quote").await.unwrap();
        assert_eq!(metrics.total_requests, 5);
        assert_eq!(metrics.successful_requests, 4);
        assert_eq!(metrics.failed_requests, 1);
        assert_eq!(metrics.success_rate, 0.8);
    }

    #[tokio::test]
    async fn test_system_analytics() {
        let tracker = UsageTracker::new();
        let user1 = Uuid::new_v4();
        let user2 = Uuid::new_v4();
        
        // Record usage for multiple users
        tracker.record_usage(user1, RateLimitTier::Free, "/api/quote", "GET", 100, true, false, 512, None).await.unwrap();
        tracker.record_usage(user2, RateLimitTier::Premium, "/api/route", "POST", 200, true, false, 1024, None).await.unwrap();
        
        let system_analytics = tracker.get_system_analytics().await;
        assert_eq!(system_analytics.total_users, 2);
        assert_eq!(system_analytics.total_requests, 2);
        assert_eq!(system_analytics.total_successful_requests, 2);
    }

    #[tokio::test]
    async fn test_cost_calculation() {
        let tracker = UsageTracker::new();
        let user_id = Uuid::new_v4();
        
        // Record 100 requests for premium user
        for _ in 0..100 {
            tracker.record_usage(
                user_id,
                RateLimitTier::Premium,
                "/api/quote",
                "GET",
                100,
                true,
                false,
                1024,
                None,
            ).await.unwrap();
        }
        
        let analytics = tracker.get_user_analytics(user_id).await.unwrap();
        assert!(analytics.cost_incurred > Decimal::new(0, 0));
        assert_eq!(analytics.total_requests, 100);
    }
}
