use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{info, error, warn};
use anyhow::Result;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::{TokenRepository, TokenCache};
use super::models::*;
use crate::token_registry::discovery_engine::TokenDiscoveryService;
use crate::token_registry::DiscoveredToken;

#[derive(Clone)]
pub struct BackgroundJobManager {
    repository: Arc<TokenRepository>,
    cache: Arc<TokenCache>,
    discovery_service: Arc<TokenDiscoveryService>,
    is_running: Arc<tokio::sync::RwLock<bool>>,
}

impl BackgroundJobManager {
    pub fn new(
        repository: Arc<TokenRepository>,
        cache: Arc<TokenCache>,
        discovery_service: Arc<TokenDiscoveryService>,
    ) -> Self {
        Self {
            repository,
            cache,
            discovery_service,
            is_running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                warn!("Background job manager is already running");
                return Ok(());
            }
            *running = true;
        }

        info!("Starting background job manager");

        // Start all background jobs concurrently
        tokio::spawn({
            let self_clone = self.clone();
            async move {
                if let Err(e) = self_clone.start_token_discovery_job().await {
                    error!("Token discovery job failed: {}", e);
                }
            }
        });

        tokio::spawn({
            let self_clone = self.clone();
            async move {
                if let Err(e) = self_clone.start_cache_refresh_job().await {
                    error!("Cache refresh job failed: {}", e);
                }
            }
        });

        tokio::spawn({
            let self_clone = self.clone();
            async move {
                if let Err(e) = self_clone.start_market_data_update_job().await {
                    error!("Market data update job failed: {}", e);
                }
            }
        });

        tokio::spawn({
            let self_clone = self.clone();
            async move {
                if let Err(e) = self_clone.start_cleanup_job().await {
                    error!("Cleanup job failed: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn stop(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
        info!("Stopped background job manager");
    }

    // Job 1: Token Discovery - Runs every 5 minutes
    async fn start_token_discovery_job(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(300)); // 5 minutes
        
        loop {
            interval.tick().await;
            
            if !*self.is_running.read().await {
                break;
            }

            info!("Starting token discovery job");
            let job_id = self.repository.create_discovery_job(
                "token_discovery", 
                None, 
                None
            ).await?;

            match self.run_token_discovery().await {
                Ok((processed, added, updated)) => {
                    self.repository.complete_discovery_job(
                        job_id, 
                        processed, 
                        added, 
                        updated
                    ).await?;
                    info!("Token discovery completed: {} processed, {} added, {} updated", 
                          processed, added, updated);
                }
                Err(e) => {
                    error!("Token discovery failed: {}", e);
                    self.repository.fail_discovery_job(job_id, &e.to_string()).await?;
                }
            }
        }

        Ok(())
    }

    // Job 2: Cache Refresh - Runs every 2 minutes
    async fn start_cache_refresh_job(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(120)); // 2 minutes
        
        loop {
            interval.tick().await;
            
            if !*self.is_running.read().await {
                break;
            }

            info!("Starting cache refresh job");
            if let Err(e) = self.refresh_cache().await {
                error!("Cache refresh failed: {}", e);
            }
        }

        Ok(())
    }

    // Job 3: Market Data Update - Runs every 1 minute
    async fn start_market_data_update_job(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(60)); // 1 minute
        
        loop {
            interval.tick().await;
            
            if !*self.is_running.read().await {
                break;
            }

            info!("Starting market data update job");
            if let Err(e) = self.update_market_data().await {
                error!("Market data update failed: {}", e);
            }
        }

        Ok(())
    }

    // Job 4: Cleanup - Runs every hour
    async fn start_cleanup_job(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(3600)); // 1 hour
        
        loop {
            interval.tick().await;
            
            if !*self.is_running.read().await {
                break;
            }

            info!("Starting cleanup job");
            if let Err(e) = self.cleanup_old_data().await {
                error!("Cleanup job failed: {}", e);
            }
        }

        Ok(())
    }

    // Token Discovery Implementation
    async fn run_token_discovery(&self) -> Result<(i32, i32, i32)> {
        let discovery_result = self.discovery_service.discover_all_tokens().await;
        
        let mut total_processed = 0;
        let mut total_added = 0;
        let mut total_updated = 0;

        // Get all discovered tokens and save to database
        let discovered_tokens = self.discovery_service.get_all_discovered_tokens().await;
        
        for (chain_id, chain_token_list) in discovered_tokens {
            let mut batch_data = Vec::new();
            
            for discovered_token in chain_token_list.tokens {
                total_processed += 1;
                
                // Convert discovered token to database format
                let new_token = NewToken {
                    symbol: discovered_token.symbol.clone(),
                    name: discovered_token.name.clone(),
                    coingecko_id: discovered_token.coingecko_id.clone(),
                    token_type: match discovered_token.symbol.as_str() {
                        "ETH" | "BNB" | "MATIC" | "AVAX" | "FTM" => TokenType::Native,
                        s if s.starts_with("W") => TokenType::Wrapped,
                        "USDC" | "USDT" | "DAI" | "BUSD" => TokenType::Stable,
                        _ => TokenType::ERC20,
                    },
                    decimals: discovered_token.decimals as i32,
                    total_supply: None,
                    is_verified: discovered_token.verified,
                    verification_level: if discovered_token.verified {
                        VerificationLevel::Community
                    } else {
                        VerificationLevel::Unverified
                    },
                    description: None,
                    website_url: None,
                    twitter_handle: None,
                    telegram_url: None,
                    discord_url: None,
                };

                let token_address = NewTokenAddress {
                    token_id: Uuid::new_v4(), // Will be updated in batch operation
                    chain_id: chain_id as i64,
                    address: discovered_token.address.clone(),
                    is_native: discovered_token.symbol == "ETH" || 
                              discovered_token.symbol == "BNB" ||
                              discovered_token.symbol == "MATIC" ||
                              discovered_token.symbol == "AVAX" ||
                              discovered_token.symbol == "FTM",
                    is_wrapped: discovered_token.symbol.starts_with("W"),
                    proxy_address: None,
                    implementation_address: None,
                };

                batch_data.push((new_token, vec![token_address]));
            }

            // Batch insert tokens for this chain
            if !batch_data.is_empty() {
                match self.repository.upsert_tokens_batch(batch_data).await {
                    Ok(token_ids) => {
                        total_added += token_ids.len() as i32;
                        info!("Added {} tokens for chain {}", token_ids.len(), chain_id);
                    }
                    Err(e) => {
                        error!("Failed to batch insert tokens for chain {}: {}", chain_id, e);
                    }
                }
            }
        }

        Ok((total_processed, total_added, total_updated))
    }

    // Cache Refresh Implementation
    async fn refresh_cache(&self) -> Result<()> {
        // Refresh essential tokens cache
        let essential_symbols = vec!["ETH", "BTC", "USDC", "USDT", "DAI", "WETH", "WBTC"];
        
        for symbol in essential_symbols {
            if let Ok(Some(token)) = self.repository.get_token_by_symbol(symbol).await {
                self.cache.cache_token(&token).await?;
            }
        }

        // Refresh trending tokens
        let trending_tokens = self.repository.get_unified_tokens(Some(50), Some(0)).await?;
        self.cache.cache_trending_tokens(&trending_tokens).await?;

        // Refresh chain-specific caches for major chains
        let major_chains = vec![1, 56, 137, 42161, 10, 8453];
        
        for chain_id in major_chains {
            let chain_tokens = self.repository.get_tokens_by_chain(chain_id).await?;
            self.cache.cache_chain_tokens(chain_id, &chain_tokens).await?;
        }

        info!("Cache refresh completed");
        Ok(())
    }

    // Market Data Update Implementation
    async fn update_market_data(&self) -> Result<()> {
        // This is a placeholder - in production, you'd integrate with CoinGecko API
        // or other price data providers to update market data
        
        info!("Market data update completed (placeholder)");
        Ok(())
    }

    // Cleanup Implementation
    async fn cleanup_old_data(&self) -> Result<()> {
        // Clean up old discovery jobs (keep last 100)
        // Clean up expired cache entries
        // Clean up old market data snapshots
        
        info!("Cleanup completed");
        Ok(())
    }

    // Manual job triggers
    pub async fn trigger_token_discovery(&self) -> Result<()> {
        info!("Manually triggering token discovery");
        let job_id = self.repository.create_discovery_job(
            "manual_token_discovery", 
            None, 
            None
        ).await?;

        match self.run_token_discovery().await {
            Ok((processed, added, updated)) => {
                self.repository.complete_discovery_job(job_id, processed, added, updated).await?;
                info!("Manual token discovery completed: {} processed, {} added, {} updated", 
                      processed, added, updated);
            }
            Err(e) => {
                error!("Manual token discovery failed: {}", e);
                self.repository.fail_discovery_job(job_id, &e.to_string()).await?;
            }
        }

        Ok(())
    }

    pub async fn trigger_cache_refresh(&self) -> Result<()> {
        info!("Manually triggering cache refresh");
        self.refresh_cache().await
    }

    pub async fn get_job_status(&self) -> Result<JobStatus> {
        let is_running = *self.is_running.read().await;
        let stats = self.cache.get_cache_stats().await?;
        let token_count = self.repository.get_token_count().await?;
        let chain_counts = self.repository.get_chain_token_counts().await?;

        Ok(JobStatus {
            is_running,
            cache_stats: stats,
            total_tokens: token_count,
            tokens_per_chain: chain_counts,
            last_discovery_run: None, // Would fetch from discovery_jobs table
            next_discovery_run: None, // Would calculate based on schedule
        })
    }
}

#[derive(Debug, Clone)]
pub struct JobStatus {
    pub is_running: bool,
    pub cache_stats: super::cache::CacheStats,
    pub total_tokens: i64,
    pub tokens_per_chain: std::collections::HashMap<i64, i64>,
    pub last_discovery_run: Option<DateTime<Utc>>,
    pub next_discovery_run: Option<DateTime<Utc>>,
}

// Job scheduler for more complex scheduling needs
pub struct JobScheduler {
    jobs: Vec<ScheduledJob>,
}

#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub id: String,
    pub name: String,
    pub schedule: String, // Cron expression
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

impl JobScheduler {
    pub fn new() -> Self {
        Self {
            jobs: vec![
                ScheduledJob {
                    id: "token_discovery".to_string(),
                    name: "Token Discovery".to_string(),
                    schedule: "0 */5 * * * *".to_string(), // Every 5 minutes
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
                ScheduledJob {
                    id: "cache_refresh".to_string(),
                    name: "Cache Refresh".to_string(),
                    schedule: "0 */2 * * * *".to_string(), // Every 2 minutes
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
                ScheduledJob {
                    id: "market_data_update".to_string(),
                    name: "Market Data Update".to_string(),
                    schedule: "0 * * * * *".to_string(), // Every minute
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
                ScheduledJob {
                    id: "cleanup".to_string(),
                    name: "Data Cleanup".to_string(),
                    schedule: "0 0 * * * *".to_string(), // Every hour
                    enabled: true,
                    last_run: None,
                    next_run: None,
                },
            ],
        }
    }

    pub fn get_jobs(&self) -> &[ScheduledJob] {
        &self.jobs
    }

    pub fn enable_job(&mut self, job_id: &str) -> Result<()> {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            job.enabled = true;
            info!("Enabled job: {}", job_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job not found: {}", job_id))
        }
    }

    pub fn disable_job(&mut self, job_id: &str) -> Result<()> {
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job_id) {
            job.enabled = false;
            info!("Disabled job: {}", job_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job not found: {}", job_id))
        }
    }
}
