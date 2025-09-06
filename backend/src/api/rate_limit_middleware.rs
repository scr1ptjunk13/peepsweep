use crate::api::rate_limiter::{RateLimiter, RateLimitDecision, RateLimitDenialReason, RateLimitTier};
use crate::api::usage_tracker::UsageTracker;
use crate::risk_management::types::UserId;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Rate limiting middleware state
#[derive(Clone)]
pub struct RateLimitMiddlewareState {
    pub rate_limiter: Arc<RateLimiter>,
    pub usage_tracker: Arc<UsageTracker>,
}

impl RateLimitMiddlewareState {
    pub fn new(rate_limiter: Arc<RateLimiter>, usage_tracker: Arc<UsageTracker>) -> Self {
        Self {
            rate_limiter,
            usage_tracker,
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<RateLimitMiddlewareState>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = SystemTime::now();
    
    // Extract user ID from headers or create anonymous user
    let user_id = extract_user_id(&request);
    let tier = extract_user_tier(&request);
    
    // Get request info for tracking
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let endpoint = format!("{} {}", method, path);
    
    // Check rate limit
    let rate_limit_result = state.rate_limiter
        .check_rate_limit(user_id, tier.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match rate_limit_result {
        RateLimitDecision::Allowed => {
            // Process request normally
            let response = next.run(request).await;
            
            // Record completion
            if let Err(_) = state.rate_limiter.record_completion(user_id).await {
                // Log error but don't fail the request
            }
            
            // Track usage
            let response_time = start_time.elapsed().unwrap_or_default().as_millis() as u64;
            let success = response.status().is_success();
            let bytes_transferred = estimate_response_size(&response);
            
            if let Err(_) = state.usage_tracker.record_usage(
                user_id,
                tier.unwrap_or(RateLimitTier::Free),
                &path,
                &method,
                response_time,
                success,
                false, // not rate limited
                bytes_transferred,
                if success { None } else { Some("http_error") },
            ).await {
                // Log error but don't fail the request
            }
            
            // Add rate limit headers
            let mut response = response;
            add_rate_limit_headers(&mut response, &state, user_id).await;
            
            Ok(response)
        },
        RateLimitDecision::BurstAllowed => {
            // Process request with burst allowance
            let response = next.run(request).await;
            
            // Record completion
            if let Err(_) = state.rate_limiter.record_completion(user_id).await {
                // Log error but don't fail the request
            }
            
            // Track usage
            let response_time = start_time.elapsed().unwrap_or_default().as_millis() as u64;
            let success = response.status().is_success();
            let bytes_transferred = estimate_response_size(&response);
            
            if let Err(_) = state.usage_tracker.record_usage(
                user_id,
                tier.unwrap_or(RateLimitTier::Free),
                &path,
                &method,
                response_time,
                success,
                false, // not rate limited (burst allowed)
                bytes_transferred,
                if success { None } else { Some("http_error") },
            ).await {
                // Log error but don't fail the request
            }
            
            // Add rate limit headers with burst indication
            let mut response = response;
            add_rate_limit_headers(&mut response, &state, user_id).await;
            response.headers_mut().insert("X-RateLimit-Burst", HeaderValue::from_static("true"));
            
            Ok(response)
        },
        RateLimitDecision::Denied(reason) => {
            // Track the rate limited request
            let response_time = start_time.elapsed().unwrap_or_default().as_millis() as u64;
            
            if let Err(_) = state.usage_tracker.record_usage(
                user_id,
                tier.unwrap_or(RateLimitTier::Free),
                &path,
                &method,
                response_time,
                false, // not successful
                true,  // rate limited
                0,     // no bytes transferred
                Some("rate_limited"),
            ).await {
                // Log error but continue
            }
            
            // Create rate limit response
            let (status_code, error_message, retry_after) = match reason {
                RateLimitDenialReason::MinuteExceeded => {
                    (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded: too many requests per minute", Some(60))
                },
                RateLimitDenialReason::HourExceeded => {
                    (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded: too many requests per hour", Some(3600))
                },
                RateLimitDenialReason::DayExceeded => {
                    (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded: daily limit reached", Some(86400))
                },
                RateLimitDenialReason::ConcurrentExceeded => {
                    (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded: too many concurrent requests", Some(10))
                },
                RateLimitDenialReason::UserBlocked { until } => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                    let retry_seconds = if until > now { until - now } else { 0 };
                    (StatusCode::TOO_MANY_REQUESTS, "User temporarily blocked due to violations", Some(retry_seconds))
                },
                RateLimitDenialReason::SystemOverload => {
                    (StatusCode::SERVICE_UNAVAILABLE, "System overloaded, please try again later", Some(30))
                },
            };
            
            let error_response = json!({
                "error": "rate_limit_exceeded",
                "message": error_message,
                "retry_after_seconds": retry_after,
                "user_id": user_id.to_string(),
                "endpoint": endpoint,
                "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            });
            
            let mut response = Response::builder()
                .status(status_code)
                .header("Content-Type", "application/json")
                .body(error_response.to_string().into())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            // Add rate limit headers
            add_rate_limit_headers(&mut response, &state, user_id).await;
            
            if let Some(retry_seconds) = retry_after {
                response.headers_mut().insert(
                    "Retry-After",
                    HeaderValue::from_str(&retry_seconds.to_string()).unwrap_or(HeaderValue::from_static("60"))
                );
            }
            
            Ok(response)
        }
    }
}

/// Extract user ID from request headers
fn extract_user_id(request: &Request) -> UserId {
    // Try to get user ID from Authorization header
    if let Some(auth_header) = request.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            // Handle Bearer token format
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if let Ok(user_id) = Uuid::parse_str(token) {
                    return user_id;
                }
            }
        }
    }
    
    // Try to get user ID from X-User-ID header
    if let Some(user_header) = request.headers().get("X-User-ID") {
        if let Ok(user_str) = user_header.to_str() {
            if let Ok(user_id) = Uuid::parse_str(user_str) {
                return user_id;
            }
        }
    }
    
    // Try to get from X-API-Key header (assuming API key contains user ID)
    if let Some(api_key) = request.headers().get("X-API-Key") {
        if let Ok(key_str) = api_key.to_str() {
            // Simple format: "user_id:secret" or just user_id
            if let Some(user_part) = key_str.split(':').next() {
                if let Ok(user_id) = Uuid::parse_str(user_part) {
                    return user_id;
                }
            }
        }
    }
    
    // Generate anonymous user ID based on IP (if available)
    if let Some(forwarded) = request.headers().get("X-Forwarded-For") {
        if let Ok(ip_str) = forwarded.to_str() {
            if let Some(ip) = ip_str.split(',').next() {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                ip.trim().hash(&mut hasher);
                let hash = hasher.finish();
                return Uuid::from_u128(hash as u128);
            }
        }
    }
    
    // Fallback to a default anonymous user
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    "anonymous".hash(&mut hasher);
    let hash = hasher.finish();
    Uuid::from_u128(hash as u128)
}

/// Extract user tier from request headers
fn extract_user_tier(request: &Request) -> Option<RateLimitTier> {
    // Check X-User-Tier header
    if let Some(tier_header) = request.headers().get("X-User-Tier") {
        if let Ok(tier_str) = tier_header.to_str() {
            return match tier_str.to_lowercase().as_str() {
                "free" => Some(RateLimitTier::Free),
                "basic" => Some(RateLimitTier::Basic),
                "premium" => Some(RateLimitTier::Premium),
                "enterprise" => Some(RateLimitTier::Enterprise),
                "unlimited" => Some(RateLimitTier::Unlimited),
                _ => None,
            };
        }
    }
    
    // Check Authorization header for tier info
    if let Some(auth_header) = request.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            // Look for tier in Bearer token format: "Bearer user_id:tier:secret"
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                let parts: Vec<&str> = token.split(':').collect();
                if parts.len() >= 2 {
                    return match parts[1].to_lowercase().as_str() {
                        "free" => Some(RateLimitTier::Free),
                        "basic" => Some(RateLimitTier::Basic),
                        "premium" => Some(RateLimitTier::Premium),
                        "enterprise" => Some(RateLimitTier::Enterprise),
                        "unlimited" => Some(RateLimitTier::Unlimited),
                        _ => None,
                    };
                }
            }
        }
    }
    
    None // Default to None, will use Free tier
}

/// Add rate limit headers to response
async fn add_rate_limit_headers(
    response: &mut Response,
    state: &RateLimitMiddlewareState,
    user_id: UserId,
) {
    if let Ok(user_status) = state.rate_limiter.get_user_status(user_id).await {
        let headers = response.headers_mut();
        
        // Add standard rate limit headers
        headers.insert(
            "X-RateLimit-Limit-Minute",
            HeaderValue::from_str(&user_status.config.requests_per_minute.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Limit-Hour",
            HeaderValue::from_str(&user_status.config.requests_per_hour.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Limit-Day",
            HeaderValue::from_str(&user_status.config.requests_per_day.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        // Add remaining requests
        let remaining_minute = user_status.config.requests_per_minute.saturating_sub(user_status.minute_count);
        let remaining_hour = user_status.config.requests_per_hour.saturating_sub(user_status.hour_count);
        let remaining_day = user_status.config.requests_per_day.saturating_sub(user_status.day_count);
        
        headers.insert(
            "X-RateLimit-Remaining-Minute",
            HeaderValue::from_str(&remaining_minute.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Remaining-Hour",
            HeaderValue::from_str(&remaining_hour.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Remaining-Day",
            HeaderValue::from_str(&remaining_day.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        // Add reset times
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let minute_reset = user_status.last_minute_reset + 60;
        let hour_reset = user_status.last_hour_reset + 3600;
        let day_reset = user_status.last_day_reset + 86400;
        
        headers.insert(
            "X-RateLimit-Reset-Minute",
            HeaderValue::from_str(&minute_reset.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Reset-Hour",
            HeaderValue::from_str(&hour_reset.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Reset-Day",
            HeaderValue::from_str(&day_reset.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        // Add tier information
        let tier_str = match user_status.tier {
            RateLimitTier::Free => "free",
            RateLimitTier::Basic => "basic",
            RateLimitTier::Premium => "premium",
            RateLimitTier::Enterprise => "enterprise",
            RateLimitTier::Unlimited => "unlimited",
        };
        
        headers.insert(
            "X-RateLimit-Tier",
            HeaderValue::from_static(tier_str)
        );
        
        // Add burst tokens remaining
        headers.insert(
            "X-RateLimit-Burst-Remaining",
            HeaderValue::from_str(&user_status.burst_tokens.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        // Add concurrent requests
        headers.insert(
            "X-RateLimit-Concurrent",
            HeaderValue::from_str(&user_status.current_concurrent.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
        
        headers.insert(
            "X-RateLimit-Concurrent-Limit",
            HeaderValue::from_str(&user_status.config.concurrent_requests.to_string())
                .unwrap_or(HeaderValue::from_static("0"))
        );
    }
}

/// Estimate response size for tracking
fn estimate_response_size(response: &Response) -> u64 {
    // Get content-length header if available
    if let Some(content_length) = response.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<u64>() {
                return length;
            }
        }
    }
    
    // Estimate based on status code and typical response sizes
    match response.status().as_u16() {
        200..=299 => 1024, // Assume 1KB for successful responses
        400..=499 => 256,  // Assume 256B for client errors
        500..=599 => 512,  // Assume 512B for server errors
        _ => 128,          // Assume 128B for other responses
    }
}

/// Helper function to create rate limit middleware state
pub fn create_rate_limit_state(
    rate_limiter: Arc<RateLimiter>,
    usage_tracker: Arc<UsageTracker>,
) -> RateLimitMiddlewareState {
    RateLimitMiddlewareState::new(rate_limiter, usage_tracker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Method};
    use uuid::Uuid;

    #[test]
    fn test_extract_user_id_from_bearer_token() {
        let user_id = Uuid::new_v4();
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Authorization", format!("Bearer {}", user_id))
            .body(Body::empty())
            .unwrap();
        
        let extracted_id = extract_user_id(&request);
        assert_eq!(extracted_id, user_id);
    }

    #[test]
    fn test_extract_user_id_from_header() {
        let user_id = Uuid::new_v4();
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("X-User-ID", user_id.to_string())
            .body(Body::empty())
            .unwrap();
        
        let extracted_id = extract_user_id(&request);
        assert_eq!(extracted_id, user_id);
    }

    #[test]
    fn test_extract_user_tier() {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("X-User-Tier", "premium")
            .body(Body::empty())
            .unwrap();
        
        let tier = extract_user_tier(&request);
        assert_eq!(tier, Some(RateLimitTier::Premium));
    }

    #[test]
    fn test_extract_user_tier_from_bearer() {
        let user_id = Uuid::new_v4();
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Authorization", format!("Bearer {}:enterprise:secret", user_id))
            .body(Body::empty())
            .unwrap();
        
        let tier = extract_user_tier(&request);
        assert_eq!(tier, Some(RateLimitTier::Enterprise));
    }

    #[test]
    fn test_anonymous_user_from_ip() {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("X-Forwarded-For", "192.168.1.1, 10.0.0.1")
            .body(Body::empty())
            .unwrap();
        
        let user_id = extract_user_id(&request);
        // Should create deterministic UUID from IP
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        "192.168.1.1".hash(&mut hasher);
        let hash = hasher.finish();
        let expected = Uuid::from_u128(hash as u128);
        assert_eq!(user_id, expected);
    }
}
