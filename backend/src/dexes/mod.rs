// Universal DEX Framework
pub mod utils;

// Active DEXes for testing
pub mod apeswap;
pub mod pancakeswap;
pub mod sushiswap;
// pub mod uniswap;
pub mod spookyswap;
pub mod spiritswap;

// Commented out other DEXes for testing
// pub mod aerodrome;
// pub mod balancer;
// pub mod bancor;
// pub mod biswap;
// pub mod camelot;
// pub mod cowswap;
// pub mod curve;
// pub mod dodo;
// pub mod dydx;
// pub mod fraxswap;
// pub mod kyber;
// pub mod kyberswap;
pub mod manager;
// pub mod maverick;
// pub mod pancakeswap_v2;
// pub mod quickswap;
// pub mod traderjoe;
// pub mod uniswap_v2;
pub mod velodrome;
// pub mod beethovenx;

// Active DEX imports
pub use apeswap::ApeSwapDex;
pub use pancakeswap::PancakeSwapDex;
pub use sushiswap::SushiswapDex;
// pub use uniswap::UniswapDex;
pub use spookyswap::SpookySwapDex;
pub use spiritswap::SpiritSwapDex;

// Commented out other DEX imports for testing
// pub use aerodrome::AerodromeDex;
// pub use balancer::BalancerDex;
// pub use bancor::BancorDex;
// pub use beethovenx::BeethovenXDex;
// pub use biswap::BiSwapDex;
// pub use camelot::CamelotDex;
// pub use cowswap::CowSwapDex;
// pub use curve::CurveDex;
// pub use dodo::DodoDex;
// pub use dydx::DydxDex;
// pub use fraxswap::FraxswapDex;
// pub use kyber::KyberDex;
// pub use kyberswap::KyberSwapDex;
pub use manager::DexManager;
// pub use maverick::MaverickDex;
// pub use pancakeswap_v2::PancakeSwapV2Dex;
// pub use quickswap::QuickSwapDex;
// pub use traderjoe::TraderJoeDex;
// pub use uniswap_v2::UniswapV2Dex;
pub use velodrome::VelodromeDex;

use crate::types::{QuoteParams, RouteBreakdown};
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
    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError>;
    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Default implementation for backward compatibility
        Ok(false)
    }
    fn get_name(&self) -> &'static str;
    fn get_supported_chains(&self) -> Vec<&'static str>;
}
