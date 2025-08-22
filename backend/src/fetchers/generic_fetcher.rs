use std::collections::HashMap;
use std::sync::Arc;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::BlockNumberOrTag,
    contract::{Contract, Interface},
    sol,
    sol_types::SolCall,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::fetchers::config_parser::{ProtocolConfig, ChainConfig, ContractFunction};
use crate::cache::CacheManager;
use crate::database::models::Position;

// Uniswap V3 Position Manager ABI
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract INonfungiblePositionManager {
        function balanceOf(address owner) external view returns (uint256);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
        function positions(uint256 tokenId) external view returns (
            uint96 nonce,
            address operator,
            address token0,
            address token1,
            uint24 fee,
            int24 tickLower,
            int24 tickUpper,
            uint128 liquidity,
            uint256 feeGrowthInside0LastX128,
            uint256 feeGrowthInside1LastX128,
            uint128 tokensOwed0,
            uint128 tokensOwed1
        );
    }
}

// ERC20 ABI for balance checking
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function totalSupply() external view returns (uint256);
    }
}

// Uniswap V2 Pair ABI
sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    contract IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function totalSupply() external view returns (uint256);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardPosition {
    pub protocol: String,
    pub position_id: String,
    pub user_address: Address,
    pub chain_id: u32,
    pub position_type: String,
    pub token0: Address,
    pub token1: Address,
    pub liquidity: U256,
    pub tick_lower: Option<i32>,
    pub tick_upper: Option<i32>,
    pub fee_tier: Option<u32>,
    pub value_usd: f64,
    pub unclaimed_fees_0: U256,
    pub unclaimed_fees_1: U256,
    pub impermanent_loss: Option<ImpermanentLossInfo>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpermanentLossInfo {
    pub percentage: f64,
    pub usd_amount: f64,
    pub is_gain: bool,
    pub predicted_24h: f64,
    pub confidence: f64,
}

#[derive(Clone)]
pub struct GenericFetcher {
    configs: HashMap<String, ProtocolConfig>,
    cache: Arc<CacheManager>,
}

impl GenericFetcher {
    pub async fn new(cache: Arc<CacheManager>) -> Result<Self> {
        let configs = crate::fetchers::config_parser::ConfigParser::load_protocol_configs()?;
        
        // Validate all loaded configs
        for (name, config) in &configs {
            if let Err(e) = crate::fetchers::config_parser::ConfigParser::validate_config(config) {
                error!("Invalid configuration for protocol {}: {}", name, e);
                return Err(e);
            }
        }
        
        info!("GenericFetcher initialized with {} protocols", configs.len());
        
        Ok(Self {
            configs,
            cache,
        })
    }
    
    pub fn get_protocol_names(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }
    
    pub fn get_supported_chains(&self, protocol_name: &str) -> Option<Vec<u32>> {
        self.configs.get(protocol_name)
            .map(|config| config.protocol.chains.keys().cloned().collect())
    }
    
    /// THE MAGIC: One function handles ALL protocols
    pub async fn fetch_positions_for_protocol(
        &self,
        protocol_name: &str,
        chain_id: u32,
        user_address: Address,
    ) -> Result<Vec<StandardPosition>> {
        let config = self.configs.get(protocol_name)
            .ok_or_else(|| anyhow::anyhow!("Protocol not found: {}", protocol_name))?;
        
        let chain_config = config.protocol.chains.get(&chain_id)
            .ok_or_else(|| anyhow::anyhow!("Chain {} not supported for {}", chain_id, protocol_name))?;
        
        // Cache check
        let cache_key = format!("positions:{}:{}:{:?}", protocol_name, chain_id, user_address);
        if let Ok(Some(cached)) = self.cache.get::<Vec<StandardPosition>>(&cache_key).await {
            info!("Cache hit for positions: {}", cache_key);
            return Ok(cached);
        }
        
        let positions = match config.position_detection.method.as_str() {
            "nft_ownership" => self.fetch_nft_positions(config, chain_config, chain_id, user_address).await?,
            "erc20_balance" => self.fetch_erc20_positions(config, chain_config, chain_id, user_address).await?,
            _ => return Err(anyhow::anyhow!("Unsupported detection method: {}", config.position_detection.method)),
        };
        
        // Cache results
        if !positions.is_empty() {
            let _ = self.cache.set(&cache_key, &positions, config.cache_strategy.ttl as u64).await;
        }
        
        info!("Fetched {} positions for {} on chain {}", positions.len(), protocol_name, chain_id);
        Ok(positions)
    }
    
    async fn fetch_nft_positions(
        &self,
        config: &ProtocolConfig,
        chain_config: &ChainConfig,
        chain_id: u32,
        user_address: Address,
    ) -> Result<Vec<StandardPosition>> {
        let provider = Arc::new(self.get_provider(chain_id).await?);
        let position_manager = chain_config.position_manager_address()?;
        
        // Create contract instance
        let contract = INonfungiblePositionManager::new(position_manager, provider.clone());
        
        // Step 1: Get NFT balance
        let balance = contract.balanceOf(user_address).call().await?;
        let nft_count = balance._0.to::<u32>();
        
        if nft_count == 0 {
            return Ok(Vec::new());
        }
        
        info!("Found {} NFT positions for {:?} on {}", nft_count, user_address, config.protocol.name);
        
        // Step 2: Get all token IDs
        let mut token_ids = Vec::new();
        for i in 0..nft_count as u64 {
            let result = contract.tokenOfOwnerByIndex(user_address, U256::from(i)).call().await?;
            token_ids.push(result._0);
        }
        
        // Step 3: Get position details for each token ID
        let mut positions = Vec::new();
        for token_id in token_ids {
            match self.fetch_nft_position_details(config, &contract, chain_id, user_address, token_id).await {
                Ok(position) => positions.push(position),
                Err(e) => {
                    warn!("Failed to fetch details for token ID {}: {}", token_id, e);
                    continue;
                }
            }
        }
        
        Ok(positions)
    }
    
    async fn fetch_nft_position_details(
        &self,
        config: &ProtocolConfig,
        contract: &INonfungiblePositionManager::INonfungiblePositionManagerInstance<_, Arc<alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>>>,
        chain_id: u32,
        user_address: Address,
        token_id: U256,
    ) -> Result<StandardPosition> {
        // Get position details from contract
        let position_data = contract.positions(token_id).call().await?;
        
        let now = chrono::Utc::now();
        
        // Convert to StandardPosition
        let position = StandardPosition {
            protocol: config.protocol.name.clone(),
            position_id: token_id.to_string(),
            user_address,
            chain_id,
            position_type: config.position_type.clone(),
            token0: position_data.token0,
            token1: position_data.token1,
            liquidity: U256::from(position_data.liquidity),
            tick_lower: Some(position_data.tickLower as i32),
            tick_upper: Some(position_data.tickUpper as i32),
            fee_tier: Some(position_data.fee as u32),
            value_usd: 0.0, // TODO: Calculate USD value
            unclaimed_fees_0: U256::from(position_data.tokensOwed0),
            unclaimed_fees_1: U256::from(position_data.tokensOwed1),
            impermanent_loss: None, // TODO: Calculate IL
            created_at: now,
            updated_at: now,
        };
        
        Ok(position)
    }
    
    async fn fetch_erc20_positions(
        &self,
        config: &ProtocolConfig,
        chain_config: &ChainConfig,
        chain_id: u32,
        user_address: Address,
    ) -> Result<Vec<StandardPosition>> {
        // TODO: Implement ERC20 position fetching for V2-style protocols
        // This would involve:
        // 1. Discovering all pairs from factory events
        // 2. Checking user's LP token balance for each pair
        // 3. Getting pair details (reserves, tokens, etc.)
        
        warn!("ERC20 position fetching not yet implemented for {}", config.protocol.name);
        Ok(Vec::new())
    }
    
    async fn get_provider(&self, chain_id: u32) -> Result<alloy::providers::RootProvider<alloy::transports::http::Http<alloy::transports::http::Client>>> {
        let rpc_url = match chain_id {
            1 => "https://eth.llamarpc.com", // Ethereum
            137 => "https://polygon.llamarpc.com", // Polygon
            42161 => "https://arb1.arbitrum.io/rpc", // Arbitrum
            _ => return Err(anyhow::anyhow!("Unsupported chain ID: {}", chain_id)),
        };
        
        let provider = ProviderBuilder::new()
            .on_http(rpc_url.parse()?);
        
        Ok(provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_generic_fetcher_initialization() {
        let cache = Arc::new(CacheManager::new().await.unwrap());
        let fetcher = GenericFetcher::new(cache).await.unwrap();
        
        let protocols = fetcher.get_protocol_names();
        assert!(!protocols.is_empty());
        assert!(protocols.contains(&&"uniswap_v3".to_string()));
    }
    
    #[tokio::test]
    async fn test_supported_chains() {
        let cache = Arc::new(CacheManager::new().await.unwrap());
        let fetcher = GenericFetcher::new(cache).await.unwrap();
        
        let chains = fetcher.get_supported_chains("uniswap_v3").unwrap();
        assert!(chains.contains(&1)); // Ethereum
        assert!(chains.contains(&137)); // Polygon
        assert!(chains.contains(&42161)); // Arbitrum
    }
    
    #[tokio::test]
    async fn test_provider_creation() {
        let cache = Arc::new(CacheManager::new().await.unwrap());
        let fetcher = GenericFetcher::new(cache).await.unwrap();
        
        // Test Ethereum provider
        let provider = fetcher.get_provider(1).await.unwrap();
        let block_number = provider.get_block_number().await.unwrap();
        assert!(block_number > 0);
    }
}
