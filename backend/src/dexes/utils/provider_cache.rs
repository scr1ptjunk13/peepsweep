use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Client, Http};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use crate::dexes::DexError;

/// Provider cache with health monitoring and fallback logic
#[derive(Clone)]
pub struct ProviderCache {
    providers: Arc<RwLock<HashMap<String, CachedProvider>>>,
    rpc_endpoints: HashMap<String, Vec<String>>,
    health_tracker: Arc<RwLock<HashMap<String, ProviderHealth>>>,
}

#[derive(Clone)]
struct CachedProvider {
    provider: RootProvider<Http<Client>>,
    created_at: Instant,
    last_used: Instant,
    success_count: u32,
    failure_count: u32,
}

#[derive(Clone)]
struct ProviderHealth {
    success_rate: f32,
    avg_response_time: Duration,
    last_failure: Option<Instant>,
    consecutive_failures: u32,
}

impl ProviderCache {
    pub fn new() -> Self {
        let mut rpc_endpoints = HashMap::new();
        
        // Ethereum mainnet RPCs
        rpc_endpoints.insert("ethereum".to_string(), vec![
            "https://eth.drpc.org".to_string(),
            "https://rpc.ankr.com/eth".to_string(),
            "https://ethereum.publicnode.com".to_string(),
            "https://eth.llamarpc.com".to_string(),
        ]);
        
        // Optimism RPCs
        rpc_endpoints.insert("optimism".to_string(), vec![
            "https://optimism.drpc.org".to_string(),
            "https://rpc.ankr.com/optimism".to_string(),
            "https://optimism.publicnode.com".to_string(),
            "https://op-pokt.nodies.app".to_string(),
        ]);
        
        // Arbitrum RPCs
        rpc_endpoints.insert("arbitrum".to_string(), vec![
            "https://arbitrum.drpc.org".to_string(),
            "https://rpc.ankr.com/arbitrum".to_string(),
            "https://arbitrum.publicnode.com".to_string(),
            "https://arb-pokt.nodies.app".to_string(),
        ]);
        
        // Polygon RPCs
        rpc_endpoints.insert("polygon".to_string(), vec![
            "https://polygon.drpc.org".to_string(),
            "https://rpc.ankr.com/polygon".to_string(),
            "https://polygon.publicnode.com".to_string(),
            "https://poly-pokt.nodies.app".to_string(),
        ]);
        
        // Base RPCs
        rpc_endpoints.insert("base".to_string(), vec![
            "https://base.drpc.org".to_string(),
            "https://rpc.ankr.com/base".to_string(),
            "https://base.publicnode.com".to_string(),
            "https://base-pokt.nodies.app".to_string(),
        ]);
        
        // BSC (Binance Smart Chain) RPCs - PancakeSwap's home chain
        rpc_endpoints.insert("bsc".to_string(), vec![
            "https://bsc.drpc.org".to_string(),
            "https://rpc.ankr.com/bsc".to_string(),
            "https://bsc.publicnode.com".to_string(),
            "https://bsc-dataseed.binance.org".to_string(),
            "https://bsc.llamarpc.com".to_string(),
            "https://binance.nodereal.io".to_string(),
        ]);
        
        // zkSync Era RPCs
        rpc_endpoints.insert("zksync".to_string(), vec![
            "https://mainnet.era.zksync.io".to_string(),
            "https://zksync.drpc.org".to_string(),
            "https://zksync-era.rpc.thirdweb.com".to_string(),
        ]);
        
        // Avalanche RPCs
        rpc_endpoints.insert("avalanche".to_string(), vec![
            "https://avalanche.drpc.org".to_string(),
            "https://rpc.ankr.com/avalanche".to_string(),
            "https://avalanche.publicnode.com".to_string(),
            "https://avax-pokt.nodies.app".to_string(),
        ]);

        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            rpc_endpoints,
            health_tracker: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get provider for chain with automatic fallback and health monitoring
    pub async fn get_provider(&self, chain: &str) -> Result<RootProvider<Http<Client>>, DexError> {
        // Try to get cached provider first
        if let Some(provider) = self.get_cached_provider(chain) {
            return Ok(provider);
        }

        // Get RPC endpoints for chain
        let endpoints = self.rpc_endpoints.get(chain)
            .ok_or_else(|| DexError::UnsupportedChain(format!("No RPCs configured for chain: {}", chain)))?;

        // Try each endpoint with health-based ordering
        let ordered_endpoints = self.order_endpoints_by_health(chain, endpoints);
        
        for (i, rpc_url) in ordered_endpoints.iter().enumerate() {
            match self.create_provider(rpc_url).await {
                Ok(provider) => {
                    // Cache the successful provider
                    self.cache_provider(chain, provider.clone(), rpc_url);
                    self.update_health_success(chain, rpc_url);
                    return Ok(provider);
                }
                Err(e) => {
                    self.update_health_failure(chain, rpc_url);
                    
                    // If not the last endpoint, wait before trying next
                    if i < ordered_endpoints.len() - 1 {
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            }
        }

        Err(DexError::ConfigError(format!("All RPC endpoints failed for chain: {}", chain)))
    }

    /// Create a new provider from RPC URL
    async fn create_provider(&self, rpc_url: &str) -> Result<RootProvider<Http<Client>>, DexError> {
        let parsed_url = rpc_url.parse()
            .map_err(|e| DexError::ConfigError(format!("Invalid RPC URL {}: {}", rpc_url, e)))?;
        
        let provider = ProviderBuilder::new().on_http(parsed_url);
        
        // Test the provider with a simple call
        match tokio::time::timeout(Duration::from_secs(5), provider.get_block_number()).await {
            Ok(Ok(_)) => Ok(provider),
            Ok(Err(e)) => Err(DexError::ConfigError(format!("Provider test failed: {}", e))),
            Err(_) => Err(DexError::ConfigError("Provider timeout".to_string())),
        }
    }

    /// Get cached provider if available and not expired
    fn get_cached_provider(&self, chain: &str) -> Option<RootProvider<Http<Client>>> {
        {
            let providers = self.providers.read().ok()?;
            let cached = providers.get(chain)?;
            
            // Check if cache is still valid (5 minutes)
            if cached.created_at.elapsed() < Duration::from_secs(300) {
                let provider = cached.provider.clone();
                drop(providers);
                
                // Update last used time
                if let Ok(mut providers) = self.providers.write() {
                    if let Some(cached) = providers.get_mut(chain) {
                        cached.last_used = Instant::now();
                    }
                }
                return Some(provider);
            }
        }
        None
    }

    /// Cache a provider for reuse
    fn cache_provider(&self, chain: &str, provider: RootProvider<Http<Client>>, _rpc_url: &str) {
        if let Ok(mut providers) = self.providers.write() {
            let cached = CachedProvider {
                provider,
                created_at: Instant::now(),
                last_used: Instant::now(),
                success_count: 1,
                failure_count: 0,
            };
            providers.insert(chain.to_string(), cached);
        }
    }

    /// Order endpoints by health score (best first)
    fn order_endpoints_by_health(&self, chain: &str, endpoints: &[String]) -> Vec<String> {
        let health = self.health_tracker.read().ok();
        let mut scored_endpoints: Vec<(String, f32)> = endpoints.iter().map(|url| {
            let score = if let Some(ref health) = health {
                let key = format!("{}:{}", chain, url);
                health.get(&key).map(|h| self.calculate_health_score(h)).unwrap_or(1.0)
            } else {
                1.0 // Default score for new endpoints
            };
            (url.clone(), score)
        }).collect();

        // Sort by score (highest first)
        scored_endpoints.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        scored_endpoints.into_iter().map(|(url, _)| url).collect()
    }

    /// Calculate health score from 0.0 (worst) to 1.0 (best)
    fn calculate_health_score(&self, health: &ProviderHealth) -> f32 {
        let mut score = health.success_rate;
        
        // Penalize recent failures
        if let Some(last_failure) = health.last_failure {
            if last_failure.elapsed() < Duration::from_secs(60) {
                score *= 0.5; // 50% penalty for recent failure
            }
        }
        
        // Penalize consecutive failures
        if health.consecutive_failures > 0 {
            score *= 0.9_f32.powi(health.consecutive_failures as i32);
        }
        
        // Bonus for fast response times
        if health.avg_response_time < Duration::from_millis(500) {
            score *= 1.1;
        }
        
        score.clamp(0.0, 1.0)
    }

    /// Update health tracking for successful call
    fn update_health_success(&self, chain: &str, rpc_url: &str) {
        if let Ok(mut health) = self.health_tracker.write() {
            let key = format!("{}:{}", chain, rpc_url);
            let entry = health.entry(key).or_insert(ProviderHealth {
                success_rate: 1.0,
                avg_response_time: Duration::from_millis(500),
                last_failure: None,
                consecutive_failures: 0,
            });
            
            // Update success rate (exponential moving average)
            entry.success_rate = entry.success_rate * 0.9 + 0.1;
            entry.consecutive_failures = 0;
        }
    }

    /// Update health tracking for failed call
    fn update_health_failure(&self, chain: &str, rpc_url: &str) {
        if let Ok(mut health) = self.health_tracker.write() {
            let key = format!("{}:{}", chain, rpc_url);
            let entry = health.entry(key).or_insert(ProviderHealth {
                success_rate: 1.0,
                avg_response_time: Duration::from_millis(500),
                last_failure: None,
                consecutive_failures: 0,
            });
            
            // Update failure tracking
            entry.success_rate = entry.success_rate * 0.9; // Decay success rate
            entry.last_failure = Some(Instant::now());
            entry.consecutive_failures += 1;
        }
    }

    /// Get health statistics for monitoring
    pub fn get_health_stats(&self, chain: &str) -> Vec<(String, f32, u32)> {
        let health = self.health_tracker.read().ok();
        let endpoints = self.rpc_endpoints.get(chain);
        
        if let (Some(health), Some(endpoints)) = (health, endpoints) {
            endpoints.iter().map(|url| {
                let key = format!("{}:{}", chain, url);
                let (success_rate, failures) = health.get(&key)
                    .map(|h| (h.success_rate, h.consecutive_failures))
                    .unwrap_or((1.0, 0));
                (url.clone(), success_rate, failures)
            }).collect()
        } else {
            vec![]
        }
    }

    /// Clear cache for a specific chain (useful for testing)
    pub fn clear_cache(&self, chain: &str) {
        if let Ok(mut providers) = self.providers.write() {
            providers.remove(chain);
        }
    }

    /// Clear all caches
    pub fn clear_all_caches(&self) {
        if let Ok(mut providers) = self.providers.write() {
            providers.clear();
        }
        if let Ok(mut health) = self.health_tracker.write() {
            health.clear();
        }
    }
}

impl Default for ProviderCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_cache_creation() {
        let cache = ProviderCache::new();
        
        // Should have configured chains
        assert!(cache.rpc_endpoints.contains_key("ethereum"));
        assert!(cache.rpc_endpoints.contains_key("optimism"));
        assert!(cache.rpc_endpoints.contains_key("arbitrum"));
    }

    #[tokio::test]
    async fn test_unsupported_chain() {
        let cache = ProviderCache::new();
        let result = cache.get_provider("unsupported").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_health_score_calculation() {
        let cache = ProviderCache::new();
        
        // Perfect health
        let perfect_health = ProviderHealth {
            success_rate: 1.0,
            avg_response_time: Duration::from_millis(200),
            last_failure: None,
            consecutive_failures: 0,
        };
        assert!(cache.calculate_health_score(&perfect_health) > 1.0);
        
        // Poor health
        let poor_health = ProviderHealth {
            success_rate: 0.5,
            avg_response_time: Duration::from_millis(2000),
            last_failure: Some(Instant::now()),
            consecutive_failures: 3,
        };
        assert!(cache.calculate_health_score(&poor_health) < 0.5);
    }
}
