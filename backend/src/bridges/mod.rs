use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// Re-export bridge implementations
pub mod hop_protocol;
pub mod across_protocol;
pub mod stargate_finance;
pub mod synapse_protocol;
pub mod multichain;
pub mod celer_cbridge;
pub mod multi_hop;
pub mod load_testing;
pub mod monitoring;
pub mod openapi;
pub mod config;

pub use hop_protocol::HopProtocol;
pub use across_protocol::AcrossProtocol;
pub use stargate_finance::StargateFinance;
pub use synapse_protocol::SynapseProtocol;
pub use multichain::Multichain;
pub use celer_cbridge::CelerCBridge;
pub use multi_hop::{MultiHopRouter, MultiHopRoute, MultiHopParams, MultiHopExecution, RouteHop};
pub use config::{BridgeConfig, BridgeSettings, ChainConfig, MonitoringConfig, RateLimitConfig, SecurityConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainParams {
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub user_address: String,
    pub slippage: f64,
    pub deadline: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeQuote {
    pub bridge_name: String,
    pub amount_out: String,
    pub estimated_time: u64, // seconds
    pub fee: String,
    pub gas_estimate: String,
    pub route: Vec<BridgeStep>,
    pub confidence_score: f64, // 0.0 to 1.0
    pub liquidity_available: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStep {
    pub bridge: String,
    pub from_chain: u64,
    pub to_chain: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub estimated_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub transaction_hash: String,
    pub bridge_id: String,
    pub status: BridgeStatus,
    pub estimated_completion: u64,
    pub tracking_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Refunded,
}

#[derive(Debug, Clone)]
pub enum BridgeError {
    NetworkError(String),
    InsufficientLiquidity,
    UnsupportedRoute,
    InvalidParameters(String),
    BridgeUnavailable,
    InvalidRoute,
    InvalidAmount,
    ExecutionFailed(String),
    ParseError(String),
    QuoteExpired, // Added this line
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            BridgeError::InsufficientLiquidity => write!(f, "Insufficient liquidity"),
            BridgeError::UnsupportedRoute => write!(f, "Unsupported route"),
            BridgeError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            BridgeError::BridgeUnavailable => write!(f, "Bridge unavailable"),
            BridgeError::InvalidRoute => write!(f, "Invalid route"),
            BridgeError::InvalidAmount => write!(f, "Invalid amount"),
            BridgeError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            BridgeError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            BridgeError::QuoteExpired => write!(f, "Quote expired"),
        }
    }
}

impl std::error::Error for BridgeError {}

#[async_trait]
pub trait BridgeIntegration: Send + Sync {
    /// Get bridge name
    fn name(&self) -> &str;
    
    /// Check if bridge supports the given route
    fn supports_route(&self, from_chain: u64, to_chain: u64) -> bool;
    
    /// Get supported tokens for a chain
    fn get_supported_tokens(&self, chain_id: u64) -> Vec<String>;
    
    /// Get quote for cross-chain transfer
    async fn get_quote(&self, params: &CrossChainParams) -> Result<BridgeQuote, BridgeError>;
    
    /// Execute cross-chain transfer
    async fn execute_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError>;
    
    /// Get transfer status
    async fn get_status(&self, bridge_id: &str) -> Result<BridgeStatus, BridgeError>;
    
    /// Get bridge health status
    async fn health_check(&self) -> Result<bool, BridgeError>;
}

pub struct BridgeManager {
    bridges: HashMap<String, Box<dyn BridgeIntegration>>,
    chain_priorities: HashMap<(u64, u64), Vec<String>>, // (from_chain, to_chain) -> bridge priority
}

impl BridgeManager {
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
            chain_priorities: HashMap::new(),
        }
    }
    
    pub fn add_bridge(&mut self, bridge: Box<dyn BridgeIntegration>) {
        let name = bridge.name().to_string();
        self.bridges.insert(name, bridge);
    }
    
    pub fn set_chain_priority(&mut self, from_chain: u64, to_chain: u64, priorities: Vec<String>) {
        self.chain_priorities.insert((from_chain, to_chain), priorities);
    }
    
    pub async fn get_best_quote(&self, params: &CrossChainParams) -> Result<BridgeQuote, BridgeError> {
        let mut quotes = Vec::new();
        
        // Get priority order for this route
        let priority_bridges = self.chain_priorities
            .get(&(params.from_chain_id, params.to_chain_id))
            .cloned()
            .unwrap_or_else(|| self.bridges.keys().cloned().collect());
        
        // Get quotes from bridges in priority order
        for bridge_name in &priority_bridges {
            if let Some(bridge) = self.bridges.get(bridge_name) {
                if bridge.supports_route(params.from_chain_id, params.to_chain_id) {
                    match bridge.get_quote(params).await {
                        Ok(quote) => quotes.push(quote),
                        Err(e) => {
                            tracing::warn!("Bridge {} failed to provide quote: {}", bridge_name, e);
                        }
                    }
                }
            }
        }
        
        if quotes.is_empty() {
            return Err(BridgeError::UnsupportedRoute);
        }
        
        // Score and rank quotes
        quotes.sort_by(|a, b| {
            let score_a = self.calculate_quote_score(a);
            let score_b = self.calculate_quote_score(b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        Ok(quotes.into_iter().next().unwrap())
    }
    
    pub async fn get_all_quotes(&self, params: &CrossChainParams) -> Vec<BridgeQuote> {
        let mut quotes = Vec::new();
        
        for bridge in self.bridges.values() {
            if bridge.supports_route(params.from_chain_id, params.to_chain_id) {
                match bridge.get_quote(params).await {
                    Ok(quote) => quotes.push(quote),
                    Err(e) => {
                        tracing::warn!("Bridge {} failed to provide quote: {}", bridge.name(), e);
                    }
                }
            }
        }
        
        // Sort by score
        quotes.sort_by(|a, b| {
            let score_a = self.calculate_quote_score(a);
            let score_b = self.calculate_quote_score(b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        quotes
    }
    
    fn calculate_quote_score(&self, quote: &BridgeQuote) -> f64 {
        let amount_out: f64 = quote.amount_out.parse().unwrap_or(0.0);
        let fee: f64 = quote.fee.parse().unwrap_or(0.0);
        let time_penalty = 1.0 / (1.0 + quote.estimated_time as f64 / 3600.0); // Penalize longer times
        
        // Weighted scoring: 50% amount out, 30% confidence, 20% time
        let score = (amount_out - fee) * 0.5 + quote.confidence_score * 0.3 + time_penalty * 0.2;
        score
    }
    
    pub async fn execute_best_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError> {
        let best_quote = self.get_best_quote(params).await?;
        
        if let Some(bridge) = self.bridges.get(&best_quote.bridge_name) {
            bridge.execute_bridge(params).await
        } else {
            Err(BridgeError::BridgeUnavailable)
        }
    }
    
    pub fn get_supported_routes(&self) -> Vec<(u64, u64)> {
        let mut routes = std::collections::HashSet::new();
        
        // Common chain IDs
        let chains = vec![1, 10, 42161, 137, 43114, 250, 56]; // Ethereum, Optimism, Arbitrum, Polygon, Avalanche, Fantom, BSC
        
        for from_chain in &chains {
            for to_chain in &chains {
                if from_chain != to_chain {
                    for bridge in self.bridges.values() {
                        if bridge.supports_route(*from_chain, *to_chain) {
                            routes.insert((*from_chain, *to_chain));
                            break;
                        }
                    }
                }
            }
        }
        
        routes.into_iter().collect()
    }
}

// Chain ID constants
pub const ETHEREUM_CHAIN_ID: u64 = 1;
pub const OPTIMISM_CHAIN_ID: u64 = 10;
pub const ARBITRUM_CHAIN_ID: u64 = 42161;
pub const POLYGON_CHAIN_ID: u64 = 137;
pub const AVALANCHE_CHAIN_ID: u64 = 43114;
pub const FANTOM_CHAIN_ID: u64 = 250;
pub const BSC_CHAIN_ID: u64 = 56;
pub const BASE_CHAIN_ID: u64 = 8453;

// Common token addresses
pub fn get_token_address(chain_id: u64, symbol: &str) -> Option<String> {
    match (chain_id, symbol) {
        // Ethereum mainnet
        (1, "ETH") => Some("0x0000000000000000000000000000000000000000".to_string()),
        (1, "WETH") => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
        (1, "USDC") => Some("0xA0b86a33E6441E6C5a6F6c7e2C0d3C8C8a2B0e8B".to_string()),
        (1, "USDT") => Some("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
        (1, "DAI") => Some("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
        
        // Arbitrum
        (42161, "ETH") => Some("0x0000000000000000000000000000000000000000".to_string()),
        (42161, "WETH") => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
        (42161, "USDC") => Some("0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".to_string()),
        (42161, "USDT") => Some("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string()),
        
        // Optimism
        (10, "ETH") => Some("0x0000000000000000000000000000000000000000".to_string()),
        (10, "WETH") => Some("0x4200000000000000000000000000000000000006".to_string()),
        (10, "USDC") => Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()),
        (10, "USDT") => Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string()),
        
        // Polygon
        (137, "MATIC") => Some("0x0000000000000000000000000000000000000000".to_string()),
        (137, "WMATIC") => Some("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270".to_string()),
        (137, "USDC") => Some("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
        (137, "USDT") => Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
        
        _ => None,
    }
}
