// src/database/models.rs - Database model structs and conversions
use serde::{Deserialize, Serialize};
use rust_decimal::prelude::ToPrimitive;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use crate::{Address, TokenInfo, ProtocolVersion, Position as ApiPosition, U256};
use std::str::FromStr;

// ============================================================================
// POSITION MODELS (matching partitioned tables)
// ============================================================================

/// Uniswap V2 position model (maps to positions_v2 table)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionV2 {
    pub id: i64,
    pub user_address: String,
    pub pair_address: String,
    pub token0: String,
    pub token1: String,
    pub liquidity: Decimal,
    pub token0_amount: Decimal,
    pub token1_amount: Decimal,
    pub block_number: i64,
    pub transaction_hash: String,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_il_percentage: Option<Decimal>,
    pub fees_earned_usd: Option<Decimal>,
}

/// Uniswap V3 position model (maps to positions_v3 table)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PositionV3 {
    pub id: i64,
    pub user_address: String,
    pub pool_address: String,
    pub token_id: i64,
    pub token0: String,
    pub token1: String,
    pub fee_tier: i32,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: Decimal,
    pub token0_amount: Option<Decimal>,
    pub token1_amount: Option<Decimal>,
    pub fees_token0: Decimal,
    pub fees_token1: Decimal,
    pub block_number: i64,
    pub transaction_hash: String,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub current_tick: Option<i32>,
    pub in_range: Option<bool>,
    pub current_il_percentage: Option<Decimal>,
    pub fees_earned_usd: Option<Decimal>,
}

/// Unified position view (maps to user_positions_summary materialized view)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserPositionSummary {
    pub user_address: String,
    pub version: String, // 'v2' or 'v3'
    pub pool_address: String,
    pub token0: String,
    pub token1: String,
    pub fee_tier: Option<i32>,
    pub token0_amount: Option<Decimal>,
    pub token1_amount: Option<Decimal>,
    pub current_il_percentage: Option<Decimal>,
    pub fees_earned_usd: Option<Decimal>,
    pub updated_at: DateTime<Utc>,
}

impl UserPositionSummary {
    pub fn id(&self) -> i64 {
        0 // Placeholder - UserPositionSummary doesn't have an ID field
    }
    
    pub fn token0_address(&self) -> &str {
        &self.token0
    }
    
    pub fn token1_address(&self) -> &str {
        &self.token1
    }
    
    pub fn initial_token0_amount(&self) -> Decimal {
        self.token0_amount.unwrap_or(Decimal::ZERO)
    }
    
    pub fn initial_token1_amount(&self) -> Decimal {
        self.token1_amount.unwrap_or(Decimal::ZERO)
    }
    
    pub fn current_token0_amount(&self) -> Decimal {
        self.token0_amount.unwrap_or(Decimal::ZERO)
    }
    
    pub fn current_token1_amount(&self) -> Decimal {
        self.token1_amount.unwrap_or(Decimal::ZERO)
    }
    
    pub fn entry_price_token0(&self) -> Option<Decimal> {
        None // Not available in summary
    }
    
    pub fn entry_price_token1(&self) -> Option<Decimal> {
        None // Not available in summary
    }
    
    pub fn fees_earned_token0(&self) -> Decimal {
        Decimal::ZERO // Not available in summary
    }
    
    pub fn fees_earned_token1(&self) -> Decimal {
        Decimal::ZERO // Not available in summary
    }
}

// ============================================================================
// PRICE AND SNAPSHOT MODELS
// ============================================================================

/// Token price model (maps to token_prices table)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenPrice {
    pub token_address: String,
    pub price_usd: Option<Decimal>,
    pub price_eth: Option<Decimal>,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// IL snapshot model (maps to il_snapshots table)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IlSnapshot {
    pub id: i64,
    pub user_address: String,
    pub position_id: String,
    pub version: String,
    pub il_percentage: Decimal,
    pub hodl_value_usd: Decimal,
    pub position_value_usd: Decimal,
    pub fees_earned_usd: Decimal,
    pub net_result_usd: Decimal,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
}

/// Pool statistics model for analytics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PoolStats {
    pub pool_address: String,
    pub token0: String,
    pub token1: String,
    pub fee_tier: Option<i32>,
    pub total_volume_usd: Option<Decimal>,
    pub total_liquidity_usd: Option<Decimal>,
    pub total_positions: Option<i64>,
    pub avg_il_percentage: Option<Decimal>,
    pub average_il_percentage: Option<Decimal>,
    pub total_fees_earned_usd: Option<Decimal>,
    pub active_positions: Option<i64>,
}

// ============================================================================
// CONVERSION IMPLEMENTATIONS
// ============================================================================

impl PositionV2 {
    /// Convert database model to API model
    pub fn to_api_position(&self, token0_info: TokenInfo, token1_info: TokenInfo) -> Result<ApiPosition, Box<dyn std::error::Error>> {
        Ok(ApiPosition {
            version: "v2".to_string(),
            pool_address: self.pair_address.clone(),
            token0: token0_info,
            token1: token1_info,
            fee_tier: None,
            liquidity: self.liquidity.to_string(),
            token0_amount: self.token0_amount.to_f64().unwrap_or(0.0),
            token1_amount: self.token1_amount.to_f64().unwrap_or(0.0),
            position_value_usd: 0.0, // Would need calculation
            hodl_value_usd: 0.0, // Would need calculation
            in_range: None,
            tick_range: None,
        })
    }
    
    /// Create new V2 position for insertion
    pub fn new(
        user_address: String,
        pair_address: String,
        token0: String,
        token1: String,
        liquidity: Decimal,
        token0_amount: Decimal,
        token1_amount: Decimal,
        block_number: i64,
        transaction_hash: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // Will be set by database
            user_address,
            pair_address,
            token0,
            token1,
            liquidity,
            token0_amount,
            token1_amount,
            block_number,
            transaction_hash,
            timestamp: now,
            created_at: now,
            updated_at: now,
            current_il_percentage: None,
            fees_earned_usd: None,
        }
    }
    
    /// Calculate days active for this position
    pub fn days_active(&self) -> Option<i64> {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        Some(duration.num_days().max(1))
    }
}

impl PositionV3 {
    /// Convert database model to API model
    pub fn to_api_position(&self, token0_info: TokenInfo, token1_info: TokenInfo) -> Result<ApiPosition, Box<dyn std::error::Error>> {
        Ok(ApiPosition {
            version: "v3".to_string(),
            pool_address: self.pool_address.clone(),
            token0: token0_info,
            token1: token1_info,
            fee_tier: Some(self.fee_tier),
            liquidity: self.liquidity.to_string(),
            token0_amount: self.token0_amount.unwrap_or(Decimal::ZERO).to_f64().unwrap_or(0.0),
            token1_amount: self.token1_amount.unwrap_or(Decimal::ZERO).to_f64().unwrap_or(0.0),
            position_value_usd: 0.0, // Would need calculation
            hodl_value_usd: 0.0, // Would need calculation
            in_range: self.in_range,
            tick_range: Some((self.tick_lower, self.tick_upper)),
        })
    }
    
    /// Create new V3 position for insertion
    pub fn new(
        user_address: String,
        pool_address: String,
        token_id: i64,
        token0: String,
        token1: String,
        fee_tier: i32,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: Decimal,
        block_number: i64,
        transaction_hash: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // Will be set by database
            user_address,
            pool_address,
            token_id,
            token0,
            token1,
            fee_tier,
            tick_lower,
            tick_upper,
            liquidity,
            token0_amount: None,
            token1_amount: None,
            fees_token0: Decimal::ZERO,
            fees_token1: Decimal::ZERO,
            block_number,
            transaction_hash,
            timestamp: now,
            created_at: now,
            updated_at: now,
            current_tick: None,
            in_range: Some(true),
            current_il_percentage: None,
            fees_earned_usd: None,
        }
    }
    
    /// Calculate days active for this position
    pub fn days_active(&self) -> Option<i64> {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        Some(duration.num_days().max(1))
    }
}

impl UserPositionSummary {
    /// Convert to API position with token info lookup
    pub fn to_api_position_with_tokens(
        &self,
        token0_info: TokenInfo,
        token1_info: TokenInfo,
    ) -> Result<ApiPosition, Box<dyn std::error::Error>> {
        let version = match self.version.as_str() {
            "v2" => ProtocolVersion::V2,
            "v3" => ProtocolVersion::V3,
            _ => return Err("Invalid protocol version".into()),
        };
        
        Ok(ApiPosition {
            version: self.version.clone(),
            pool_address: self.pool_address.clone(),
            token0: token0_info,
            token1: token1_info,
            fee_tier: self.fee_tier,
            liquidity: "0".to_string(), // Would need to fetch from individual tables
            token0_amount: self.token0_amount.unwrap_or(Decimal::ZERO).to_f64().unwrap_or(0.0),
            token1_amount: self.token1_amount.unwrap_or(Decimal::ZERO).to_f64().unwrap_or(0.0),
            position_value_usd: 0.0, // UserPositionSummary doesn't have current_value_usd field
            hodl_value_usd: 0.0, // Would need calculation
            tick_range: None, // UserPositionSummary doesn't have tick fields
            in_range: None,
        })
    }
}

impl TokenPrice {
    /// Create new token price entry
    pub fn new(
        token_address: String,
        price_usd: Decimal,
        price_eth: Option<Decimal>,
        block_number: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            token_address,
            price_usd: Some(price_usd),
            price_eth,
            block_number,
            timestamp: now,
            updated_at: now,
        }
    }
}

impl IlSnapshot {
    /// Create new IL snapshot
    pub fn new(
        user_address: String,
        position_id: String,
        version: String,
        il_percentage: Decimal,
        hodl_value_usd: Decimal,
        position_value_usd: Decimal,
        fees_earned_usd: Decimal,
        net_result_usd: Decimal,
        block_number: i64,
    ) -> Self {
        Self {
            id: 0, // Will be set by database
            user_address,
            position_id,
            version,
            il_percentage,
            hodl_value_usd,
            position_value_usd,
            fees_earned_usd,
            net_result_usd,
            block_number,
            timestamp: Utc::now(),
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert string address to Address type with validation
pub fn parse_address(address_str: &str) -> Result<Address, Box<dyn std::error::Error>> {
    Address::from_str(address_str)
        .map_err(|e| e.into())
}

/// Format position ID for V2 positions
pub fn format_v2_position_id(user_address: &str, pair_address: &str) -> String {
    format!("{}:{}", user_address, pair_address)
}

/// Format position ID for V3 positions
pub fn format_v3_position_id(user_address: &str, token_id: i64) -> String {
    format!("{}:{}", user_address, token_id)
}

/// Extract user address from position ID
pub fn extract_user_address(position_id: &str) -> Option<&str> {
    position_id.split(':').next()
}

/// Price snapshot model for storing historical price data
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PriceSnapshot {
    pub token_address: String,
    pub price_usd: Decimal,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}
