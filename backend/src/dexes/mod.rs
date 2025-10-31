// Universal DEX Framework
pub mod utils;

// DEX MODULES
pub mod velodrome;
pub mod uniswap_v3;
pub mod uniswap_v2;
pub mod pancakeswap_v2;
pub mod spiritswap_v2;
pub mod apeswap;
pub mod aerodrome;
pub mod sushiswap;

// DEX IMPORTS
pub use velodrome::VelodromeDex;
pub use uniswap_v3::UniswapV3Dex;
pub use uniswap_v2::UniswapV2Dex;
pub use pancakeswap_v2::PancakeSwapV2Dex;
pub use spiritswap_v2::SpiritSwapV2Dex;
pub use apeswap::ApeSwapDex;
pub use aerodrome::AerodromeDex;
pub use sushiswap::SushiSwapV2Dex;

use crate::types::{QuoteParams, RouteBreakdown, SwapParams};
use async_trait::async_trait;
use thiserror::Error;

// Re-export framework components
pub use utils::*;

#[derive(Error, Debug)]
pub enum DexError {
    #[error("Network request failed: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Invalid response from DEX: {0}")]
    InvalidResponse(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Insufficient liquidity")]
    InsufficientLiquidity,
    #[error("Unsupported trading pair: {0}")]
    UnsupportedPair(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Contract error: {0}")]
    ContractError(String),
    #[error("Contract call failed: {0}")]
    ContractCallFailed(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Unsupported chain: {0}")]
    UnsupportedChain(String),
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    #[error("No liquidity available")]
    NoLiquidity,
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Request timeout: {0}")]
    Timeout(String),
    #[error("Invalid address: {0}")] 
    InvalidAddress(String),
    #[error("Invalid pair: {0}")]
    InvalidPair(String),
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

#[async_trait]
pub trait DexIntegration: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_supported_chains(&self) -> Vec<&str>;
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError>;
    async fn execute_swap(&self, params: &SwapParams) -> Result<String, DexError>;
    async fn get_gas_estimate(&self, params: &SwapParams) -> Result<u64, DexError>;
    fn clone_box(&self) -> Box<dyn DexIntegration + Send + Sync>;
    
    /// NEW: Build transaction for gas estimation (optional implementation)
    /// Returns a TransactionRequest that can be used with eth_estimateGas
    async fn build_transaction(&self, _params: &QuoteParams) -> Result<alloy::rpc::types::TransactionRequest, DexError> {
        // Default implementation: not supported
        Err(DexError::NotImplemented(format!("{} does not support transaction building yet", self.get_name())))
    }
    
    async fn is_pair_supported(&self, _token_in: &str, _token_out: &str, _chain: &str) -> Result<bool, DexError> {
        // Default implementation - can be overridden by specific DEXes
        Ok(true)
    }
}
