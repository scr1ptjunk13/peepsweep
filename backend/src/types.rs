use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Chain {
    Ethereum,
    Polygon,
    Arbitrum,
    Optimism,
    Base,
    Avalanche,
    BNB,
    Fantom,
}

impl Chain {
    pub fn as_str(&self) -> &'static str {
        match self {
            Chain::Ethereum => "ethereum",
            Chain::Polygon => "polygon",
            Chain::Arbitrum => "arbitrum",
            Chain::Optimism => "optimism",
            Chain::Base => "base",
            Chain::Avalanche => "avalanche",
            Chain::BNB => "bnb",
            Chain::Fantom => "fantom",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ethereum" | "eth" => Some(Chain::Ethereum),
            "polygon" | "matic" => Some(Chain::Polygon),
            "arbitrum" | "arb" => Some(Chain::Arbitrum),
            "optimism" | "op" => Some(Chain::Optimism),
            "base" => Some(Chain::Base),
            "avalanche" | "avax" => Some(Chain::Avalanche),
            "bnb" | "bsc" => Some(Chain::BNB),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnhancedRouteBreakdown {
    pub dex: String,
    pub amount_out: String,
    pub gas_used: String,
    pub execution_time_ms: u64,
    pub confidence_score: f64,
    
    // NEW: Enhanced data
    pub price_impact: Option<f64>,           // Real price impact %
    pub price_impact_category: Option<String>, // "low", "Medium", "High", etc.
    pub real_gas_estimate: Option<u64>,      // Actual blockchain gas estimate
    pub gas_cost_usd: Option<f64>,           // Gas cost in USD
    pub gas_savings_vs_hardcoded: Option<f64>, // % savings vs hardcoded
    pub liquidity_depth: Option<String>,     // "High", "Medium", "low"
    pub recommended_slippage: Option<f64>,   // Recommended slippage %
    pub trade_recommendation: Option<String>, // "Execute", "Split", "Avoid"
    pub reserve_info: Option<ReserveInfo>,   // Reserve details
    
    // NEW: Advanced slippage analysis
    pub slippage_analysis: Option<SlippageBreakdown>, // Detailed slippage breakdown
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlippageBreakdown {
    pub recommended_slippage: f64,      // Final recommended slippage %
    pub minimum_slippage: f64,          // Absolute minimum based on price impact
    pub conservative_slippage: f64,     // Conservative estimate for safety
    pub aggressive_slippage: f64,       // Aggressive estimate for speed
    pub liquidity_score: f64,           // 0-100 liquidity depth score
    pub volatility_factor: f64,         // Market volatility multiplier
    pub gas_pressure_factor: f64,       // Gas price impact on slippage
    pub confidence_level: f64,          // Confidence in the estimate (0-1)
    pub reasoning: String,              // Human-readable explanation
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReserveInfo {
    pub reserve0: String,
    pub reserve1: String,
    pub reserve0_formatted: String,  // Human readable (e.g., "24.5M USDC")
    pub reserve1_formatted: String,  // Human readable (e.g., "6,468 ETH")
    pub total_liquidity_usd: Option<f64>,
    pub pair_address: String,
    pub last_updated: u32,  // timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BundleStatus {
    Pending,
    Included,
    Failed,
    Timeout,
}

// Enhanced QuoteParams that works with token discovery system
#[derive(Debug, Clone, Deserialize)]
pub struct QuoteParams {
    #[serde(rename = "tokenIn")]
    pub token_in: String,           // Symbol (for display)
    #[serde(rename = "tokenInAddress", default)]
    pub token_in_address: Option<String>,   // Contract address from discovery
    #[serde(rename = "tokenInDecimals", default)]
    pub token_in_decimals: Option<u8>,      // Decimals from discovery
    
    #[serde(rename = "tokenOut")]
    pub token_out: String,          // Symbol (for display) 
    #[serde(rename = "tokenOutAddress", default)]
    pub token_out_address: Option<String>,  // Contract address from discovery
    #[serde(rename = "tokenOutDecimals", default)]
    pub token_out_decimals: Option<u8>,     // Decimals from discovery
    
    #[serde(rename = "amountIn")]
    pub amount_in: String,          // Human readable amount
    pub chain: Option<String>,      // Chain name
    pub slippage: Option<f64>,      // Slippage tolerance
}

// Example usage with token discovery system
impl QuoteParams {
    pub fn from_discovery_tokens(
        token_in_symbol: &str,
        token_in_address: &str,
        token_in_decimals: u8,
        token_out_symbol: &str,
        token_out_address: &str,
        token_out_decimals: u8,
        amount: &str,
        chain: &str
    ) -> Self {
        Self {
            token_in: token_in_symbol.to_string(),
            token_in_address: Some(token_in_address.to_string()),
            token_in_decimals: Some(token_in_decimals),
            
            token_out: token_out_symbol.to_string(),
            token_out_address: Some(token_out_address.to_string()),
            token_out_decimals: Some(token_out_decimals),
            
            amount_in: amount.to_string(),
            chain: Some(chain.to_string()),
            slippage: Some(0.5),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "responseTime")]
    pub response_time: u128,
    pub routes: Vec<RouteBreakdown>,
    #[serde(rename = "priceImpact")]
    pub price_impact: f64,
    #[serde(rename = "gasEstimate")]
    pub gas_estimate: String,
    pub savings: Option<SavingsComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBreakdown {
    pub dex: String,
    pub percentage: f64,
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "gasUsed")]
    pub gas_used: String,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsComparison {
    #[serde(rename = "vsUniswap")]
    pub vs_uniswap: f64,
    #[serde(rename = "vsSushiswap")]
    pub vs_sushiswap: f64,
    #[serde(rename = "vs1inch")]
    pub vs_1inch: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwapParams {
    #[serde(rename = "tokenIn")]
    pub token_in: String,
    #[serde(rename = "tokenOut")]
    pub token_out: String,
    #[serde(rename = "amountIn")]
    pub amount_in: String,
    #[serde(rename = "amountOutMin")]
    pub amount_out_min: String,
    pub routes: Vec<RouteBreakdown>,
    #[serde(rename = "userAddress")]
    pub user_address: String,
    pub slippage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapResponse {
    #[serde(rename = "txHash")]
    pub tx_hash: String,
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "gasUsed")]
    pub gas_used: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
    pub status: String,
    #[serde(rename = "mevProtection")]
    pub mev_protection: Option<String>,
    #[serde(rename = "executionTimeMs")]
    pub execution_time_ms: u64,
}

// DEX-specific response types
#[derive(Debug, Clone, Deserialize)]
pub struct OneInchQuote {
    #[serde(rename = "toTokenAmount")]
    pub to_token_amount: String,
    #[serde(rename = "estimatedGas")]
    pub estimated_gas: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UniswapQuote {
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "gasEstimate")]
    pub gas_estimate: String,
    pub route: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SushiswapQuote {
    #[serde(rename = "amountOut")]
    pub amount_out: String,
    #[serde(rename = "priceImpact")]
    pub price_impact: String,
    #[serde(rename = "gasPrice")]
    pub gas_price: String,
}

/// Route request for preference-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub from_token: String,
    pub to_token: String,
    pub amount: Decimal,
    pub user_id: Option<Uuid>,
}

/// Route hop representing a single DEX interaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    pub dex_name: String,
    pub from_token: String,
    pub to_token: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub gas_estimate: Decimal,
    pub pool_address: Option<String>,
}

/// Complete route with multiple hops and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub from_token: String,
    pub to_token: String,
    pub input_amount: Decimal,
    pub output_amount: Decimal,
    pub hops: Vec<RouteHop>,
    pub gas_estimate: Decimal,
    pub estimated_slippage: Decimal,
    pub liquidity_usd: Decimal,
    pub execution_time_estimate_ms: u64,
    pub mev_protection_level: String,
}

/// Route step for DEX framework - represents a single swap step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub dex: String,
    pub token_in: alloy::primitives::Address,
    pub token_out: alloy::primitives::Address,
    pub amount_in: alloy::primitives::U256,
    pub amount_out: alloy::primitives::U256,
    pub fee: u32,
    pub pool_address: Option<alloy::primitives::Address>,
}

// DexQuote struct for TUI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexQuote {
    pub dex_name: String,
    pub output_amount: String,
    pub gas_estimate: u64,
    pub slippage: f64,
    pub price_impact: f64,
}
