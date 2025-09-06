use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use tokio::time::{sleep, Duration};

use super::DiscoveredToken;

#[async_trait]
pub trait TokenSource: Send + Sync {
    async fn fetch_tokens(&self, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError>;
    fn source_name(&self) -> &str;
    fn supported_chains(&self) -> Vec<u64>;
    fn priority(&self) -> u8; // Higher number = higher priority
}

#[derive(Debug, thiserror::Error)]
pub enum TokenSourceError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Chain not supported: {0}")]
    ChainNotSupported(u64),
    #[error("API error: {0}")]
    ApiError(String),
}

pub struct TokenSourceManager {
    sources: Vec<Box<dyn TokenSource + Send + Sync>>,
    http_client: Client,
    last_request_times: HashMap<String, std::time::Instant>,
    min_request_interval: Duration,
}

impl TokenSourceManager {
    pub fn new() -> Self {
        let mut sources: Vec<Box<dyn TokenSource + Send + Sync>> = Vec::new();
        
        // Add all token sources
        sources.push(Box::new(OneInchTokenSource::new()));
        sources.push(Box::new(UniswapTokenSource::new()));
        sources.push(Box::new(CoinGeckoTokenSource::new()));
        sources.push(Box::new(ChainTokenListSource::new()));
        sources.push(Box::new(DEXTokenSource::new()));
        
        Self {
            sources,
            http_client: Client::new(),
            last_request_times: HashMap::new(),
            min_request_interval: Duration::from_millis(100), // 100ms between requests
        }
    }

    pub fn get_sources(&self) -> &Vec<Box<dyn TokenSource + Send + Sync>> {
        &self.sources
    }

    pub async fn fetch_all_tokens(&mut self, chain_id: u64) -> HashMap<String, Vec<DiscoveredToken>> {
        let mut results = HashMap::new();
        
        for source in &self.sources {
            if !source.supported_chains().contains(&chain_id) {
                continue;
            }
            
            // Rate limiting
            let source_name = source.source_name();
            if let Some(last_time) = self.last_request_times.get(source_name) {
                let elapsed = last_time.elapsed();
                if elapsed < self.min_request_interval {
                    let wait_time = self.min_request_interval - elapsed;
                    sleep(wait_time).await;
                }
            }
            
            match source.fetch_tokens(chain_id).await {
                Ok(tokens) => {
                    info!("Fetched {} tokens from {}", tokens.len(), source_name);
                    results.insert(source_name.to_string(), tokens);
                }
                Err(e) => {
                    error!("Failed to fetch tokens from {}: {}", source_name, e);
                }
            }
            
            self.last_request_times.insert(source_name.to_string(), std::time::Instant::now());
        }
        
        results
    }
}

// 1inch Token Source - FIXED with fallback endpoints and better error handling
pub struct OneInchTokenSource {
    client: Client,
}

impl OneInchTokenSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    async fn try_fetch_from_url(&self, url: &str, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        let response: Value = self.client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .json()
            .await?;
        
        // Handle both old format {"tokens": {...}} and new format {...}
        let tokens_obj = if let Some(tokens) = response.get("tokens") {
            tokens.as_object()
        } else {
            response.as_object()
        }.ok_or_else(|| TokenSourceError::ApiError("Invalid response format".to_string()))?;
        
        let mut tokens = Vec::new();
        
        for (address, token_data) in tokens_obj {
            if let Some(token_obj) = token_data.as_object() {
                tokens.push(DiscoveredToken {
                    symbol: token_data.get("symbol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNKNOWN")
                        .to_string(),
                    name: token_data.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown Token")
                        .to_string(),
                    address: address.to_string(),
                    decimals: token_data.get("decimals")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(18) as u8,
                    chain_id,
                    logo_uri: token_data.get("logoURI")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    coingecko_id: None,
                    source: "1inch".to_string(),
                    verified: false,
                    trading_volume_24h: None,
                    market_cap: None,
                    discovered_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }
        
        Ok(tokens)
    }
}

#[async_trait]
impl TokenSource for OneInchTokenSource {
    fn source_name(&self) -> &str {
        "1inch"
    }

    fn supported_chains(&self) -> Vec<u64> {
        vec![1, 56, 137, 43114, 42161, 10] // Ethereum, BSC, Polygon, Avalanche, Arbitrum, Optimism
    }

    fn priority(&self) -> u8 {
        8
    }

    async fn fetch_tokens(&self, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        // Try the new public API endpoint first
        let public_url = format!("https://api.1inch.io/v5.0/{}/tokens", chain_id);
        
        match self.try_fetch_from_url(&public_url, chain_id).await {
            Ok(tokens) => return Ok(tokens),
            Err(e) => {
                warn!("1inch public API failed: {}, trying fallback", e);
                // Try alternative endpoint
                let fallback_url = format!("https://tokens.1inch.io/v1.1/{}", chain_id);
                return self.try_fetch_from_url(&fallback_url, chain_id).await;
            }
        }
    }
}

// CoinGecko Token Source - FIXED with better error handling and response validation
pub struct CoinGeckoTokenSource {
    client: Client,
}

impl CoinGeckoTokenSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn get_platform_name(&self, chain_id: u64) -> Option<&str> {
        match chain_id {
            1 => Some("ethereum"),
            56 => Some("binance-smart-chain"),
            137 => Some("polygon-pos"),
            43114 => Some("avalanche"),
            42161 => Some("arbitrum-one"),
            10 => Some("optimistic-ethereum"),
            250 => Some("fantom"),
            25 => Some("cronos"),
            _ => None,
        }
    }
}

#[async_trait]
impl TokenSource for CoinGeckoTokenSource {
    fn source_name(&self) -> &str {
        "coingecko"
    }

    fn supported_chains(&self) -> Vec<u64> {
        vec![1, 56, 137, 43114, 42161, 10, 250, 25] // Major chains
    }

    fn priority(&self) -> u8 {
        7
    }

    async fn fetch_tokens(&self, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        let url = "https://api.coingecko.com/api/v3/coins/list?include_platform=true";
        
        let response: Value = self.client
            .get(url)
            .timeout(Duration::from_secs(15))
            .header("Accept", "application/json")
            .send()
            .await?
            .json()
            .await?;
        
        // Handle both array response and potential error object
        let coins_array = if response.is_array() {
            response.as_array().unwrap()
        } else if let Some(error) = response.get("error") {
            return Err(TokenSourceError::ApiError(format!("CoinGecko API error: {}", error)));
        } else {
            return Err(TokenSourceError::ApiError("Unexpected response format from CoinGecko".to_string()));
        };
        
        let platform_name = self.get_platform_name(chain_id)
            .ok_or_else(|| TokenSourceError::ChainNotSupported(chain_id))?;
        
        let mut tokens = Vec::new();
        
        for coin in coins_array {
            if let Some(coin_obj) = coin.as_object() {
                if let (Some(platforms), Some(symbol), Some(name)) = (
                    coin_obj.get("platforms").and_then(|v| v.as_object()),
                    coin_obj.get("symbol").and_then(|v| v.as_str()),
                    coin_obj.get("name").and_then(|v| v.as_str())
                ) {
                    if let Some(address) = platforms.get(platform_name).and_then(|v| v.as_str()) {
                        if !address.is_empty() && address != "0x" {
                            tokens.push(DiscoveredToken {
                                address: address.to_string(),
                                symbol: symbol.to_string(),
                                name: name.to_string(),
                                decimals: 18, // Default, would need another API call for exact decimals
                                chain_id,
                                logo_uri: coin_obj.get("image")
                                    .and_then(|v| v.get("thumb"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                coingecko_id: coin_obj.get("id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                source: "coingecko".to_string(),
                                verified: false,
                                trading_volume_24h: None,
                                market_cap: None,
                                discovered_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                            });
                        }
                    }
                }
            }
        }
        
        info!("CoinGecko fetched {} tokens for chain {}", tokens.len(), chain_id);
        Ok(tokens)
    }
}

// Chain Token List Source - FIXED with multiple URLs and better JSON parsing
pub struct ChainTokenListSource {
    client: Client,
}

impl ChainTokenListSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn get_chain_token_list_urls(&self, chain_id: u64) -> Option<Vec<&str>> {
        match chain_id {
            1 => Some(vec![
                "https://tokens.uniswap.org",
                "https://tokens.coingecko.com/uniswap/all.json",
                "https://raw.githubusercontent.com/compound-finance/token-list/master/compound.tokenlist.json"
            ]), // Ethereum
            56 => Some(vec![
                "https://tokens.pancakeswap.finance/pancakeswap-extended.json",
                "https://raw.githubusercontent.com/pancakeswap/pancake-toolkit/master/packages/token-lists/lists/pancakeswap-default.json"
            ]), // BSC
            137 => Some(vec![
                "https://unpkg.com/quickswap-default-token-list@1.2.28/build/quickswap-default.tokenlist.json",
                "https://wallet-asset.matic.network/data/listMatic.json"
            ]), // Polygon
            43114 => Some(vec![
                "https://raw.githubusercontent.com/traderjoe-xyz/joe-tokenlists/main/joe.tokenlist.json",
                "https://raw.githubusercontent.com/pangolindex/tokenlists/main/pangolin.tokenlist.json"
            ]), // Avalanche
            42161 => Some(vec![
                "https://bridge.arbitrum.io/token-list-42161.json",
                "https://raw.githubusercontent.com/sushiswap/default-token-list/master/tokens/arbitrum.json"
            ]), // Arbitrum
            10 => Some(vec![
                "https://static.optimism.io/optimism.tokenlist.json",
                "https://raw.githubusercontent.com/ethereum-optimism/ethereum-optimism.github.io/master/optimism.tokenlist.json"
            ]), // Optimism
            250 => Some(vec![
                "https://raw.githubusercontent.com/SpookySwap/spooky-info/master/src/constants/token/spookyswap.json"
            ]), // Fantom
            _ => None,
        }
    }

    async fn fetch_from_url(&self, url: &str, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        let response_text = self.client
            .get(url)
            .timeout(Duration::from_secs(10))
            .header("Accept", "application/json")
            .header("User-Agent", "HyperDEX-TokenRegistry/1.0")
            .send()
            .await?
            .text()
            .await?;
        
        // Clean up response text to handle potential BOM or extra characters
        let cleaned_text = response_text.trim().trim_start_matches('\u{feff}');
        
        let response: Value = serde_json::from_str(cleaned_text)
            .map_err(|e| TokenSourceError::ApiError(format!("JSON parse error: {}", e)))?;
        
        let tokens_array = response
            .get("tokens")
            .ok_or_else(|| TokenSourceError::ApiError("No tokens field in response".to_string()))?
            .as_array()
            .ok_or_else(|| TokenSourceError::ApiError("Tokens field is not an array".to_string()))?;
        
        let mut tokens = Vec::new();
        
        for token in tokens_array {
            if let Some(token_obj) = token.as_object() {
                if let (Some(address), Some(symbol), Some(name), Some(decimals), Some(token_chain_id)) = (
                    token_obj.get("address").and_then(|v| v.as_str()),
                    token_obj.get("symbol").and_then(|v| v.as_str()),
                    token_obj.get("name").and_then(|v| v.as_str()),
                    token_obj.get("decimals").and_then(|v| v.as_u64()),
                    token_obj.get("chainId").and_then(|v| v.as_u64())
                ) {
                    if token_chain_id == chain_id && !address.is_empty() {
                        tokens.push(DiscoveredToken {
                            address: address.to_string(),
                            symbol: symbol.to_string(),
                            name: name.to_string(),
                            decimals: decimals as u8,
                            chain_id,
                            logo_uri: token_obj.get("logoURI")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            coingecko_id: None,
                            source: "chain_lists".to_string(),
                            verified: false,
                            trading_volume_24h: None,
                            market_cap: None,
                            discovered_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                    }
                }
            }
        }
        
        Ok(tokens)
    }
}

#[async_trait]
impl TokenSource for ChainTokenListSource {
    fn source_name(&self) -> &str {
        "chain_lists"
    }

    fn supported_chains(&self) -> Vec<u64> {
        vec![1, 56, 137, 43114, 42161, 10, 250] // Chains with known token lists
    }

    fn priority(&self) -> u8 {
        6
    }

    async fn fetch_tokens(&self, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        let list_urls = self.get_chain_token_list_urls(chain_id)
            .ok_or(TokenSourceError::ChainNotSupported(chain_id))?;
        
        let mut all_tokens = Vec::new();
        
        for url in list_urls {
            match self.fetch_from_url(&url, chain_id).await {
                Ok(mut tokens) => {
                    info!("Fetched {} tokens from {}", tokens.len(), url);
                    all_tokens.append(&mut tokens);
                }
                Err(e) => {
                    warn!("Failed to fetch from {}: {}", url, e);
                    // Continue with other URLs
                }
            }
        }
        
        // Deduplicate by address
        all_tokens.sort_by(|a, b| a.address.cmp(&b.address));
        all_tokens.dedup_by(|a, b| a.address == b.address);
        
        Ok(all_tokens)
    }
}

// Uniswap Token Source
pub struct UniswapTokenSource {
    client: Client,
}

impl UniswapTokenSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl TokenSource for UniswapTokenSource {
    fn source_name(&self) -> &str {
        "uniswap"
    }

    fn supported_chains(&self) -> Vec<u64> {
        vec![1, 10, 42161, 137, 56] // Ethereum, Optimism, Arbitrum, Polygon, BSC
    }

    fn priority(&self) -> u8 {
        9
    }

    async fn fetch_tokens(&self, chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        let url = "https://tokens.uniswap.org/";
        
        let response: Value = self.client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .json()
            .await?;
        
        let tokens_array = response
            .get("tokens")
            .ok_or_else(|| TokenSourceError::ApiError("No tokens field in response".to_string()))?
            .as_array()
            .ok_or_else(|| TokenSourceError::ApiError("Tokens field is not an array".to_string()))?;
        
        let mut tokens = Vec::new();
        
        for token in tokens_array {
            if let Some(token_obj) = token.as_object() {
                if let (Some(address), Some(symbol), Some(name), Some(decimals), Some(token_chain_id)) = (
                    token_obj.get("address").and_then(|v| v.as_str()),
                    token_obj.get("symbol").and_then(|v| v.as_str()),
                    token_obj.get("name").and_then(|v| v.as_str()),
                    token_obj.get("decimals").and_then(|v| v.as_u64()),
                    token_obj.get("chainId").and_then(|v| v.as_u64())
                ) {
                    if token_chain_id == chain_id {
                        tokens.push(DiscoveredToken {
                            address: address.to_string(),
                            symbol: symbol.to_string(),
                            name: name.to_string(),
                            decimals: decimals as u8,
                            chain_id,
                            logo_uri: token_obj.get("logoURI")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            coingecko_id: None,
                            source: "uniswap".to_string(),
                            verified: false,
                            trading_volume_24h: None,
                            market_cap: None,
                            discovered_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        });
                    }
                }
            }
        }
        
        Ok(tokens)
    }
}

// DEX Token Source (placeholder for other DEX-specific sources)
pub struct DEXTokenSource {
    client: Client,
}

impl DEXTokenSource {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl TokenSource for DEXTokenSource {
    fn source_name(&self) -> &str {
        "dex_aggregated"
    }

    fn supported_chains(&self) -> Vec<u64> {
        vec![1, 56, 137, 43114, 42161, 10] // Major chains
    }

    fn priority(&self) -> u8 {
        5
    }

    async fn fetch_tokens(&self, _chain_id: u64) -> Result<Vec<DiscoveredToken>, TokenSourceError> {
        // Placeholder - could aggregate from multiple DEX-specific sources
        Ok(Vec::new())
    }
}
