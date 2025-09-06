use std::sync::Arc;
use tracing::{error, info, warn};
use tokio::time::{sleep, Duration};

use crate::crosschain::unified_token_interface::UnifiedTokenInterface;
use super::{
    TokenDiscoveryService, TokenDiscoveryScheduler, TokenRegistryConfig,
    discovery_engine::TokenDiscoveryService as DiscoveryEngine,
};

/// Integration service that connects token discovery with the existing unified interface
pub struct TokenRegistryIntegrationService {
    discovery_service: Arc<TokenDiscoveryService>,
    scheduler: Arc<TokenDiscoveryScheduler>,
    unified_interface: Arc<UnifiedTokenInterface>,
    config: TokenRegistryConfig,
}

impl TokenRegistryIntegrationService {
    pub fn new(
        unified_interface: Arc<UnifiedTokenInterface>,
        config: TokenRegistryConfig,
    ) -> Self {
        let discovery_service = Arc::new(TokenDiscoveryService::new(
            config.clone(),
            Arc::clone(&unified_interface),
        ));
        
        let scheduler = Arc::new(TokenDiscoveryScheduler::new(
            Arc::clone(&discovery_service),
            config.clone(),
        ));

        Self {
            discovery_service,
            scheduler,
            unified_interface,
            config,
        }
    }

    /// Initialize the token registry system
    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing Token Registry Integration Service");

        // Start with initial token discovery if enabled
        if self.should_run_initial_discovery().await {
            info!("Running initial token discovery...");
            let result = self.discovery_service.discover_all_tokens().await;
            info!("Initial token discovery completed: {} tokens discovered", result.tokens_discovered);
        }

        // Start the scheduler
        self.scheduler.start().await;
        info!("Token discovery scheduler started");

        Ok(())
    }

    /// Check if initial discovery should run
    async fn should_run_initial_discovery(&self) -> bool {
        // Check if we already have a reasonable number of tokens
        let current_tokens = self.unified_interface.get_all_tokens().await;
        
        // If we have fewer than 50 tokens total, run initial discovery
        if current_tokens.len() < 50 {
            return true;
        }

        // Check if we have tokens for major chains
        let major_chains = vec![1, 137, 42161, 10, 8453]; // ETH, Polygon, Arbitrum, Optimism, Base
        let mut chains_with_tokens = 0;

        for &chain_id in &major_chains {
            let chain_tokens = self.unified_interface.get_tokens_for_chain(chain_id).await;
            if chain_tokens.len() > 5 { // At least 5 tokens per major chain
                chains_with_tokens += 1;
            }
        }

        // Run initial discovery if we don't have tokens for most major chains
        chains_with_tokens < 3
    }

    /// Get the discovery service for API integration
    pub fn get_discovery_service(&self) -> Arc<TokenDiscoveryService> {
        Arc::clone(&self.discovery_service)
    }

    /// Get the scheduler for API integration
    pub fn get_scheduler(&self) -> Arc<TokenDiscoveryScheduler> {
        Arc::clone(&self.scheduler)
    }

    /// Get the unified interface
    pub fn get_unified_interface(&self) -> Arc<UnifiedTokenInterface> {
        Arc::clone(&self.unified_interface)
    }

    /// Shutdown the service gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down Token Registry Integration Service");
        self.scheduler.stop().await;
        
        // Wait a bit for any ongoing operations to complete
        sleep(Duration::from_secs(2)).await;
        info!("Token Registry Integration Service shutdown complete");
    }

    /// Health check for the service
    pub async fn health_check(&self) -> TokenRegistryHealthStatus {
        let stats = self.discovery_service.get_stats().await;
        let scheduler_running = self.scheduler.is_running().await;
        let total_unified_tokens = self.unified_interface.get_all_tokens().await.len();

        TokenRegistryHealthStatus {
            discovery_service_active: true,
            scheduler_running,
            total_discovered_tokens: stats.total_tokens,
            total_unified_tokens,
            last_discovery_run: stats.last_discovery_run,
            next_scheduled_run: stats.next_scheduled_run,
            sources_active: stats.sources_active,
            healthy: scheduler_running && stats.total_tokens > 0,
        }
    }

    /// Force sync discovered tokens with unified interface
    pub async fn force_sync(&self) -> Result<SyncResult, Box<dyn std::error::Error>> {
        info!("Force syncing discovered tokens with unified interface");
        
        let all_discovered = self.discovery_service.get_all_discovered_tokens().await;
        let mut synced_count = 0;
        let mut error_count = 0;

        for (chain_id, chain_tokens) in all_discovered {
            for discovered_token in chain_tokens.tokens {
                match self.sync_single_token(discovered_token.clone(), chain_id).await {
                    Ok(_) => synced_count += 1,
                    Err(e) => {
                        error!("Failed to sync token {}: {}", discovered_token.symbol, e);
                        error_count += 1;
                    }
                }
            }
        }

        Ok(SyncResult {
            synced_count,
            error_count,
            success: error_count == 0,
        })
    }

    /// Sync a single discovered token with the unified interface
    async fn sync_single_token(
        &self,
        discovered_token: super::DiscoveredToken,
        chain_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::crosschain::unified_token_interface::{UnifiedToken, TokenType};
        use std::collections::HashMap;

        // Check if token already exists
        if let Some(mut existing_token) = self.unified_interface.get_token(&discovered_token.symbol).await {
            // Update existing token with new chain address if not present
            if !existing_token.chain_addresses.contains_key(&chain_id) {
                existing_token.chain_addresses.insert(chain_id, discovered_token.address);
                self.unified_interface.add_token(existing_token).await;
            }
        } else {
            // Create new unified token
            let mut chain_addresses = HashMap::new();
            let is_native = discovered_token.address == "0x0000000000000000000000000000000000000000";
            chain_addresses.insert(chain_id, discovered_token.address.clone());

            let token_type = match discovered_token.symbol.to_uppercase().as_str() {
                s if s == "ETH" || s == "MATIC" || s == "AVAX" || s == "BNB" => TokenType::Native,
                s if s.starts_with('W') && (s == "WETH" || s == "WMATIC" || s == "WAVAX") => TokenType::Wrapped,
                s if s == "USDC" || s == "USDT" || s == "DAI" || s == "BUSD" => TokenType::Stable,
                s if s.contains("SYNTH") => TokenType::Synthetic,
                _ => TokenType::ERC20,
            };

            let unified_token = UnifiedToken {
                symbol: discovered_token.symbol.clone(),
                name: discovered_token.name.clone(),
                decimals: discovered_token.decimals,
                chain_addresses,
                coingecko_id: discovered_token.coingecko_id.clone(),
                token_type,
                is_native,
                logo_uri: discovered_token.logo_uri.clone(),
            };

            self.unified_interface.add_token(unified_token).await;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TokenRegistryHealthStatus {
    pub discovery_service_active: bool,
    pub scheduler_running: bool,
    pub total_discovered_tokens: usize,
    pub total_unified_tokens: usize,
    pub last_discovery_run: u64,
    pub next_scheduled_run: u64,
    pub sources_active: usize,
    pub healthy: bool,
}

#[derive(Debug, Clone)]
pub struct SyncResult {
    pub synced_count: usize,
    pub error_count: usize,
    pub success: bool,
}
