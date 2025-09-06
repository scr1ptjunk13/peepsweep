pub mod discovery_engine;
pub mod token_sources;
pub mod validation_service;
pub mod scheduler;
pub mod integration_service;

pub use discovery_engine::{TokenDiscoveryService, TokenDiscoveryResult};
pub use token_sources::{TokenSource, TokenSourceManager};
pub use validation_service::TokenValidationService;
pub use scheduler::TokenDiscoveryScheduler;
pub use integration_service::{TokenRegistryIntegrationService, TokenRegistryHealthStatus, SyncResult};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredToken {
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub decimals: u8,
    pub chain_id: u64,
    pub logo_uri: Option<String>,
    pub coingecko_id: Option<String>,
    pub source: String,
    pub verified: bool,
    pub trading_volume_24h: Option<f64>,
    pub market_cap: Option<f64>,
    pub discovered_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainTokenList {
    pub chain_id: u64,
    pub chain_name: String,
    pub tokens: Vec<DiscoveredToken>,
    pub last_updated: u64,
    pub source_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDiscoveryStats {
    pub total_tokens: usize,
    pub tokens_by_chain: HashMap<u64, usize>,
    pub verified_tokens: usize,
    pub sources_active: usize,
    pub last_discovery_run: u64,
    pub next_scheduled_run: u64,
}

#[derive(Debug, Clone)]
pub struct TokenRegistryConfig {
    pub discovery_interval_hours: u64,
    pub max_tokens_per_chain: usize,
    pub min_trading_volume: f64,
    pub enable_verification: bool,
    pub redis_url: Option<String>,
    pub rate_limit_per_minute: u32,
}

impl Default for TokenRegistryConfig {
    fn default() -> Self {
        Self {
            discovery_interval_hours: 0, // Immediate discovery on startup
            max_tokens_per_chain: 5000,
            min_trading_volume: 10000.0, // $10k daily volume minimum
            enable_verification: true,
            redis_url: None,
            rate_limit_per_minute: 60,
        }
    }
}
