use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rust_decimal::Decimal;
use uuid::Uuid;

pub type UserId = Uuid;
pub type TradeId = Uuid;
pub type TokenAddress = String;
pub type DexId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeEvent {
    pub user_id: UserId,
    pub trade_id: TradeId,
    pub token_in: TokenAddress,
    pub token_out: TokenAddress,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub timestamp: u64,
    pub dex_source: DexId,
    pub gas_used: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenBalance {
    pub token_address: TokenAddress,
    pub balance: Decimal,
    pub value_usd: Decimal,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPositions {
    pub balances: HashMap<TokenAddress, TokenBalance>,
    pub pnl: Decimal,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub total_exposure_usd: Decimal,
    pub concentration_risk: Decimal,
    pub var_95: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: Decimal,
    pub win_rate: Decimal,
    pub avg_trade_size: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureSnapshot {
    pub user_id: UserId,
    pub total_exposure_usd: Decimal,
    pub token_exposures: Vec<TokenExposure>,
    pub timestamp: u64,
    pub calculation_time_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenExposure {
    pub token: TokenAddress,
    pub amount: Decimal,
    pub value_usd: Decimal,
    pub percentage: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub price: Decimal,
    pub timestamp: u64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub user_id: UserId,
    pub alert_id: String,
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: u64,
    pub trade_id: Option<TradeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, thiserror::Error)]
pub enum RiskError {
    #[error("User not found: {0}")]
    UserNotFound(UserId),
    #[error("Price data missing for token: {0}")]
    PriceMissing(TokenAddress),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Price not found: {0}")]
    PriceNotFound(String),
    #[error("Data integrity error: {0}")]
    DataIntegrityError(String),
    #[error("Cache error: {0}")]
    CacheError(String),
    #[error("Calculation error: {0}")]
    CalculationError(String),
    #[error("System error: {0}")]
    SystemError(String),
    #[error("Routing error: {0}")]
    RoutingError(String),
    #[error("Compression error: {0}")]
    CompressionError(String),
    #[error("SQL error: {0}")]
    SqlError(sqlx::Error),
    #[error("External API error: {0}")]
    ExternalApiError(String),
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    #[error("Service already running: {0}")]
    ServiceAlreadyRunning(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Notification error: {0}")]
    NotificationError(String),
}

impl From<sqlx::Error> for RiskError {
    fn from(error: sqlx::Error) -> Self {
        RiskError::SqlError(error)
    }
}

impl From<serde_json::Error> for RiskError {
    fn from(error: serde_json::Error) -> Self {
        RiskError::SerializationError(error.to_string())
    }
}

impl UserPositions {
    pub fn new() -> Self {
        Self {
            balances: HashMap::new(),
            pnl: Decimal::ZERO,
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
}

impl Default for UserPositions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_trade_event_creation() {
        let trade = TradeEvent {
            user_id: Uuid::new_v4(),
            trade_id: Uuid::new_v4(),
            token_in: "ETH".to_string(),
            token_out: "USDC".to_string(),
            amount_in: dec!(1.0),
            amount_out: dec!(3400.0),
            timestamp: 1640995200000,
            dex_source: "uniswap".to_string(),
            gas_used: dec!(0.01),
        };

        assert_eq!(trade.token_in, "ETH");
        assert_eq!(trade.amount_in, dec!(1.0));
    }

    #[test]
    fn test_user_positions_default() {
        let positions = UserPositions::new();
        assert_eq!(positions.pnl, Decimal::ZERO);
        assert!(positions.balances.is_empty());
    }

    #[test]
    fn test_token_balance_creation() {
        let balance = TokenBalance {
            token_address: "0x1234".to_string(),
            balance: dec!(100.0),
            value_usd: dec!(340000.0),
            last_updated: 1234567890,
        };

        assert_eq!(balance.balance, dec!(100.0));
        assert_eq!(balance.value_usd, dec!(340000.0));
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert_ne!(AlertSeverity::Low, AlertSeverity::High);
        assert_eq!(AlertSeverity::Critical, AlertSeverity::Critical);
    }

    #[test]
    fn test_exposure_snapshot_creation() {
        let user_id = Uuid::new_v4();
        let snapshot = ExposureSnapshot {
            user_id,
            total_exposure_usd: dec!(10000.0),
            token_exposures: vec![],
            timestamp: 1640995200000,
            calculation_time_us: 500,
        };

        assert_eq!(snapshot.user_id, user_id);
        assert_eq!(snapshot.total_exposure_usd, dec!(10000.0));
    }
}
