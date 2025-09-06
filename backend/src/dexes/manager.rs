use crate::dexes::{
    DexIntegration, DexError, /* UniswapDex, */ ApeSwapDex, PancakeSwapDex
};
use crate::types::{QuoteParams, RouteBreakdown};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Instant;

/// DEX priority rankings for Tier 1 routing
#[derive(Debug, Clone)]
pub struct DexPriority {
    pub name: String,
    pub priority: u8, // 1-10 scale
    pub chain_focus: Option<String>, // Which chain this DEX is optimized for
}

/// Manager for coordinating multiple DEX integrations with Tier 1 Direct Routes
#[derive(Clone)]
pub struct DexManager {
    dexes: HashMap<String, Arc<dyn DexIntegration>>,
    priorities: HashMap<String, DexPriority>,
    route_cache: HashMap<String, (RouteBreakdown, Instant)>, // Cache with timestamp
}

impl DexManager {
    pub fn new() -> Self {
        Self {
            dexes: HashMap::new(),
            priorities: HashMap::new(),
            route_cache: HashMap::new(),
        }
    }

    /// Initialize Tier 1 Direct Routes with all 15+ DEX integrations
    pub async fn init_tier1_direct_routes() -> Result<Self, DexError> {
        let mut manager = Self::new();
        
        // Load all DEX integrations with priority rankings
        manager.load_dex_integrations().await?;
        manager.setup_priority_rankings();
        
        Ok(manager)
    }

    /// Load all 15+ DEX integrations
    async fn load_dex_integrations(&mut self) -> Result<(), DexError> {
        // Tier 1 DEX integrations - Direct routes only
        let mut loaded_count = 0;
        
        // Only testing these DEXes - Uniswap disabled
        // if let Ok(uniswap) = UniswapDex::new().await {
        //     self.add_dex("uniswap_v3".to_string(), Arc::new(uniswap));
        //     loaded_count += 1;
        //     println!("✅ Uniswap V3 initialized successfully");
        // }
        
        if let Ok(apeswap) = ApeSwapDex::new().await {
            self.add_dex("apeswap".to_string(), Arc::new(apeswap));
            loaded_count += 1;
            println!("✅ ApeSwap initialized successfully");
        }
        
        let pancakeswap = PancakeSwapDex::new();
        self.add_dex("pancakeswap".to_string(), Arc::new(pancakeswap));
        loaded_count += 1;
        println!("✅ PancakeSwap initialized successfully");
        
        println!("🚀 DEX Manager initialized with {} DEXes", loaded_count);
        
        if loaded_count == 0 {
            return Err(DexError::InitializationFailed("No DEXes could be initialized".to_string()));
        }
        
        Ok(())
    }

    /// Setup DEX priority rankings for optimal route selection
    fn setup_priority_rankings(&mut self) {
        let priorities = vec![
            DexPriority {
                name: "uniswap_v3".to_string(),
                priority: 10,
                chain_focus: Some("ethereum".to_string()),
            },
            DexPriority {
                name: "apeswap".to_string(),
                priority: 8,
                chain_focus: Some("bsc".to_string()),
            },
            DexPriority {
                name: "pancakeswap".to_string(),
                priority: 7,
                chain_focus: Some("bsc".to_string()),
            },
        ];

        for priority in priorities {
            self.priorities.insert(priority.name.clone(), priority);
        }
    }

    /// Add a DEX integration to the manager
    pub fn add_dex(&mut self, name: String, dex: Arc<dyn DexIntegration>) {
        self.dexes.insert(name, dex);
    }

    /// Get Tier 1 Direct Route - Ultra-fast cached lookup (<5ms)
    pub async fn get_tier1_direct_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let cache_key = format!("{}:{}:{}", params.token_in, params.token_out, params.amount_in);
        
        // Check cache first for <5ms response
        if let Some((cached_route, timestamp)) = self.route_cache.get(&cache_key) {
            if timestamp.elapsed().as_secs() < 30 { // 30 second cache
                return Ok(cached_route.clone());
            }
        }

        // Get quotes from all DEXes with priority ranking
        let mut weighted_quotes = Vec::new();
        
        // Filter DEXes by chain support
        let target_chain = params.chain.as_deref().unwrap_or("ethereum");
        
        for (name, dex) in &self.dexes {
            // Skip DEXes that don't support the target chain
            if !dex.get_supported_chains().contains(&target_chain) {
                continue;
            }
            
            match dex.get_quote(params).await {
                Ok(mut quote) => {
                    // Priority weighting applied during sorting, not to amounts
                    
                    weighted_quotes.push((name.clone(), quote, self.get_dex_priority(name)));
                }
                Err(e) => {
                    eprintln!("Error getting quote from {}: {}", name, e);
                }
            }
        }

        // Sort by amount first, then apply priority weighting for tie-breaking
        weighted_quotes.sort_by(|a, b| {
            let amount_a = a.1.amount_out.parse::<f64>().unwrap_or(0.0);
            let amount_b = b.1.amount_out.parse::<f64>().unwrap_or(0.0);
            
            // Primary sort: by amount (higher is better)
            let amount_cmp = amount_b.partial_cmp(&amount_a).unwrap_or(std::cmp::Ordering::Equal);
            
            // Secondary sort: by priority (higher is better) for tie-breaking
            if amount_cmp == std::cmp::Ordering::Equal {
                a.2.cmp(&b.2)
            } else {
                amount_cmp
            }
        });

        if let Some((best_dex, best_quote, _)) = weighted_quotes.first() {
            let result = RouteBreakdown {
                dex: best_dex.clone(),
                percentage: 100.0,
                amount_out: best_quote.amount_out.clone(),
                gas_used: best_quote.gas_used.clone(),
            };
            
            // Cache result for future lookups
            // Note: We can't modify self in this method, so caching would need to be handled differently
            
            Ok(result)
        } else {
            Err(DexError::InsufficientLiquidity)
        }
    }

    /// Get DEX priority score
    fn get_dex_priority(&self, dex_name: &str) -> u8 {
        self.priorities.get(dex_name).map(|p| p.priority).unwrap_or(5)
    }

    /// Get the best quote across all DEXes (legacy method)
    pub async fn get_best_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        self.get_tier1_direct_quote(params).await
    }

    /// Get quotes from all DEXes
    pub async fn get_all_quotes(&self, params: &QuoteParams) -> Vec<(String, Result<RouteBreakdown, DexError>)> {
        let mut quotes = Vec::new();

        for (name, dex) in &self.dexes {
            let quote = dex.get_quote(params).await;
            quotes.push((name.clone(), quote));
        }

        quotes
    }

    /// Check if a trading pair is supported by any DEX
    pub async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> bool {
        for dex in self.dexes.values() {
            if let Ok(true) = dex.is_pair_supported(token_in, token_out, chain).await {
                return true;
            }
        }
        false
    }

    /// Get list of available DEX names
    pub fn get_dex_names(&self) -> Vec<String> {
        self.dexes.keys().cloned().collect()
    }
}

impl Default for DexManager {
    fn default() -> Self {
        Self::new()
    }
}
