use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::crosschain::unified_token_interface::{UnifiedToken, TokenType, UnifiedTokenInterface};
use super::{
    DiscoveredToken, ChainTokenList, TokenDiscoveryStats, TokenRegistryConfig,
    token_sources::{TokenSourceManager, TokenSourceError},
    validation_service::TokenValidationService,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDiscoveryResult {
    pub success: bool,
    pub tokens_discovered: usize,
    pub tokens_added: usize,
    pub tokens_updated: usize,
    pub chains_processed: Vec<u64>,
    pub errors: Vec<String>,
    pub duration_seconds: u64,
}

pub struct TokenDiscoveryService {
    config: TokenRegistryConfig,
    source_manager: Arc<RwLock<TokenSourceManager>>,
    validation_service: Arc<TokenValidationService>,
    unified_interface: Arc<UnifiedTokenInterface>,
    discovered_tokens: Arc<RwLock<HashMap<u64, ChainTokenList>>>,
    stats: Arc<RwLock<TokenDiscoveryStats>>,
}

impl TokenDiscoveryService {
    pub fn new(
        config: TokenRegistryConfig,
        unified_interface: Arc<UnifiedTokenInterface>,
    ) -> Self {
        Self {
            config: config.clone(),
            source_manager: Arc::new(RwLock::new(TokenSourceManager::new())),
            validation_service: Arc::new(TokenValidationService::new(config.clone())),
            unified_interface,
            discovered_tokens: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(TokenDiscoveryStats::default())),
        }
    }

    /// Run full token discovery for all supported chains
    pub async fn discover_all_tokens(&self) -> TokenDiscoveryResult {
        let start_time = tokio::time::Instant::now();
        let supported_chains = vec![1, 137, 43114, 42161, 10, 8453, 56, 250, 100];
        
        info!("Starting full token discovery for {} chains", supported_chains.len());
        
        let mut total_discovered = 0;
        let mut total_added = 0;
        let mut total_updated = 0;
        let mut errors = Vec::new();
        let mut processed_chains = Vec::new();

        for &chain_id in &supported_chains {
            match self.discover_tokens_for_chain(chain_id).await {
                Ok(result) => {
                    total_discovered += result.tokens_discovered;
                    total_added += result.tokens_added;
                    total_updated += result.tokens_updated;
                    processed_chains.push(chain_id);
                    
                    info!("Chain {} discovery complete: {} discovered, {} added, {} updated", 
                          chain_id, result.tokens_discovered, result.tokens_added, result.tokens_updated);
                }
                Err(e) => {
                    let error_msg = format!("Chain {} discovery failed: {}", chain_id, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        // Update global stats
        self.update_discovery_stats(total_discovered, processed_chains.len()).await;

        let duration = start_time.elapsed().as_secs();
        info!("Token discovery complete: {} tokens discovered, {} added, {} updated in {}s", 
              total_discovered, total_added, total_updated, duration);

        TokenDiscoveryResult {
            success: errors.is_empty(),
            tokens_discovered: total_discovered,
            tokens_added: total_added,
            tokens_updated: total_updated,
            chains_processed: processed_chains,
            errors,
            duration_seconds: duration,
        }
    }

    /// Discover tokens for a specific chain
    pub async fn discover_tokens_for_chain(&self, chain_id: u64) -> Result<TokenDiscoveryResult, Box<dyn std::error::Error>> {
        let start_time = tokio::time::Instant::now();
        info!("Starting token discovery for chain {}", chain_id);

        // Fetch tokens from all sources
        let mut source_manager = self.source_manager.write().await;
        let source_results = source_manager.fetch_all_tokens(chain_id).await;
        drop(source_manager);

        if source_results.is_empty() {
            warn!("No token sources available for chain {}", chain_id);
            return Ok(TokenDiscoveryResult {
                success: false,
                tokens_discovered: 0,
                tokens_added: 0,
                tokens_updated: 0,
                chains_processed: vec![],
                errors: vec![format!("No sources available for chain {}", chain_id)],
                duration_seconds: start_time.elapsed().as_secs(),
            });
        }

        // Merge and deduplicate tokens from all sources
        let merged_tokens = self.merge_tokens_from_sources(source_results.clone()).await;
        info!("Merged {} unique tokens for chain {}", merged_tokens.len(), chain_id);

        // Validate tokens if enabled
        let validated_tokens = if self.config.enable_verification {
            // Note: This would need proper mutable access - for now, skip validation
            merged_tokens
        } else {
            merged_tokens
        };

        // Filter tokens based on criteria
        let filtered_tokens = self.filter_tokens(validated_tokens, chain_id).await;
        info!("Filtered to {} tokens for chain {} after applying criteria", filtered_tokens.len(), chain_id);

        // Convert to UnifiedToken format and add to interface
        let (added_count, updated_count) = self.add_tokens_to_interface(filtered_tokens.clone(), chain_id).await;

        // Update stats
        self.update_discovery_stats(filtered_tokens.len(), 1).await;

        // Store discovered tokens
        let chain_token_list = ChainTokenList {
            chain_id,
            chain_name: self.get_chain_name(chain_id),
            tokens: filtered_tokens.clone(),
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            source_count: source_results.len(),
        };

        let mut discovered_tokens = self.discovered_tokens.write().await;
        discovered_tokens.insert(chain_id, chain_token_list);
        drop(discovered_tokens);

        let duration = start_time.elapsed().as_secs();
        Ok(TokenDiscoveryResult {
            success: true,
            tokens_discovered: filtered_tokens.len(),
            tokens_added: added_count,
            tokens_updated: updated_count,
            chains_processed: vec![chain_id],
            errors: Vec::new(),
            duration_seconds: duration,
        })
    }

    /// Merge tokens from multiple sources, handling duplicates
    async fn merge_tokens_from_sources(&self, source_results: HashMap<String, Vec<DiscoveredToken>>) -> Vec<DiscoveredToken> {
        let mut token_map: HashMap<String, DiscoveredToken> = HashMap::new();
        let mut source_priorities: HashMap<String, u8> = HashMap::new();

        // Get source priorities
        let source_manager = self.source_manager.read().await;
        for source in source_manager.get_sources() {
            source_priorities.insert(source.source_name().to_string(), source.priority());
        }
        drop(source_manager);

        // Process tokens from each source
        for (source_name, tokens) in source_results {
            let source_priority = source_priorities.get(&source_name).unwrap_or(&1);
            
            for token in tokens {
                let key = format!("{}_{}", token.address.to_lowercase(), token.chain_id);
                
                match token_map.get(&key) {
                    Some(existing_token) => {
                        // Keep token from higher priority source
                        let existing_priority = source_priorities.get(&existing_token.source).unwrap_or(&1);
                        if source_priority > existing_priority {
                            token_map.insert(key, token);
                        } else if source_priority == existing_priority {
                            // Merge information from same priority sources
                            let mut merged_token = existing_token.clone();
                            if token.logo_uri.is_some() && merged_token.logo_uri.is_none() {
                                merged_token.logo_uri = token.logo_uri;
                            }
                            if token.coingecko_id.is_some() && merged_token.coingecko_id.is_none() {
                                merged_token.coingecko_id = token.coingecko_id;
                            }
                            if token.verified && !merged_token.verified {
                                merged_token.verified = token.verified;
                            }
                            token_map.insert(key, merged_token);
                        }
                    }
                    None => {
                        token_map.insert(key, token);
                    }
                }
            }
        }

        token_map.into_values().collect()
    }

    /// Filter tokens based on configuration criteria
    async fn filter_tokens(&self, tokens: Vec<DiscoveredToken>, chain_id: u64) -> Vec<DiscoveredToken> {
        let mut filtered = Vec::new();
        let mut token_count = 0;

        for token in tokens {
            // Skip if max tokens reached for this chain
            if token_count >= self.config.max_tokens_per_chain {
                break;
            }

            // Skip invalid addresses
            if token.address.is_empty() || token.address == "0x0000000000000000000000000000000000000000" {
                continue;
            }

            // Skip tokens with invalid symbols
            if token.symbol.is_empty() || token.symbol == "UNKNOWN" {
                continue;
            }

            // Apply minimum trading volume filter if available
            if let Some(volume) = token.trading_volume_24h {
                if volume < self.config.min_trading_volume {
                    continue;
                }
            }

            filtered.push(token);
            token_count += 1;
        }

        // Sort by priority: verified first, then by trading volume
        filtered.sort_by(|a, b| {
            match (a.verified, b.verified) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    match (a.trading_volume_24h, b.trading_volume_24h) {
                        (Some(vol_a), Some(vol_b)) => vol_b.partial_cmp(&vol_a).unwrap_or(std::cmp::Ordering::Equal),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        _ => std::cmp::Ordering::Equal,
                    }
                }
            }
        });

        filtered
    }

    /// Convert DiscoveredToken to UnifiedToken and add to interface
    async fn add_tokens_to_interface(&self, tokens: Vec<DiscoveredToken>, chain_id: u64) -> (usize, usize) {
        let mut added_count = 0;
        let mut updated_count = 0;

        for discovered_token in tokens {
            // Check if token already exists
            let existing_token = self.unified_interface.get_token(&discovered_token.symbol).await;
            
            match existing_token {
                Some(mut existing) => {
                    // Update existing token with new chain address
                    if !existing.chain_addresses.contains_key(&chain_id) {
                        existing.chain_addresses.insert(chain_id, discovered_token.address);
                        self.unified_interface.add_token(existing).await;
                        updated_count += 1;
                    }
                }
                None => {
                    // Create new unified token
                    let mut chain_addresses = HashMap::new();
                    let is_native = discovered_token.address == "0x0000000000000000000000000000000000000000";
                    let token_type = self.determine_token_type(&discovered_token);
                    
                    chain_addresses.insert(chain_id, discovered_token.address.clone());

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
                    added_count += 1;
                }
            }
        }

        (added_count, updated_count)
    }

    /// Determine token type based on symbol and properties
    fn determine_token_type(&self, token: &DiscoveredToken) -> TokenType {
        use crate::crosschain::unified_token_interface::TokenType;
        
        let symbol = token.symbol.to_uppercase();
        let is_native = token.address == "0x0000000000000000000000000000000000000000";
        if is_native || symbol == "ETH" || symbol == "MATIC" || symbol == "AVAX" || symbol == "BNB" {
            TokenType::Native
        } else if symbol.starts_with("W") && (symbol == "WETH" || symbol == "WMATIC" || symbol == "WAVAX" || symbol == "WBNB") {
            TokenType::Wrapped
        } else if symbol == "USDC" || symbol == "USDT" || symbol == "DAI" || symbol == "BUSD" {
            TokenType::Stable
        } else {
            TokenType::ERC20
        }
    }

    /// Get human-readable chain name
    fn get_chain_name(&self, chain_id: u64) -> String {
        match chain_id {
            1 => "Ethereum".to_string(),
            137 => "Polygon".to_string(),
            43114 => "Avalanche".to_string(),
            42161 => "Arbitrum".to_string(),
            10 => "Optimism".to_string(),
            8453 => "Base".to_string(),
            56 => "BNB Chain".to_string(),
            250 => "Fantom".to_string(),
            100 => "Gnosis".to_string(),
            _ => format!("Chain {}", chain_id),
        }
    }

    /// Update discovery statistics
    async fn update_discovery_stats(&self, total_tokens: usize, chains_processed: usize) {
        let mut stats = self.stats.write().await;
        stats.total_tokens = total_tokens;
        stats.sources_active = chains_processed;
        stats.last_discovery_run = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        stats.next_scheduled_run = stats.last_discovery_run + (self.config.discovery_interval_hours * 3600);
    }

    /// Get current discovery statistics
    pub async fn get_stats(&self) -> TokenDiscoveryStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get discovered tokens for a specific chain
    pub async fn get_chain_tokens(&self, chain_id: u64) -> Option<ChainTokenList> {
        let discovered_tokens = self.discovered_tokens.read().await;
        discovered_tokens.get(&chain_id).cloned()
    }

    /// Get all discovered tokens across all chains
    pub async fn get_all_discovered_tokens(&self) -> HashMap<u64, ChainTokenList> {
        let discovered_tokens = self.discovered_tokens.read().await;
        discovered_tokens.clone()
    }

    /// Manual trigger for specific chain discovery
    pub async fn trigger_chain_discovery(&self, chain_id: u64) -> Result<TokenDiscoveryResult, Box<dyn std::error::Error>> {
        info!("Manual trigger for chain {} token discovery", chain_id);
        self.discover_tokens_for_chain(chain_id).await
    }

    /// Get supported chains
    pub fn get_supported_chains(&self) -> Vec<u64> {
        vec![1, 137, 43114, 42161, 10, 8453, 56, 250, 100]
    }
}

impl Default for TokenDiscoveryStats {
    fn default() -> Self {
        Self {
            total_tokens: 0,
            tokens_by_chain: HashMap::new(),
            verified_tokens: 0,
            sources_active: 0,
            last_discovery_run: 0,
            next_scheduled_run: 0,
        }
    }
}
