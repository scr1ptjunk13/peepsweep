// Re-export common types for easier imports
pub use std::sync::Arc;
pub use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;
use rust_decimal::prelude::ToPrimitive;

// Error types
pub type DatabaseResult<T> = Result<T, DatabaseError>;
pub type IndexerResult<T> = Result<T, IndexerError>;
pub type ApiResult<T> = Result<T, ApiError>;
pub type CacheResult<T> = Result<T, CacheError>;
pub type CalculationResult<T> = Result<T, CalculationError>;

// Cache TTL constants
pub const USER_POSITIONS_TTL: u64 = 300; // 5 minutes
pub const TOKEN_PRICES_TTL: u64 = 60;    // 1 minute
pub const IL_SNAPSHOTS_TTL: u64 = 600;   // 10 minutes

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Migration failed: {0}")]
    MigrationFailed(String),
    #[error("Query error: {0}")]
    QueryError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("Provider error: {0}")]
    ProviderError(String),
    #[error("Event processing error: {0}")]
    EventProcessingError(String),
    #[error("Event decoding failed: {0}")]
    EventDecodingFailed(String),
    #[error("Processing error: {0}")]
    ProcessingError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] DatabaseError),
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
    #[error("Block processing failed: {0}")]
    BlockProcessingFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    RedisError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Cache miss")]
    CacheMiss,
    #[error("Connection error: {0}")]
    ConnectionError(String),
    #[error("Operation error: {0}")]
    OperationError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum CalculationError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Price feed error: {0}")]
    PriceFeedError(String),
    #[error("Math error: {0}")]
    MathError(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Price not found: {0}")]
    PriceNotFound(String),
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    #[error("Decimal parsing error: {0}")]
    DecimalError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Database error: {0}")]
    DatabaseError(DatabaseError),
    #[error("Cache error: {0}")]
    CacheError(CacheError),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
    #[error("Calculation error: {0}")]
    CalculationError(CalculationError),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Indexer error: {0}")]
    IndexerError(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Not found: {0}")]
    NotFound(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;
        use axum::response::Json;
        use serde_json::json;

        let (status, error_message) = match self {
            ApiError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            ApiError::CacheError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Cache error"),
            ApiError::ValidationError(_) => (StatusCode::BAD_REQUEST, "Validation error"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "Not found"),
            ApiError::Forbidden(_) => (StatusCode::FORBIDDEN, "Forbidden"),
            ApiError::IndexerError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Indexer error"),
            ApiError::CalculationError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Calculation error"),
            ApiError::InternalError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };

        let body = Json(json!({
            "error": error_message,
            "message": self.to_string()
        }));

        (status, body).into_response()
    }
}

impl From<CalculationError> for ApiError {
    fn from(err: CalculationError) -> Self {
        ApiError::CalculationError(err)
    }
}

// Common structs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenPrice {
    pub token_address: String,
    pub price_usd: rust_decimal::Decimal,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// Legacy enum - deprecated in favor of config-driven protocols
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ProtocolVersion {
    V2,
    V3,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeesInfo {
    pub token0_amount: f64,
    pub token1_amount: f64,
    pub usd_amount: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PositionSummary {
    pub user_address: String,
    pub total_positions: i32,
    pub total_value_usd: f64,
    pub total_il_usd: f64,
    pub total_fees_usd: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Position {
    pub version: String,
    pub pool_address: String,
    pub token0: TokenInfo,
    pub token1: TokenInfo,
    pub fee_tier: Option<i32>,
    pub liquidity: String,
    pub token0_amount: f64,
    pub token1_amount: f64,
    pub position_value_usd: f64,
    pub hodl_value_usd: f64,
    pub in_range: Option<bool>,
    pub tick_range: Option<(i32, i32)>,
}

pub type ILCalculationResult = crate::calculations::impermanent_loss::ImpermanentLossCalculation;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpermanentLossInfo {
    pub percentage: f64,
    pub usd_amount: f64,
    pub is_gain: bool,
}

#[derive(Debug, Clone)]
pub struct CalculationMetrics {
    pub average_calculation_time_ms: std::sync::Arc<std::sync::RwLock<f64>>,
    pub successful_calculations: std::sync::Arc<std::sync::RwLock<u64>>,
    pub failed_calculations: std::sync::Arc<std::sync::RwLock<u64>>,
    pub total_calculations: std::sync::Arc<std::sync::RwLock<u64>>,
    pub cache_hits: std::sync::Arc<std::sync::RwLock<u64>>,
    pub cache_misses: std::sync::Arc<std::sync::RwLock<u64>>,
}

impl CalculationMetrics {
    pub fn new() -> Self {
        Self {
            average_calculation_time_ms: std::sync::Arc::new(std::sync::RwLock::new(0.0)),
            total_calculations: std::sync::Arc::new(std::sync::RwLock::new(0)),
            successful_calculations: std::sync::Arc::new(std::sync::RwLock::new(0)),
            failed_calculations: std::sync::Arc::new(std::sync::RwLock::new(0)),
            cache_hits: std::sync::Arc::new(std::sync::RwLock::new(0)),
            cache_misses: std::sync::Arc::new(std::sync::RwLock::new(0)),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriceSnapshot {
    pub token_address: String,
    pub price_usd: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: PriceSource,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PriceSource {
    Coingecko,
    Chainlink,
    Uniswap,
    Cache,
}


// Constants for pricing
pub const COINGECKO_API_BASE: &str = "https://api.coingecko.com/api/v3";
pub const CHAINLINK_PRICE_FEEDS: &str = "chainlink_feeds";

// Utility functions
pub fn validate_ethereum_address(address: &str) -> bool {
    address.len() == 42 && address.starts_with("0x")
}

// Module declarations
pub mod api;
pub mod cache;
pub mod calculations;
pub mod database;
pub mod fetchers;
pub mod indexer;
pub mod utils;