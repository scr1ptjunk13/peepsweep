use redis::{Client, Commands, RedisResult};
use serde::{Serialize, Deserialize};
use std::time::Duration;
use anyhow::Result;
use tracing::{info, error, warn};
use uuid::Uuid;
use std::collections::HashMap;

use super::models::UnifiedToken;

pub struct TokenCache {
    client: Client,
    default_ttl: Duration,
}

impl TokenCache {
    pub fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)?;
        
        Ok(Self {
            client,
            default_ttl: Duration::from_secs(300), // 5 minutes default TTL
        })
    }

    pub async fn get_connection(&self) -> Result<redis::Connection> {
        Ok(self.client.get_connection()?)
    }

    // Token caching operations
    pub async fn cache_token(&self, token: &UnifiedToken) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let key = format!("token:symbol:{}", token.symbol.to_uppercase());
        let value = serde_json::to_string(token)?;
        
        let _: () = conn.set_ex(&key, &value, self.default_ttl.as_secs() as usize)?;
        
        // Also cache by ID
        let id_key = format!("token:id:{}", token.id);
        let _: () = conn.set_ex(&id_key, &value, self.default_ttl.as_secs() as usize)?;
        
        // Cache chain addresses for fast lookup
        for (chain_id, address) in &token.chain_addresses {
            let addr_key = format!("token:address:{}:{}", chain_id, address.to_lowercase());
            let token_ref = TokenReference {
                id: token.id,
                symbol: token.symbol.clone(),
            };
            let ref_value = serde_json::to_string(&token_ref)?;
            let _: () = conn.set_ex(&addr_key, &ref_value, self.default_ttl.as_secs() as usize)?;
        }
        
        info!("Cached token: {} with {} chain addresses", token.symbol, token.chain_addresses.len());
        Ok(())
    }

    pub async fn get_token_by_symbol(&self, symbol: &str) -> Result<Option<UnifiedToken>> {
        let mut conn = self.get_connection().await?;
        let key = format!("token:symbol:{}", symbol.to_uppercase());
        
        let cached: RedisResult<String> = conn.get(&key);
        match cached {
            Ok(value) => {
                match serde_json::from_str::<UnifiedToken>(&value) {
                    Ok(token) => {
                        info!("Cache hit for token: {}", symbol);
                        Ok(Some(token))
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached token {}: {}", symbol, e);
                        Ok(None)
                    }
                }
            }
            Err(_) => {
                info!("Cache miss for token: {}", symbol);
                Ok(None)
            }
        }
    }

    pub async fn get_token_by_address(&self, chain_id: i64, address: &str) -> Result<Option<UnifiedToken>> {
        let mut conn = self.get_connection().await?;
        let addr_key = format!("token:address:{}:{}", chain_id, address.to_lowercase());
        
        // First get the token reference
        let cached_ref: RedisResult<String> = conn.get(&addr_key);
        if let Ok(ref_value) = cached_ref {
            if let Ok(token_ref) = serde_json::from_str::<TokenReference>(&ref_value) {
                // Now get the full token data
                return self.get_token_by_symbol(&token_ref.symbol).await;
            }
        }
        
        Ok(None)
    }

    // Batch caching operations
    pub async fn cache_tokens_batch(&self, tokens: &[UnifiedToken]) -> Result<()> {
        for token in tokens {
            if let Err(e) = self.cache_token(token).await {
                error!("Failed to cache token {}: {}", token.symbol, e);
            }
        }
        
        info!("Batch cached {} tokens", tokens.len());
        Ok(())
    }

    // Token list caching
    pub async fn cache_token_list(&self, list_key: &str, tokens: &[UnifiedToken], ttl: Option<Duration>) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let ttl = ttl.unwrap_or(self.default_ttl);
        
        let token_list = TokenList {
            tokens: tokens.to_vec(),
            cached_at: chrono::Utc::now(),
            count: tokens.len(),
        };
        
        let value = serde_json::to_string(&token_list)?;
        let _: () = conn.set_ex(list_key, &value, ttl.as_secs() as usize)?;
        
        info!("Cached token list '{}' with {} tokens", list_key, tokens.len());
        Ok(())
    }

    pub async fn get_token_list(&self, list_key: &str) -> Result<Option<Vec<UnifiedToken>>> {
        let mut conn = self.get_connection().await?;
        
        let cached: RedisResult<String> = conn.get(list_key);
        match cached {
            Ok(value) => {
                match serde_json::from_str::<TokenList>(&value) {
                    Ok(token_list) => {
                        info!("Cache hit for token list '{}' with {} tokens", list_key, token_list.count);
                        Ok(Some(token_list.tokens))
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached token list '{}': {}", list_key, e);
                        Ok(None)
                    }
                }
            }
            Err(_) => {
                info!("Cache miss for token list: {}", list_key);
                Ok(None)
            }
        }
    }

    // Chain-specific caching
    pub async fn cache_chain_tokens(&self, chain_id: i64, tokens: &[UnifiedToken]) -> Result<()> {
        let list_key = format!("tokens:chain:{}", chain_id);
        self.cache_token_list(&list_key, tokens, Some(Duration::from_secs(900))).await // 15 minutes
    }

    pub async fn get_chain_tokens(&self, chain_id: i64) -> Result<Option<Vec<UnifiedToken>>> {
        let list_key = format!("tokens:chain:{}", chain_id);
        self.get_token_list(&list_key).await
    }

    // Search result caching
    pub async fn cache_search_results(&self, query: &str, tokens: &[UnifiedToken]) -> Result<()> {
        let search_key = format!("search:{}", query.to_lowercase());
        self.cache_token_list(&search_key, tokens, Some(Duration::from_secs(600))).await // 10 minutes
    }

    pub async fn get_search_results(&self, query: &str) -> Result<Option<Vec<UnifiedToken>>> {
        let search_key = format!("search:{}", query.to_lowercase());
        self.get_token_list(&search_key).await
    }

    // Market data caching
    pub async fn cache_trending_tokens(&self, tokens: &[UnifiedToken]) -> Result<()> {
        self.cache_token_list("tokens:trending", tokens, Some(Duration::from_secs(300))).await // 5 minutes
    }

    pub async fn get_trending_tokens(&self) -> Result<Option<Vec<UnifiedToken>>> {
        self.get_token_list("tokens:trending").await
    }

    // Cache invalidation
    pub async fn invalidate_token(&self, symbol: &str) -> Result<()> {
        let mut conn = self.get_connection().await?;
        
        let keys_to_delete = vec![
            format!("token:symbol:{}", symbol.to_uppercase()),
            format!("tokens:*"), // Invalidate all token lists
            format!("search:*"), // Invalidate search results
        ];
        
        for key_pattern in keys_to_delete {
            if key_pattern.contains('*') {
                // Use SCAN for pattern-based deletion
                let keys: Vec<String> = conn.keys(&key_pattern)?;
                if !keys.is_empty() {
                    let _: () = conn.del(&keys)?;
                }
            } else {
                let _: () = conn.del(&key_pattern)?;
            }
        }
        
        info!("Invalidated cache for token: {}", symbol);
        Ok(())
    }

    pub async fn invalidate_chain_cache(&self, chain_id: i64) -> Result<()> {
        let mut conn = self.get_connection().await?;
        let pattern = format!("tokens:chain:{}", chain_id);
        let _: () = conn.del(&pattern)?;
        
        info!("Invalidated cache for chain: {}", chain_id);
        Ok(())
    }

    pub async fn clear_all_cache(&self) -> Result<()> {
        let mut conn = self.get_connection().await?;
        redis::cmd("FLUSHDB").execute(&mut conn);
        
        info!("Cleared all token cache");
        Ok(())
    }

    // Cache statistics
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        let mut conn = self.get_connection().await?;
        
        let token_keys: Vec<String> = conn.keys("token:*")?;
        let list_keys: Vec<String> = conn.keys("tokens:*")?;
        let search_keys: Vec<String> = conn.keys("search:*")?;
        
        let info: String = redis::cmd("INFO").arg("memory").query(&mut conn)?;
        let memory_usage = Self::parse_memory_usage(&info);
        
        Ok(CacheStats {
            cached_tokens: token_keys.len(),
            cached_lists: list_keys.len(),
            cached_searches: search_keys.len(),
            memory_usage_bytes: memory_usage,
        })
    }

    fn parse_memory_usage(info: &str) -> u64 {
        for line in info.lines() {
            if line.starts_with("used_memory:") {
                if let Some(value) = line.split(':').nth(1) {
                    return value.parse().unwrap_or(0);
                }
            }
        }
        0
    }

    // Health check
    pub async fn health_check(&self) -> Result<bool> {
        let mut conn = self.get_connection().await?;
        let pong: String = redis::cmd("PING").query(&mut conn)?;
        Ok(pong == "PONG")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenReference {
    id: Uuid,
    symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenList {
    tokens: Vec<UnifiedToken>,
    cached_at: chrono::DateTime<chrono::Utc>,
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub cached_tokens: usize,
    pub cached_lists: usize,
    pub cached_searches: usize,
    pub memory_usage_bytes: u64,
}

// Cache warming service
pub struct CacheWarmer {
    cache: TokenCache,
}

impl CacheWarmer {
    pub fn new(cache: TokenCache) -> Self {
        Self { cache }
    }

    pub async fn warm_essential_tokens(&self, essential_symbols: &[&str]) -> Result<()> {
        info!("Warming cache for {} essential tokens", essential_symbols.len());
        
        // This would typically fetch from database and cache
        // For now, we'll just ensure the cache is ready for these symbols
        for symbol in essential_symbols {
            if let None = self.cache.get_token_by_symbol(symbol).await? {
                info!("Essential token {} not in cache - will be loaded on first request", symbol);
            }
        }
        
        Ok(())
    }

    pub async fn warm_popular_searches(&self, popular_queries: &[&str]) -> Result<()> {
        info!("Warming cache for {} popular search queries", popular_queries.len());
        
        for query in popular_queries {
            if let None = self.cache.get_search_results(query).await? {
                info!("Popular search '{}' not in cache - will be loaded on first request", query);
            }
        }
        
        Ok(())
    }
}
