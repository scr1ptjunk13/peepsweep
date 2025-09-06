use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "token_type")]
pub enum TokenType {
    #[sqlx(rename = "Native")]
    Native,
    #[sqlx(rename = "ERC20")]
    ERC20,
    #[sqlx(rename = "ERC721")]
    ERC721,
    #[sqlx(rename = "ERC1155")]
    ERC1155,
    #[sqlx(rename = "Wrapped")]
    Wrapped,
    #[sqlx(rename = "Stable")]
    Stable,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenType::Native => write!(f, "Native"),
            TokenType::ERC20 => write!(f, "ERC20"),
            TokenType::ERC721 => write!(f, "ERC721"),
            TokenType::ERC1155 => write!(f, "ERC1155"),
            TokenType::Wrapped => write!(f, "Wrapped"),
            TokenType::Stable => write!(f, "Stable"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "verification_level")]
pub enum VerificationLevel {
    #[sqlx(rename = "Unverified")]
    Unverified,
    #[sqlx(rename = "Community")]
    Community,
    #[sqlx(rename = "DEX")]
    DEX,
    #[sqlx(rename = "Verified")]
    Verified,
    #[sqlx(rename = "Official")]
    Official,
}

impl std::fmt::Display for VerificationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationLevel::Unverified => write!(f, "Unverified"),
            VerificationLevel::Community => write!(f, "Community"),
            VerificationLevel::DEX => write!(f, "DEX"),
            VerificationLevel::Verified => write!(f, "Verified"),
            VerificationLevel::Official => write!(f, "Official"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_status", rename_all = "PascalCase")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Chain {
    pub id: i64,
    pub name: String,
    pub symbol: String,
    pub native_currency_symbol: String,
    pub native_currency_decimals: i32,
    pub rpc_urls: Vec<String>,
    pub block_explorer_url: Option<String>,
    pub is_testnet: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Token {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub token_type: TokenType,
    pub decimals: i32,
    pub total_supply: Option<Decimal>,
    pub is_verified: bool,
    pub verification_level: VerificationLevel,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub twitter_handle: Option<String>,
    pub telegram_url: Option<String>,
    pub discord_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenAddress {
    pub id: Uuid,
    pub token_id: Uuid,
    pub chain_id: i64,
    pub address: String,
    pub is_native: bool,
    pub is_wrapped: bool,
    pub proxy_address: Option<String>,
    pub implementation_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenLogo {
    pub id: Uuid,
    pub token_id: Uuid,
    pub logo_url: Option<String>,
    pub local_path: Option<String>,
    pub cdn_url: Option<String>,
    pub image_format: Option<String>,
    pub image_size: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub is_cached: bool,
    pub cache_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenMarketData {
    pub id: Uuid,
    pub token_id: Uuid,
    pub price_usd: Option<Decimal>,
    pub market_cap_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub volume_7d_usd: Option<Decimal>,
    pub price_change_24h: Option<Decimal>,
    pub price_change_7d: Option<Decimal>,
    pub circulating_supply: Option<Decimal>,
    pub max_supply: Option<Decimal>,
    pub ath_usd: Option<Decimal>,
    pub atl_usd: Option<Decimal>,
    pub liquidity_usd: Option<Decimal>,
    pub holders_count: Option<i64>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenSource {
    pub id: Uuid,
    pub token_id: Uuid,
    pub source_name: String,
    pub source_priority: i32,
    pub first_discovered_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub is_active: bool,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenTag {
    pub id: Uuid,
    pub token_id: Uuid,
    pub tag: String,
    pub category: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenSecurity {
    pub id: Uuid,
    pub token_id: Uuid,
    pub is_honeypot: Option<bool>,
    pub is_rugpull_risk: Option<bool>,
    pub contract_verified: Option<bool>,
    pub proxy_contract: Option<bool>,
    pub mint_function: Option<bool>,
    pub burn_function: Option<bool>,
    pub pause_function: Option<bool>,
    pub blacklist_function: Option<bool>,
    pub security_score: Option<i32>,
    pub last_analyzed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenLiquidity {
    pub id: Uuid,
    pub token_id: Uuid,
    pub chain_id: i64,
    pub dex_name: String,
    pub pool_address: Option<String>,
    pub pair_token_symbol: Option<String>,
    pub liquidity_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub fee_tier: Option<Decimal>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DiscoveryJob {
    pub id: Uuid,
    pub job_type: String,
    pub chain_id: Option<i64>,
    pub source_name: Option<String>,
    pub status: JobStatus,
    pub tokens_processed: i32,
    pub tokens_added: i32,
    pub tokens_updated: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Unified token view for API responses
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UnifiedTokenView {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub token_type: TokenType,
    pub decimals: i32,
    pub is_verified: bool,
    pub verification_level: VerificationLevel,
    pub chain_addresses: Option<serde_json::Value>, // JSON object with chain_id -> address mapping
    pub logo_url: Option<String>,
    pub cdn_url: Option<String>,
    pub price_usd: Option<Decimal>,
    pub market_cap_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub updated_at: DateTime<Utc>,
}

// API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToken {
    pub id: Uuid,
    pub symbol: String,
    pub name: String,
    pub token_type: TokenType,
    pub decimals: i32,
    pub is_verified: bool,
    pub verification_level: VerificationLevel,
    pub chain_addresses: HashMap<i64, String>,
    pub logo_url: Option<String>,
    pub cdn_url: Option<String>,
    pub price_usd: Option<Decimal>,
    pub market_cap_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub market_data: Option<TokenMarketDataResponse>,
    pub security: Option<TokenSecurityResponse>,
    pub tags: Vec<String>,
    pub sources: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMarketDataResponse {
    pub price_usd: Option<Decimal>,
    pub market_cap_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub price_change_24h: Option<Decimal>,
    pub liquidity_usd: Option<Decimal>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSecurityResponse {
    pub is_honeypot: Option<bool>,
    pub contract_verified: Option<bool>,
    pub security_score: Option<i32>,
    pub last_analyzed: DateTime<Utc>,
}

// Database insertion structures
#[derive(Debug, Clone)]
pub struct NewToken {
    pub symbol: String,
    pub name: String,
    pub coingecko_id: Option<String>,
    pub token_type: TokenType,
    pub decimals: i32,
    pub total_supply: Option<Decimal>,
    pub is_verified: bool,
    pub verification_level: VerificationLevel,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub twitter_handle: Option<String>,
    pub telegram_url: Option<String>,
    pub discord_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewTokenAddress {
    pub token_id: Uuid,
    pub chain_id: i64,
    pub address: String,
    pub is_native: bool,
    pub is_wrapped: bool,
    pub proxy_address: Option<String>,
    pub implementation_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewTokenMarketData {
    pub token_id: Uuid,
    pub price_usd: Option<Decimal>,
    pub market_cap_usd: Option<Decimal>,
    pub volume_24h_usd: Option<Decimal>,
    pub volume_7d_usd: Option<Decimal>,
    pub price_change_24h: Option<Decimal>,
    pub price_change_7d: Option<Decimal>,
    pub circulating_supply: Option<Decimal>,
    pub max_supply: Option<Decimal>,
    pub ath_usd: Option<Decimal>,
    pub atl_usd: Option<Decimal>,
    pub liquidity_usd: Option<Decimal>,
    pub holders_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewTokenSource {
    pub token_id: Uuid,
    pub source_name: String,
    pub source_priority: i32,
    pub metadata: Option<serde_json::Value>,
}

impl From<UnifiedTokenView> for UnifiedToken {
    fn from(view: UnifiedTokenView) -> Self {
        let chain_addresses = if let Some(addresses_json) = view.chain_addresses {
            serde_json::from_value::<HashMap<String, String>>(addresses_json)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(k, v)| k.parse::<i64>().ok().map(|chain_id| (chain_id, v)))
                .collect()
        } else {
            HashMap::new()
        };

        let market_data = if view.price_usd.is_some() || view.market_cap_usd.is_some() || view.volume_24h_usd.is_some() {
            Some(TokenMarketDataResponse {
                price_usd: view.price_usd,
                market_cap_usd: view.market_cap_usd,
                volume_24h_usd: view.volume_24h_usd,
                price_change_24h: None,
                liquidity_usd: None,
                last_updated: view.updated_at,
            })
        } else {
            None
        };

        UnifiedToken {
            id: view.id,
            symbol: view.symbol,
            name: view.name,
            token_type: view.token_type,
            decimals: view.decimals,
            is_verified: view.is_verified,
            verification_level: view.verification_level,
            chain_addresses,
            logo_url: view.logo_url,
            cdn_url: view.cdn_url,
            price_usd: None,
            market_cap_usd: None,
            volume_24h_usd: None,
            market_data,
            security: None,
            tags: Vec::new(),
            sources: Vec::new(),
            updated_at: view.updated_at,
        }
    }
}
